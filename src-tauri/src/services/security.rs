use rand::RngCore;
use reqwest::redirect::Policy;
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use subtle::ConstantTimeEq;
use url::{Host, Url};

pub fn write_file_atomic(path: &Path, content: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {e}", parent.display()))?;
        }
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let pid = std::process::id();

    let mut rand_bytes = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut rand_bytes);
    let rand_hex = hex::encode(rand_bytes);

    let tmp_name = format!(".{file_name}.{pid}.{rand_hex}.tmp");
    let tmp_path = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(tmp_name),
        _ => PathBuf::from(tmp_name),
    };

    let write_res = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .map_err(|e| format!("Failed to create temp file {}: {e}", tmp_path.display()))?;
        file.write_all(content)
            .map_err(|e| format!("Failed to write temp file {}: {e}", tmp_path.display()))?;
        file.sync_all()
            .map_err(|e| format!("Failed to sync temp file {}: {e}", tmp_path.display()))?;
        drop(file);
        fs::rename(&tmp_path, path)
            .map_err(|e| format!("Failed to finalize {}: {e}", path.display()))?;
        Ok(())
    })();

    if write_res.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    write_res
}

pub fn is_windows_reserved_name(name: &str) -> bool {
    if name.is_empty() || name.ends_with(' ') || name.ends_with('.') {
        return true;
    }
    let stem = name.split('.').next().unwrap_or("");
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

pub fn validate_mod_filename(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || name != trimmed {
        return Err("Invalid mod filename".to_string());
    }
    if trimmed.contains('\0') || trimmed.contains('/') || trimmed.contains('\\') {
        return Err("Mod filename cannot contain path separators".to_string());
    }
    if is_windows_reserved_name(trimmed) {
        return Err("Mod filename uses a reserved system name".to_string());
    }

    let path = Path::new(trimmed);
    let mut comps = path.components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(_)), None) => {}
        _ => return Err("Mod filename must be a plain file name".to_string()),
    }

    let is_zip = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);
    if !is_zip {
        return Err("Mod filename must end with .zip".to_string());
    }

    Ok(trimmed.to_string())
}

pub fn ensure_no_symlink_escape(root: &Path, target: &Path) -> Result<(), String> {
    let canonical_root = fs::canonicalize(root)
        .map_err(|e| format!("Failed to resolve base directory {}: {e}", root.display()))?;

    let mut cur: Option<&Path> = Some(target);
    let mut deepest_existing: Option<PathBuf> = None;

    while let Some(p) = cur {
        if p == root {
            break;
        }
        match fs::symlink_metadata(p) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(format!("Symlinks are not allowed: {}", p.display()));
                }
                if deepest_existing.is_none() {
                    deepest_existing = Some(p.to_path_buf());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!("Failed to inspect path {}: {e}", p.display()));
            }
        }
        cur = p.parent();
    }

    let check_path = deepest_existing.unwrap_or_else(|| root.to_path_buf());
    let canonical_target = fs::canonicalize(&check_path)
        .map_err(|e| format!("Failed to resolve path {}: {e}", check_path.display()))?;

    if !canonical_target.starts_with(&canonical_root) {
        return Err("Path escapes target directory".to_string());
    }

    Ok(())
}

pub fn validate_exe_path(exe_path: &str, allowed_basenames: &[&str]) -> Result<PathBuf, String> {
    let trimmed = exe_path.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err("Executable path is invalid".to_string());
    }
    let path = PathBuf::from(trimmed);
    if !path.is_file() {
        return Err(format!("Executable not found at '{}'", trimmed));
    }
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .ok_or_else(|| "Executable filename is invalid".to_string())?;

    if !allowed_basenames
        .iter()
        .any(|allowed| file_name == allowed.to_ascii_lowercase())
    {
        return Err(format!(
            "Executable '{}' is not an expected Vintage Story binary",
            file_name
        ));
    }

    Ok(path)
}

pub fn validate_external_http_url(raw: &str) -> Result<Url, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("URL cannot be empty".to_string());
    }
    let parsed = Url::parse(trimmed).map_err(|e| format!("Invalid URL: {e}"))?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err("Only http:// and https:// URLs are allowed".to_string());
    }
    if parsed.host_str().is_none() {
        return Err("URL is missing a host".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("URLs with embedded credentials are not allowed".to_string());
    }
    Ok(parsed)
}

pub fn validate_https_url(raw: &str) -> Result<Url, String> {
    let parsed = validate_external_http_url(raw)?;
    if parsed.scheme() != "https" {
        return Err("Only https:// URLs are allowed".to_string());
    }
    Ok(parsed)
}

fn is_disallowed_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    let is_cgnat = octets[0] == 100 && (octets[1] & 0b1100_0000) == 64;
    let is_ietf_prot = octets[0] == 192 && octets[1] == 0 && octets[2] == 0;
    let is_benchmark = octets[0] == 198 && (octets[1] & 0b1111_1110) == 18;
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || is_cgnat
        || is_ietf_prot
        || is_benchmark
}

fn is_disallowed_ipv6(ip: Ipv6Addr) -> bool {
    let seg0 = ip.segments()[0];
    let is_unique_local = (seg0 & 0xfe00) == 0xfc00;
    let is_link_local = (seg0 & 0xffc0) == 0xfe80;
    if ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || is_unique_local
        || is_link_local
    {
        return true;
    }
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_disallowed_ipv4(v4);
    }
    if let Some(v4) = ip.to_ipv4() {
        return is_disallowed_ipv4(v4);
    }
    false
}

pub fn is_private_or_loopback_host(url: &Url) -> bool {
    let Some(host) = url.host() else {
        return true;
    };
    match host {
        Host::Domain(domain) => {
            let d = domain.trim_end_matches('.').to_ascii_lowercase();
            if d.is_empty()
                || d == "localhost"
                || d.ends_with(".localhost")
                || d.ends_with(".local")
                || d.ends_with(".internal")
            {
                return true;
            }
            if let Ok(ip) = d.parse::<IpAddr>() {
                return match ip {
                    IpAddr::V4(v4) => is_disallowed_ipv4(v4),
                    IpAddr::V6(v6) => is_disallowed_ipv6(v6),
                };
            }
            false
        }
        Host::Ipv4(v4) => is_disallowed_ipv4(v4),
        Host::Ipv6(v6) => is_disallowed_ipv6(v6),
    }
}

pub fn is_allowed_moddb_host(url: &Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    h == "mods.vintagestory.at"
        || h == "moddbcdn.vintagestory.at"
        || h.ends_with(".vintagestory.at")
}

pub fn validate_moddb_download_url(raw: &str) -> Result<Url, String> {
    let parsed = validate_https_url(raw)?;
    if !is_allowed_moddb_host(&parsed) {
        return Err("Mod download URL must originate from *.vintagestory.at".to_string());
    }
    Ok(parsed)
}

pub fn is_allowed_github_download_host(url: &Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    matches!(
        h.as_str(),
        "github.com" | "objects.githubusercontent.com" | "release-assets.githubusercontent.com"
    )
}

pub fn validate_github_release_url(raw: &str, expected_repo: &str) -> Result<Url, String> {
    let parsed = validate_https_url(raw)?;
    let host = parsed
        .host_str()
        .map(|s| s.trim_end_matches('.').to_ascii_lowercase())
        .unwrap_or_default();
    if host != "github.com" {
        return Err("Release asset URL must be hosted on github.com".to_string());
    }
    if parsed
        .path_segments()
        .map(|mut segs| segs.any(|s| s == ".."))
        .unwrap_or(false)
    {
        return Err("Release asset URL contains invalid path segments".to_string());
    }
    let expected_prefix = format!("/{}/releases/download/", expected_repo.to_ascii_lowercase());
    if !parsed.path().to_ascii_lowercase().starts_with(&expected_prefix) {
        return Err("Release asset URL does not match the official repository".to_string());
    }
    Ok(parsed)
}

pub fn checked_redirect_policy(max_hops: usize, allow_fn: fn(&Url) -> bool) -> Policy {
    Policy::custom(move |attempt| {
        if attempt.previous().len() >= max_hops {
            return attempt.error("Too many HTTP redirects");
        }
        if attempt.url().scheme() != "https" || !allow_fn(attempt.url()) {
            return attempt.error("Blocked redirect to untrusted or insecure URL");
        }
        attempt.follow()
    })
}

pub fn build_hardened_http_client(
    user_agent: &str,
    timeout_secs: u64,
    redirect_policy: Policy,
) -> Result<Client, String> {
    Client::builder()
        .user_agent(user_agent)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(timeout_secs))
        .https_only(true)
        .redirect(redirect_policy)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

pub fn read_bounded<R: Read>(
    reader: &mut R,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, String> {
    let limit = (max_bytes as u64).saturating_add(1);
    let mut buf = Vec::new();
    reader
        .take(limit)
        .read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read {label}: {e}"))?;
    if buf.len() > max_bytes {
        return Err(format!(
            "{label} exceeds maximum allowed size ({max_bytes} bytes)"
        ));
    }
    Ok(buf)
}

pub fn is_zip_symlink(unix_mode: Option<u32>) -> bool {
    unix_mode.map(|m| (m & 0o170000) == 0o120000).unwrap_or(false)
}

#[allow(dead_code)]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn ct_eq_hex(a: &str, b: &str) -> bool {
    let a_norm = a.trim().to_ascii_lowercase();
    let b_norm = b.trim().to_ascii_lowercase();
    if a_norm.is_empty() || a_norm.len() != b_norm.len() {
        return false;
    }
    bool::from(a_norm.as_bytes().ct_eq(b_norm.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn mod_filename_validation_blocks_traversal_and_reserved() {
        assert!(validate_mod_filename("carryon_1.0.0.zip").is_ok());
        assert!(validate_mod_filename("../evil.zip").is_err());
        assert!(validate_mod_filename("sub/mod.zip").is_err());
        assert!(validate_mod_filename(r"sub\mod.zip").is_err());
        assert!(validate_mod_filename("C:\\mod.zip").is_err());
        assert!(validate_mod_filename("CON.zip").is_err());
        assert!(validate_mod_filename("nul.zip").is_err());
        assert!(validate_mod_filename("mod.exe").is_err());
        assert!(validate_mod_filename(" mod.zip ").is_err());
        assert!(validate_mod_filename("mod.zip\0.txt").is_err());
    }

    #[test]
    fn url_and_ssrf_checks_block_private_and_untrusted_hosts() {
        assert!(validate_external_http_url("https://github.com/mxltreated/VMM").is_ok());
        assert!(validate_external_http_url("file:///C:/Windows/win.ini").is_err());
        assert!(validate_external_http_url("javascript:alert(1)").is_err());
        assert!(validate_external_http_url("https://user:pass@example.com").is_err());

        assert!(validate_moddb_download_url("https://moddbcdn.vintagestory.at/a.zip").is_ok());
        assert!(validate_moddb_download_url("http://moddbcdn.vintagestory.at/a.zip").is_err());
        assert!(validate_moddb_download_url("https://evil.example.com/a.zip").is_err());

        assert!(is_private_or_loopback_host(&Url::parse("https://127.0.0.1/x").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("https://localhost/x").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("https://169.254.169.254/latest").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("https://[::1]/x").unwrap()));
        assert!(is_private_or_loopback_host(&Url::parse("https://[::ffff:127.0.0.1]/x").unwrap()));
        assert!(!is_private_or_loopback_host(&Url::parse("https://raw.githubusercontent.com/a/b").unwrap()));

        assert!(validate_github_release_url(
            "https://github.com/mxltreated/VMM/releases/download/v1.2.0/VMM-windows-x64.zip",
            "mxltreated/VMM"
        )
        .is_ok());
        assert!(validate_github_release_url(
            "https://github.com/other/repo/releases/download/v1.2.0/VMM-windows-x64.zip",
            "mxltreated/VMM"
        )
        .is_err());
    }

    #[test]
    fn read_bounded_and_atomic_write_work() {
        let mut cursor = std::io::Cursor::new(b"hello world");
        let ok = read_bounded(&mut cursor, 11, "test").unwrap();
        assert_eq!(ok, b"hello world");

        let mut cursor2 = std::io::Cursor::new(b"hello world!");
        assert!(read_bounded(&mut cursor2, 11, "test").is_err());

        let dir = tempdir().unwrap();
        let file = dir.path().join("nested").join("cfg.json");
        write_file_atomic(&file, br#"{"a":1}"#).unwrap();
        write_file_atomic(&file, br#"{"a":2}"#).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), r#"{"a":2}"#);

        assert!(ct_eq_hex(
            &sha256_hex(b"abc"),
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        ));
        assert!(!ct_eq_hex(&sha256_hex(b"abc"), &sha256_hex(b"abd")));
    }

    #[test]
    fn exe_validation_checks_basename() {
        let dir = tempdir().unwrap();
        let good = dir.path().join("Vintagestory.exe");
        let bad = dir.path().join("cmd.exe");
        fs::write(&good, b"MZ").unwrap();
        fs::write(&bad, b"MZ").unwrap();

        assert!(validate_exe_path(good.to_str().unwrap(), &["vintagestory.exe"]).is_ok());
        assert!(validate_exe_path(bad.to_str().unwrap(), &["vintagestory.exe"]).is_err());
    }
}
