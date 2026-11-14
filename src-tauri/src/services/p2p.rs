use crate::models::P2pStatus;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use iroh::{endpoint::presets, Endpoint, EndpointAddr};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::async_runtime::JoinHandle;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

const ROOM_PREFIX: &str = "vmm-p2p:";
const ALPN: &[u8] = b"vmm-vs-p2p/1";

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
}

impl P2pManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
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

    pub fn stop(&self) -> P2pStatus {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(active) = guard.take() {
                active.running.store(false, Ordering::Relaxed);
                active.task.abort();
                let ep = active.endpoint;
                tauri::async_runtime::spawn(async move {
                    ep.close().await;
                });
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

    pub async fn start_host(&self, target_port: u16) -> Result<P2pStatus, String> {
        self.stop();

        let endpoint = Endpoint::builder(presets::N0)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await
            .map_err(|e| format!("Failed to start P2P endpoint: {e}"))?;
        endpoint.online().await;

        let addr = endpoint.addr();
        let mut secret = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut secret);
        let room_code = encode_room_ticket(&addr, secret)?;

        let peers = Arc::new(AtomicUsize::new(0));
        let peers_clone = Arc::clone(&peers);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let ep_clone = endpoint.clone();

        let task = tauri::async_runtime::spawn(async move {
            while let Some(incoming) = ep_clone.accept().await {
                let peers_conn = Arc::clone(&peers_clone);
                tauri::async_runtime::spawn(async move {
                    let Ok(conn) = incoming.await else {
                        return;
                    };
                    peers_conn.fetch_add(1, Ordering::Relaxed);
                    while let Ok((mut send, mut recv)) = conn.accept_bi().await {
                        tauri::async_runtime::spawn(async move {
                            let mut token = [0u8; 16];
                            if recv.read_exact(&mut token).await.is_err() || token != secret {
                                let _ = send.finish();
                                return;
                            }
                            let Ok(tcp) = TcpStream::connect(("127.0.0.1", target_port)).await else {
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
                    peers_conn.fetch_sub(1, Ordering::Relaxed);
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
        self.stop();

        let endpoint = Endpoint::bind(presets::N0)
            .await
            .map_err(|e| format!("Failed to start P2P client endpoint: {e}"))?;

        let conn = endpoint
            .connect(ticket.addr.clone(), ALPN)
            .await
            .map_err(|e| format!("Could not connect to host P2P room: {e}"))?;

        let listener = match TcpListener::bind(("127.0.0.1", preferred_port)).await {
            Ok(l) => l,
            Err(_) => TcpListener::bind(("127.0.0.1", 0))
                .await
                .map_err(|e| format!("Failed to bind local P2P proxy socket: {e}"))?,
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

        let task = tauri::async_runtime::spawn(async move {
            let mut active_conn = conn;
            while let Ok((tcp, _)) = listener.accept().await {
                if active_conn.close_reason().is_some() {
                    if let Ok(reconnected) = ep_clone.connect(remote_addr.clone(), ALPN).await {
                        active_conn = reconnected;
                    } else {
                        peers_clone.store(0, Ordering::Relaxed);
                        continue;
                    }
                }
                let conn_ref = active_conn.clone();
                tauri::async_runtime::spawn(async move {
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
    }
}
