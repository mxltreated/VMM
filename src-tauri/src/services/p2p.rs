use crate::models::P2pStatus;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use iroh::{endpoint::presets, Endpoint, EndpointAddr};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use subtle::ConstantTimeEq;
use tauri::async_runtime::JoinHandle;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

const ROOM_PREFIX: &str = "vmm-p2p:";
const ALPN: &[u8] = b"vmm-vs-p2p/1";
const MAX_ROOM_CODE_LEN: usize = 4096;
const MAX_P2P_PEERS: usize = 32;
const MAX_STREAMS_PER_PEER: usize = 16;
const MAX_CLIENT_PROXY_STREAMS: usize = 32;
const AUTH_TIMEOUT: Duration = Duration::from_secs(5);
const ONLINE_TIMEOUT: Duration = Duration::from_secs(15);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoomTicket {
    addr: EndpointAddr,
    secret: [u8; 16],
}

pub fn encode_room_ticket(addr: &EndpointAddr, secret: [u8; 16]) -> Result<String, String> {
    let ticket = RoomTicket {
        addr: addr.clone(),
        secret,
    };
    let json = serde_json::to_vec(&ticket)
        .map_err(|e| format!("Failed to serialize P2P room ticket: {e}"))?;
    Ok(format!("{ROOM_PREFIX}{}", URL_SAFE_NO_PAD.encode(json)))
}

fn decode_room_ticket(code: &str) -> Result<RoomTicket, String> {
    let trimmed = code.trim();
    if trimmed.len() > MAX_ROOM_CODE_LEN {
        return Err("Room Code exceeds maximum allowed length".to_string());
    }
    let payload = trimmed
        .strip_prefix(ROOM_PREFIX)
        .ok_or_else(|| "Invalid Room Code: expected 'vmm-p2p:' prefix".to_string())?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| format!("Invalid Room Code encoding: {e}"))?;
    serde_json::from_slice::<RoomTicket>(&bytes)
        .map_err(|e| format!("Invalid Room Code payload: {e}"))
}

struct ActiveP2p {
    mode: String,
    room_code: Option<String>,
    local_address: Option<String>,
    peers: Arc<AtomicUsize>,
    running: Arc<AtomicBool>,
    task: JoinHandle<()>,
    endpoint: Endpoint,
}

pub struct P2pManager {
    inner: Mutex<Option<ActiveP2p>>,
    lifecycle: AsyncMutex<()>,
}

impl P2pManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            lifecycle: AsyncMutex::new(()),
        }
    }

    pub fn status(&self) -> P2pStatus {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => {
                return P2pStatus {
                    active: false,
                    mode: "idle".to_string(),
                    room_code: None,
                    local_address: None,
                    connected_peers: 0,
                }
            }
        };

        if let Some(active) = guard.as_ref() {
            if active.running.load(Ordering::Relaxed) {
                return P2pStatus {
                    active: true,
                    mode: active.mode.clone(),
                    room_code: active.room_code.clone(),
                    local_address: active.local_address.clone(),
                    connected_peers: active.peers.load(Ordering::Relaxed),
                };
            }
        }

        P2pStatus {
            active: false,
            mode: "idle".to_string(),
            room_code: None,
            local_address: None,
            connected_peers: 0,
        }
    }

    fn take_active(&self) -> Option<ActiveP2p> {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(active) = guard.take() {
                active.running.store(false, Ordering::Relaxed);
                active.task.abort();
                return Some(active);
            }
        }
        None
    }

    pub fn stop(&self) -> P2pStatus {
        if let Some(active) = self.take_active() {
            let ep = active.endpoint;
            tauri::async_runtime::spawn(async move {
                ep.close().await;
            });
        }
        P2pStatus {
            active: false,
            mode: "idle".to_string(),
            room_code: None,
            local_address: None,
            connected_peers: 0,
        }
    }

    async fn stop_and_await_close(&self) {
        if let Some(active) = self.take_active() {
            active.endpoint.close().await;
        }
    }

    pub async fn start_host(&self, target_port: u16) -> Result<P2pStatus, String> {
        if target_port < 1024 {
            return Err("P2P target port must be between 1024 and 65535".to_string());
        }

        let _lifecycle_guard = self.lifecycle.lock().await;
        self.stop_and_await_close().await;

        let endpoint = Endpoint::builder(presets::N0)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| format!("Failed to start P2P endpoint: {e}"))?;

        if tokio::time::timeout(ONLINE_TIMEOUT, async { endpoint.online().await })
            .await
            .is_err()
        {
            endpoint.close().await;
            return Err("Timed out establishing P2P relay connectivity".to_string());
        }

        let addr = endpoint.addr();
        let mut secret = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut secret);
        let room_code = encode_room_ticket(&addr, secret)?;

        let peers = Arc::new(AtomicUsize::new(0));
        let peers_clone = Arc::clone(&peers);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let ep_clone = endpoint.clone();
        let peer_semaphore = Arc::new(Semaphore::new(MAX_P2P_PEERS));

        let task = tauri::async_runtime::spawn(async move {
            while let Some(incoming) = ep_clone.accept().await {
                let Ok(peer_permit) = Arc::clone(&peer_semaphore).try_acquire_owned() else {
                    if let Ok(Ok(conn)) =
                        tokio::time::timeout(AUTH_TIMEOUT, async move { incoming.await }).await
                    {
                        conn.close(2u32.into(), b"server full");
                    }
                    continue;
                };

                let peers_conn = Arc::clone(&peers_clone);
                tauri::async_runtime::spawn(async move {
                    let _permit = peer_permit;
                    let Ok(Ok(conn)) =
                        tokio::time::timeout(CONNECT_TIMEOUT, async move { incoming.await }).await
                    else {
                        return;
                    };

                    let stream_sem = Arc::new(Semaphore::new(MAX_STREAMS_PER_PEER));
                    let mut authenticated = false;

                    while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                        let mut token = [0u8; 16];
                        let read_res =
                            tokio::time::timeout(AUTH_TIMEOUT, recv.read_exact(&mut token)).await;
                        let token_valid = matches!(read_res, Ok(Ok(())))
                            && bool::from(token.ct_eq(&secret));

                        if !token_valid {
                            let _ = send.finish();
                            conn.close(1u32.into(), b"unauthorized");
                            break;
                        }

                        if !authenticated {
                            authenticated = true;
                            peers_conn.fetch_add(1, Ordering::Relaxed);
                        }

                        let Ok(stream_permit) = Arc::clone(&stream_sem).try_acquire_owned() else {
                            let _ = send.finish();
                            continue;
                        };

                        tauri::async_runtime::spawn(async move {
                            let _stream_permit = stream_permit;
                            let Ok(Ok(tcp)) = tokio::time::timeout(
                                TCP_CONNECT_TIMEOUT,
                                TcpStream::connect(("127.0.0.1", target_port)),
                            )
                            .await
                            else {
                                let _ = send.finish();
                                return;
                            };
                            let (mut tcp_r, mut tcp_w) = tcp.into_split();
                            let _ = tokio::try_join!(
                                async {
                                    let res = tokio::io::copy(&mut tcp_r, &mut send).await;
                                    let _ = send.finish();
                                    res
                                },
                                async {
                                    let res = tokio::io::copy(&mut recv, &mut tcp_w).await;
                                    let _ = tcp_w.shutdown().await;
                                    res
                                }
                            );
                        });
                    }

                    if authenticated {
                        peers_conn.fetch_sub(1, Ordering::Relaxed);
                    }
                });
            }
            running_clone.store(false, Ordering::Relaxed);
        });

        let status = P2pStatus {
            active: true,
            mode: "host".to_string(),
            room_code: Some(room_code.clone()),
            local_address: Some(format!("127.0.0.1:{target_port}")),
            connected_peers: 0,
        };

        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(ActiveP2p {
                mode: "host".to_string(),
                room_code: Some(room_code),
                local_address: status.local_address.clone(),
                peers,
                running,
                task,
                endpoint,
            });
        }

        Ok(status)
    }

    pub async fn start_client(&self, room_code: &str, preferred_port: u16) -> Result<P2pStatus, String> {
        let ticket = decode_room_ticket(room_code)?;
        let _lifecycle_guard = self.lifecycle.lock().await;
        self.stop_and_await_close().await;

        let endpoint = Endpoint::bind(presets::N0)
            .await
            .map_err(|e| format!("Failed to start P2P client endpoint: {e}"))?;

        let conn = match tokio::time::timeout(
            CONNECT_TIMEOUT,
            endpoint.connect(ticket.addr.clone(), ALPN),
        )
        .await
        {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                endpoint.close().await;
                return Err(format!("Could not connect to host P2P room: {e}"));
            }
            Err(_) => {
                endpoint.close().await;
                return Err("Timed out connecting to host P2P room".to_string());
            }
        };

        let listener = match TcpListener::bind(("127.0.0.1", preferred_port)).await {
            Ok(l) => l,
            Err(_) => match TcpListener::bind(("127.0.0.1", 0)).await {
                Ok(l) => l,
                Err(e) => {
                    endpoint.close().await;
                    return Err(format!("Failed to bind local P2P proxy socket: {e}"));
                }
            },
        };
        let local_port = listener
            .local_addr()
            .map_err(|e| format!("Failed to read local proxy port: {e}"))?
            .port();
        let local_address = format!("127.0.0.1:{local_port}");

        let peers = Arc::new(AtomicUsize::new(1));
        let peers_clone = Arc::clone(&peers);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let secret = ticket.secret;
        let ep_clone = endpoint.clone();
        let remote_addr = ticket.addr;
        let stream_sem = Arc::new(Semaphore::new(MAX_CLIENT_PROXY_STREAMS));

        let task = tauri::async_runtime::spawn(async move {
            let mut active_conn = conn;
            while let Ok((tcp, _)) = listener.accept().await {
                let Ok(stream_permit) = Arc::clone(&stream_sem).try_acquire_owned() else {
                    drop(tcp);
                    continue;
                };
                if active_conn.close_reason().is_some() {
                    if let Ok(Ok(reconnected)) = tokio::time::timeout(
                        CONNECT_TIMEOUT,
                        ep_clone.connect(remote_addr.clone(), ALPN),
                    )
                    .await
                    {
                        active_conn = reconnected;
                        peers_clone.store(1, Ordering::Relaxed);
                    } else {
                        peers_clone.store(0, Ordering::Relaxed);
                        drop(stream_permit);
                        continue;
                    }
                }
                let conn_ref = active_conn.clone();
                tauri::async_runtime::spawn(async move {
                    let _permit = stream_permit;
                    let Ok((mut send, mut recv)) = conn_ref.open_bi().await else {
                        return;
                    };
                    if send.write_all(&secret).await.is_err() {
                        return;
                    }
                    let (mut tcp_r, mut tcp_w) = tcp.into_split();
                    let _ = tokio::try_join!(
                        async {
                            let res = tokio::io::copy(&mut tcp_r, &mut send).await;
                            let _ = send.finish();
                            res
                        },
                        async {
                            let res = tokio::io::copy(&mut recv, &mut tcp_w).await;
                            let _ = tcp_w.shutdown().await;
                            res
                        }
                    );
                });
            }
            running_clone.store(false, Ordering::Relaxed);
        });

        let status = P2pStatus {
            active: true,
            mode: "client".to_string(),
            room_code: Some(room_code.trim().to_string()),
            local_address: Some(local_address.clone()),
            connected_peers: 1,
        };

        if let Ok(mut guard) = self.inner.lock() {
            *guard = Some(ActiveP2p {
                mode: "client".to_string(),
                room_code: Some(room_code.trim().to_string()),
                local_address: Some(local_address),
                peers,
                running,
                task,
                endpoint,
            });
        }

        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    #[test]
    fn room_ticket_roundtrip_and_reject_invalid() {
        let key = SecretKey::generate();
        let addr = EndpointAddr::new(key.public());
        let secret = [7u8; 16];
        let code = encode_room_ticket(&addr, secret).unwrap();
        assert!(code.starts_with("vmm-p2p:"));
        let decoded = decode_room_ticket(&code).unwrap();
        assert_eq!(decoded.secret, secret);
        assert_eq!(decoded.addr.id, addr.id);

        assert!(decode_room_ticket("invalid-code").is_err());
        assert!(decode_room_ticket("vmm-p2p:!!!").is_err());

        let oversized = format!("vmm-p2p:{}", "A".repeat(MAX_ROOM_CODE_LEN + 1));
        assert!(decode_room_ticket(&oversized).is_err());
    }

    #[tokio::test]
    async fn start_host_rejects_privileged_ports() {
        let mgr = P2pManager::new();
        assert!(mgr.start_host(0).await.is_err());
        assert!(mgr.start_host(80).await.is_err());
        assert!(mgr.start_host(1023).await.is_err());
    }
}
