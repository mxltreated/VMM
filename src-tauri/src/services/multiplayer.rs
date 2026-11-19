use crate::models::GameStatus;
use crate::services::launcher::GameLauncher;

pub fn validate_server_address(address: &str) -> Result<&str, String> {
    let trimmed = address.trim();
    if trimmed.is_empty() {
        return Err("Server address cannot be empty".to_string());
    }
    if trimmed.len() > 256 {
        return Err("Server address is too long".to_string());
    }
    if trimmed.starts_with('-') {
        return Err("Server address cannot start with '-'".to_string());
    }
    if trimmed
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\''))
    {
        return Err("Server address contains invalid characters".to_string());
    }
    Ok(trimmed)
}

pub fn join_server(
    launcher: &GameLauncher,
    exe_path: &str,
    data_path: Option<&str>,
    address: &str,
    extra_launch_args: Option<&str>,
    max_ram_mb: Option<u32>,
) -> Result<GameStatus, String> {
    let valid_addr = validate_server_address(address)?;
    launcher.launch(exe_path, data_path, extra_launch_args, max_ram_mb, Some(valid_addr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_server_addresses_against_arg_injection() {
        assert_eq!(
            validate_server_address("127.0.0.1:42420").unwrap(),
            "127.0.0.1:42420"
        );
        assert_eq!(
            validate_server_address("play.example.com:42420").unwrap(),
            "play.example.com:42420"
        );
        assert!(validate_server_address("").is_err());
        assert!(validate_server_address("127.0.0.1\" --openWorld x").is_err());
        assert!(validate_server_address("--evil-flag").is_err());
        assert!(validate_server_address("host name").is_err());
        assert!(validate_server_address("host\n--flag").is_err());
    }
}
