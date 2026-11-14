use crate::models::GameStatus;
use crate::services::launcher::GameLauncher;

pub fn join_server(
    launcher: &GameLauncher,
    exe_path: &str,
    data_path: Option<&str>,
    address: &str,
    extra_launch_args: Option<&str>,
    max_ram_mb: Option<u32>,
) -> Result<GameStatus, String> {
    let trimmed = address.trim();
    if trimmed.is_empty() {
        return Err("Server address cannot be empty".to_string());
    }
    let combined_args = match extra_launch_args {
        Some(existing) if !existing.trim().is_empty() => {
            format!("{} --connect \"{trimmed}\"", existing.trim())
        }
        _ => format!("--connect \"{trimmed}\""),
    };
    launcher.launch(exe_path, data_path, Some(&combined_args), max_ram_mb)
}
