use crate::models::{FirewallStatus, GameStatus, NetworkEndpoint};
use crate::services::launcher::GameLauncher;
use serde_json::Value;
use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn silent_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

pub fn detect_network_endpoints() -> Vec<NetworkEndpoint> {
    let mut endpoints = Vec::new();
    let mut seen_ips = HashSet::new();

    // 1. Try Tailscale CLI for exact Tailscale IPv4
    if let Ok(out) = silent_command("tailscale").args(["status", "--json"]).output() {
        if out.status.success() {
            if let Ok(json) = serde_json::from_slice::<Value>(&out.stdout) {
                if let Some(ips) = json.pointer("/Self/TailscaleIPs").and_then(|v| v.as_array()) {
                    for ip_val in ips {
                        if let Some(ip_str) = ip_val.as_str() {
                            if ip_str.parse::<Ipv4Addr>().is_ok() && seen_ips.insert(ip_str.to_string()) {
                                endpoints.push(NetworkEndpoint {
                                    kind: "tailscale".to_string(),
                                    label: "Tailscale".to_string(),
                                    ip: ip_str.to_string(),
                                    interface_name: "Tailscale".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Try ZeroTier CLI
    if let Ok(out) = silent_command("zerotier-cli").args(["listnetworks", "-j"]).output() {
        if out.status.success() {
            if let Ok(Value::Array(nets)) = serde_json::from_slice::<Value>(&out.stdout) {
                for net in nets {
                    let name = net
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("ZeroTier")
                        .to_string();
                    if let Some(addrs) = net.get("assignedAddresses").and_then(|v| v.as_array()) {
                        for addr in addrs {
                            if let Some(cidr) = addr.as_str() {
                                let ip = cidr.split('/').next().unwrap_or("");
                                if ip.parse::<Ipv4Addr>().is_ok() && seen_ips.insert(ip.to_string()) {
                                    endpoints.push(NetworkEndpoint {
                                        kind: "zerotier".to_string(),
                                        label: format!("ZeroTier ({name})"),
                                        ip: ip.to_string(),
                                        interface_name: name.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Inspect OS IPv4 adapters (catches Radmin VPN, LAN, and fallback Tailscale/ZeroTier)
    #[cfg(windows)]
    {
        if let Ok(out) = silent_command("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -notlike '127.*' -and $_.IPAddress -notlike '169.254.*' } | Select-Object InterfaceAlias,IPAddress | ConvertTo-Json -Compress",
            ])
            .output()
        {
            if out.status.success() {
                let raw = String::from_utf8_lossy(&out.stdout);
                for ep in parse_powershell_adapters(&raw) {
                    if seen_ips.insert(ep.ip.clone()) {
                        endpoints.push(ep);
                    }
                }
            }
        }
    }

    endpoints
}

pub fn parse_powershell_adapters(raw: &str) -> Vec<NetworkEndpoint> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    let parsed: Result<Value, _> = serde_json::from_str(trimmed);
    let items = match parsed {
        Ok(Value::Array(arr)) => arr,
        Ok(obj @ Value::Object(_)) => vec![obj],
        _ => return vec![],
    };

    let mut out = Vec::new();
    for item in items {
        let alias = item
            .get("InterfaceAlias")
            .and_then(|v| v.as_str())
            .unwrap_or("Network")
            .to_string();
        let ip = item
            .get("IPAddress")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if ip.parse::<Ipv4Addr>().is_err() {
            continue;
        }

        let lower = alias.to_lowercase();
        let (kind, label) = if lower.contains("tailscale") {
            ("tailscale".to_string(), "Tailscale".to_string())
        } else if lower.contains("zerotier") {
            ("zerotier".to_string(), "ZeroTier".to_string())
        } else if lower.contains("radmin") || ip.starts_with("26.") {
            ("radmin".to_string(), "Radmin VPN".to_string())
        } else if lower.contains("vethernet") || lower.contains("wsl") || lower.contains("virtual") {
            continue;
        } else {
            ("lan".to_string(), format!("Local LAN ({alias})"))
        };

        out.push(NetworkEndpoint {
            kind,
            label,
            ip,
            interface_name: alias,
        });
    }
    out
}

pub fn check_firewall_rule(port: u16) -> FirewallStatus {
    #[cfg(windows)]
    {
        let rule_name = format!("Vintage Story Server (VMM {port})");
        let out = silent_command("netsh")
            .args([
                "advfirewall",
                "firewall",
                "show",
                "rule",
                &format!("name={rule_name}"),
            ])
            .output();

        match out {
            Ok(res) if res.status.success() => FirewallStatus {
                rule_present: true,
                port,
                details: format!("Inbound TCP/UDP rule '{rule_name}' is active."),
            },
            _ => FirewallStatus {
                rule_present: false,
                port,
                details: format!("No VMM firewall rule found for TCP/UDP port {port}."),
            },
        }
    }

    #[cfg(not(windows))]
    {
        FirewallStatus {
            rule_present: true,
            port,
            details: "Firewall check is only required on Windows.".to_string(),
        }
    }
}

pub fn add_firewall_rule(port: u16) -> Result<FirewallStatus, String> {
    #[cfg(windows)]
    {
        let rule_name = format!("Vintage Story Server (VMM {port})");
        for proto in ["TCP", "UDP"] {
            let out = silent_command("netsh")
                .args([
                    "advfirewall",
                    "firewall",
                    "add",
                    "rule",
                    &format!("name={rule_name}"),
                    "dir=in",
                    "action=allow",
                    &format!("protocol={proto}"),
                    &format!("localport={port}"),
                ])
                .output()
                .map_err(|e| format!("Failed to run netsh: {e}"))?;

            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stdout).to_string()
                    + &String::from_utf8_lossy(&out.stderr);
                return Err(format!(
                    "Could not add Windows Firewall rule (may require running VMM as Administrator): {}",
                    err.trim()
                ));
            }
        }
        Ok(check_firewall_rule(port))
    }

    #[cfg(not(windows))]
    {
        Ok(check_firewall_rule(port))
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_adapters_array() {
        let json = r#"[
            {"InterfaceAlias":"Ethernet","IPAddress":"192.168.1.42"},
            {"InterfaceAlias":"Radmin VPN","IPAddress":"26.14.88.10"},
            {"InterfaceAlias":"vEthernet (WSL)","IPAddress":"172.25.0.1"}
        ]"#;
        let eps = parse_powershell_adapters(json);
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0].kind, "lan");
        assert_eq!(eps[0].ip, "192.168.1.42");
        assert_eq!(eps[1].kind, "radmin");
        assert_eq!(eps[1].ip, "26.14.88.10");
    }
}
