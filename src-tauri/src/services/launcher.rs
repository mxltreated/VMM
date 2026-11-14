use crate::models::GameStatus;
use std::path::Path;
use std::process::{Child, Command};
use std::sync::Mutex;

pub struct GameLauncher {
    child: Mutex<Option<Child>>,
}

impl GameLauncher {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }

    pub fn launch(
        &self,
        exe_path: &str,
        data_path: Option<&str>,
        launch_args: Option<&str>,
        max_ram_mb: Option<u32>,
    ) -> Result<GameStatus, String> {
        let exe = Path::new(exe_path);
        if !exe.is_file() {
            return Err(format!("Game executable not found at '{}'", exe_path));
        }

        let mut guard = self
            .child
            .lock()
            .map_err(|_| "Launcher lock poisoned".to_string())?;

        if let Some(existing) = guard.as_mut() {
            if let Ok(None) = existing.try_wait() {
                return Ok(GameStatus {
                    running: true,
                    pid: Some(existing.id()),
                });
            }
        }

        let mut cmd = Command::new(exe);
        if let Some(parent) = exe.parent() {
            cmd.current_dir(parent);
        }
        if let Some(dp) = data_path {
            if !dp.trim().is_empty() {
                cmd.arg("--dataPath").arg(dp);
            }
        }
        if let Some(extra) = launch_args {
            for arg in split_args(extra) {
                cmd.arg(arg);
            }
        }
        if let Some(mb) = max_ram_mb {
            if mb > 0 {
                let bytes = (mb as u64) * 1024 * 1024;
                cmd.env("DOTNET_GCHeapHardLimit", format!("{bytes:x}"));
            }
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to launch Vintage Story: {e}"))?;
        let pid = child.id();
        *guard = Some(child);

        Ok(GameStatus {
            running: true,
            pid: Some(pid),
        })
    }

    pub fn status(&self) -> GameStatus {
        let mut guard = match self.child.lock() {
            Ok(g) => g,
            Err(_) => {
                return GameStatus {
                    running: false,
                    pid: None,
                }
            }
        };

        if let Some(existing) = guard.as_mut() {
            match existing.try_wait() {
                Ok(None) => {
                    return GameStatus {
                        running: true,
                        pid: Some(existing.id()),
                    };
                }
                _ => {
                    *guard = None;
                }
            }
        }

        GameStatus {
            running: false,
            pid: None,
        }
    }
}

fn split_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in input.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}
