use crate::models::{ServerConfigSummary, ServerLogEvent, ServerStatus};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const MAX_LOG_LINES: usize = 400;

pub fn detect_server_exe(game_exe_path: Option<&str>) -> Option<String> {
    if let Some(client_exe) = game_exe_path {
        let p = Path::new(client_exe);
        if let Some(parent) = p.parent() {
            let sibling = parent.join("VintagestoryServer.exe");
            if sibling.is_file() {
                return Some(sibling.to_string_lossy().to_string());
            }
        }
    }

    if let Some(roaming) = dirs::data_dir() {
        let candidate = roaming.join("Vintagestory").join("VintagestoryServer.exe");
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }

    let fallbacks = [
        PathBuf::from(r"C:\Program Files\Vintagestory\VintagestoryServer.exe"),
        PathBuf::from(r"C:\Program Files (x86)\Vintagestory\VintagestoryServer.exe"),
    ];
    for c in fallbacks {
        if c.is_file() {
            return Some(c.to_string_lossy().to_string());
        }
    }

    None
}

pub fn default_server_data_path(client_data_path: &Path) -> PathBuf {
    client_data_path.join("VMM_Server")
}

pub fn load_server_config(server_data_path: &Path) -> Result<ServerConfigSummary, String> {
    let cfg_path = server_data_path.join("serverconfig.json");
    if !cfg_path.is_file() {
        return Ok(ServerConfigSummary {
            server_name: "Vintage Story Server".to_string(),
            port: 42420,
            max_clients: 16,
            password: String::new(),
            allow_pvp: true,
            whitelist_mode: 0,
        });
    }

    let raw = fs::read_to_string(&cfg_path)
        .map_err(|e| format!("Failed to read serverconfig.json: {e}"))?;
    let val: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse serverconfig.json: {e}"))?;

    Ok(ServerConfigSummary {
        server_name: val
            .get("ServerName")
            .and_then(|v| v.as_str())
            .unwrap_or("Vintage Story Server")
            .to_string(),
        port: val
            .get("Port")
            .and_then(|v| v.as_u64())
            .and_then(|n| u16::try_from(n).ok())
            .unwrap_or(42420),
        max_clients: val
            .get("MaxClients")
            .and_then(|v| v.as_u64())
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(16),
        password: val
            .get("Password")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        allow_pvp: val
            .get("AllowPvP")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        whitelist_mode: val
            .get("WhitelistMode")
            .and_then(|v| v.as_u64())
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
    })
}

pub fn ensure_server_config_exists(
    server_data_path: &Path,
    exe_path: Option<&str>,
) -> Result<(), String> {
    let cfg_path = server_data_path.join("serverconfig.json");
    let needs_gen = if !cfg_path.is_file() {
        true
    } else {
        fs::read_to_string(&cfg_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .map(|val| {
                val.get("Roles")
                    .and_then(|r| r.as_array())
                    .map(|a| a.is_empty())
                    .unwrap_or(true)
            })
            .unwrap_or(true)
    };

    if !needs_gen {
        return Ok(());
    }

    let Some(exe) = exe_path.filter(|s| Path::new(s).is_file()) else {
        return Ok(());
    };

    fs::create_dir_all(server_data_path)
        .map_err(|e| format!("Failed to create server data directory: {e}"))?;

    let mut cmd = Command::new(exe);
    if let Some(parent) = Path::new(exe).parent() {
        cmd.current_dir(parent);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let out = cmd
        .arg("--dataPath")
        .arg(server_data_path)
        .arg("--genconfig")
        .output()
        .map_err(|e| format!("Failed to generate serverconfig.json: {e}"))?;

    if !out.status.success() && !cfg_path.is_file() {
        return Err("VintagestoryServer.exe --genconfig failed to create serverconfig.json".to_string());
    }

    Ok(())
}

pub fn save_server_config(
    server_data_path: &Path,
    summary: &ServerConfigSummary,
    exe_path: Option<&str>,
) -> Result<(), String> {
    fs::create_dir_all(server_data_path)
        .map_err(|e| format!("Failed to create server data directory: {e}"))?;
    ensure_server_config_exists(server_data_path, exe_path)?;

    let cfg_path = server_data_path.join("serverconfig.json");

    let mut root: Value = if cfg_path.is_file() {
        let raw = fs::read_to_string(&cfg_path)
            .map_err(|e| format!("Failed to read existing serverconfig.json: {e}"))?;
        serde_json::from_str(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if let Some(obj) = root.as_object_mut() {
        obj.insert("ServerName".to_string(), json!(summary.server_name));
        obj.insert("Port".to_string(), json!(summary.port));
        obj.insert("MaxClients".to_string(), json!(summary.max_clients));
        if summary.password.is_empty() {
            obj.insert("Password".to_string(), Value::Null);
        } else {
            obj.insert("Password".to_string(), json!(summary.password));
        }
        obj.insert("AllowPvP".to_string(), json!(summary.allow_pvp));
        obj.insert("WhitelistMode".to_string(), json!(summary.whitelist_mode));
    }

    let formatted = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize serverconfig.json: {e}"))?;
    fs::write(&cfg_path, formatted)
        .map_err(|e| format!("Failed to write serverconfig.json: {e}"))
}

pub fn load_server_config_raw(
    server_data_path: &Path,
    exe_path: Option<&str>,
) -> Result<String, String> {
    fs::create_dir_all(server_data_path)
        .map_err(|e| format!("Failed to create server data directory: {e}"))?;
    ensure_server_config_exists(server_data_path, exe_path)?;

    let cfg_path = server_data_path.join("serverconfig.json");
    if !cfg_path.is_file() {
        let default_cfg = json!({
            "ServerName": "Vintage Story Server",
            "Port": 42420,
            "MaxClients": 16,
            "Password": null,
            "AllowPvP": true,
            "WhitelistMode": 0
        });
        let formatted = serde_json::to_string_pretty(&default_cfg)
            .map_err(|e| format!("Failed to serialize default serverconfig.json: {e}"))?;
        fs::write(&cfg_path, &formatted)
            .map_err(|e| format!("Failed to write serverconfig.json: {e}"))?;
        return Ok(formatted);
    }

    fs::read_to_string(&cfg_path)
        .map_err(|e| format!("Failed to read serverconfig.json: {e}"))
}

pub fn save_server_config_raw(server_data_path: &Path, raw: &str) -> Result<(), String> {
    let val: Value = serde_json::from_str(raw)
        .map_err(|e| format!("Invalid JSON in serverconfig.json: {e}"))?;
    let formatted = serde_json::to_string_pretty(&val)
        .map_err(|e| format!("Failed to format serverconfig.json: {e}"))?;
    fs::create_dir_all(server_data_path)
        .map_err(|e| format!("Failed to create server data directory: {e}"))?;
    let cfg_path = server_data_path.join("serverconfig.json");
    fs::write(&cfg_path, formatted)
        .map_err(|e| format!("Failed to write serverconfig.json: {e}"))
}

struct RunningServer {
    child: Child,
    stdin: Option<ChildStdin>,
    port: u16,
    exe_path: String,
    data_path: String,
}

pub struct ServerManager {
    inner: Mutex<Option<RunningServer>>,
    logs: Arc<Mutex<VecDeque<ServerLogEvent>>>,
}

impl ServerManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            logs: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_LINES))),
        }
    }

    fn push_log(logs: &Arc<Mutex<VecDeque<ServerLogEvent>>>, app: &AppHandle, line: String, is_error: bool) {
        let ev = ServerLogEvent { line, is_error };
        if let Ok(mut guard) = logs.lock() {
            if guard.len() >= MAX_LOG_LINES {
                guard.pop_front();
            }
            guard.push_back(ev.clone());
        }
        let _ = app.emit("server-log", ev);
    }

    pub fn recent_logs(&self) -> Vec<ServerLogEvent> {
        self.logs
            .lock()
            .map(|g| g.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn start(
        &self,
        app: &AppHandle,
        exe_path: &str,
        server_data_path: &Path,
        mods_dir_path: Option<&Path>,
        port: u16,
        max_clients: u32,
    ) -> Result<ServerStatus, String> {
        let exe = Path::new(exe_path);
        if !exe.is_file() {
            return Err(format!("Server executable not found at '{exe_path}'"));
        }

        fs::create_dir_all(server_data_path)
            .map_err(|e| format!("Failed to create server data directory: {e}"))?;

        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Server manager lock poisoned".to_string())?;

        if let Some(existing) = guard.as_mut() {
            if let Ok(None) = existing.child.try_wait() {
                return Ok(ServerStatus {
                    running: true,
                    pid: Some(existing.child.id()),
                    port: existing.port,
                    exe_path: Some(existing.exe_path.clone()),
                    data_path: Some(existing.data_path.clone()),
                });
            }
        }

        if let Ok(mut log_guard) = self.logs.lock() {
            log_guard.clear();
        }

        let mut cmd = Command::new(exe);
        if let Some(parent) = exe.parent() {
            cmd.current_dir(parent);
        }
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        cmd.arg("--dataPath")
            .arg(server_data_path)
            .arg("--port")
            .arg(port.to_string())
            .arg("--maxclients")
            .arg(max_clients.to_string());

        if let Some(mods) = mods_dir_path {
            if mods.is_dir() {
                cmd.arg("--addModPath").arg(mods);
            }
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start VintagestoryServer: {e}"))?;

        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        if let Some(out) = stdout {
            let logs_ref = Arc::clone(&self.logs);
            let app_handle = app.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(out);
                for line in reader.lines().map_while(Result::ok) {
                    Self::push_log(&logs_ref, &app_handle, line, false);
                }
            });
        }

        if let Some(err) = stderr {
            let logs_ref = Arc::clone(&self.logs);
            let app_handle = app.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(err);
                for line in reader.lines().map_while(Result::ok) {
                    Self::push_log(&logs_ref, &app_handle, line, true);
                }
            });
        }

        let data_str = server_data_path.to_string_lossy().to_string();
        *guard = Some(RunningServer {
            child,
            stdin,
            port,
            exe_path: exe_path.to_string(),
            data_path: data_str.clone(),
        });

        Ok(ServerStatus {
            running: true,
            pid: Some(pid),
            port,
            exe_path: Some(exe_path.to_string()),
            data_path: Some(data_str),
        })
    }

    pub fn send_command(&self, command: &str) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Server manager lock poisoned".to_string())?;
        let srv = guard
            .as_mut()
            .ok_or_else(|| "Dedicated server is not running".to_string())?;
        let stdin = srv
            .stdin
            .as_mut()
            .ok_or_else(|| "Server stdin is not available".to_string())?;
        writeln!(stdin, "{}", command.trim())
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("Failed to send command to server: {e}"))
    }

    pub fn stop(&self) -> Result<ServerStatus, String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| "Server manager lock poisoned".to_string())?;
        if let Some(mut srv) = guard.take() {
            if let Some(stdin) = srv.stdin.as_mut() {
                let _ = writeln!(stdin, "/stop");
                let _ = stdin.flush();
            }
            for _ in 0..20 {
                if let Ok(Some(_)) = srv.child.try_wait() {
                    return Ok(ServerStatus {
                        running: false,
                        pid: None,
                        port: srv.port,
                        exe_path: Some(srv.exe_path),
                        data_path: Some(srv.data_path),
                    });
                }
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            let _ = srv.child.kill();
            let _ = srv.child.wait();
            return Ok(ServerStatus {
                running: false,
                pid: None,
                port: srv.port,
                exe_path: Some(srv.exe_path),
                data_path: Some(srv.data_path),
            });
        }
        Ok(ServerStatus {
            running: false,
            pid: None,
            port: 42420,
            exe_path: None,
            data_path: None,
        })
    }

    pub fn status(&self, default_exe: Option<String>, default_data: Option<String>) -> ServerStatus {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(_) => {
                return ServerStatus {
                    running: false,
                    pid: None,
                    port: 42420,
                    exe_path: default_exe,
                    data_path: default_data,
                }
            }
        };

        if let Some(srv) = guard.as_mut() {
            match srv.child.try_wait() {
                Ok(None) => {
                    return ServerStatus {
                        running: true,
                        pid: Some(srv.child.id()),
                        port: srv.port,
                        exe_path: Some(srv.exe_path.clone()),
                        data_path: Some(srv.data_path.clone()),
                    };
                }
                _ => {
                    *guard = None;
                }
            }
        }

        ServerStatus {
            running: false,
            pid: None,
            port: 42420,
            exe_path: default_exe,
            data_path: default_data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_config_preserves_unknown_fields() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join("serverconfig.json");
        fs::write(
            &cfg_path,
            r#"{"ServerName":"Old","Port":42420,"CustomKeepMe":true}"#,
        )
        .unwrap();

        let updated = ServerConfigSummary {
            server_name: "New Coop".to_string(),
            port: 42421,
            max_clients: 8,
            password: "secret".to_string(),
            allow_pvp: false,
            whitelist_mode: 1,
        };
        save_server_config(dir.path(), &updated, None).unwrap();

        let raw = fs::read_to_string(&cfg_path).unwrap();
        let val: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["ServerName"], "New Coop");
        assert_eq!(val["Port"], 42421);
        assert_eq!(val["CustomKeepMe"], true);
    }
}
