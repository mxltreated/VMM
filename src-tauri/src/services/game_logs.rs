use crate::models::{CrashAnalysisReport, CrashSuspect, GameLogContent, GameLogFile, LocalMod};
use crate::services::security::{ensure_no_symlink_escape, is_windows_reserved_name};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_LOG_DEPTH: usize = 3;
const MAX_LOG_FILES: usize = 300;
const MAX_READ_LOG_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ANALYZE_BYTES: u64 = 2 * 1024 * 1024;

pub fn logs_dir(data_path: &Path) -> PathBuf {
    data_path.join("Logs")
}

fn is_allowed_log_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("log") || ext.eq_ignore_ascii_case("txt"))
        .unwrap_or(false)
}

fn classify_log(rel_path: &str) -> (bool, String) {
    let lower = rel_path.to_lowercase();
    let is_crash = lower.contains("crash");
    let category = if is_crash {
        "crash"
    } else if lower.contains("client") {
        "client"
    } else if lower.contains("server") {
        "server"
    } else {
        "other"
    };
    (is_crash, category.to_string())
}

fn resolve_safe_log_path(data_path: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let trimmed = rel_path.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err("Log path cannot be empty".to_string());
    }
    let rel = Path::new(trimmed);
    if rel.is_absolute() {
        return Err("Log path must be relative".to_string());
    }
    let mut has_normal = false;
    for comp in rel.components() {
        match comp {
            Component::Normal(part) => {
                has_normal = true;
                let s = part
                    .to_str()
                    .ok_or_else(|| "Invalid UTF-8 in log path".to_string())?;
                if is_windows_reserved_name(s) {
                    return Err("Log path uses a reserved system name".to_string());
                }
            }
            _ => return Err("Invalid log path".to_string()),
        }
    }
    if !has_normal {
        return Err("Invalid log path".to_string());
    }
    if !is_allowed_log_ext(rel) {
        return Err("Log file must have a .log or .txt extension".to_string());
    }
    Ok(logs_dir(data_path).join(rel))
}

pub fn list_game_logs(data_path: &Path) -> Result<Vec<GameLogFile>, String> {
    let root = logs_dir(data_path);
    if !root.is_dir() {
        return Ok(vec![]);
    }

    let mut out = Vec::new();
    collect_log_files(&root, &root, 0, &mut out)?;
    out.sort_by(|a, b| {
        b.modified_ms
            .cmp(&a.modified_ms)
            .then_with(|| b.is_crash.cmp(&a.is_crash))
            .then_with(|| a.rel_path.to_lowercase().cmp(&b.rel_path.to_lowercase()))
    });
    Ok(out)
}

fn collect_log_files(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<GameLogFile>,
) -> Result<(), String> {
    if depth > MAX_LOG_DEPTH || out.len() >= MAX_LOG_FILES {
        return Ok(());
    }

    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read Logs directory {}: {e}", dir.display()))?;

    for entry in entries.flatten() {
        if out.len() >= MAX_LOG_FILES {
            break;
        }
        let path = entry.path();
        let Ok(sym_meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if sym_meta.file_type().is_symlink() {
            continue;
        }
        if sym_meta.is_dir() {
            collect_log_files(root, &path, depth + 1, out)?;
            continue;
        }
        if !sym_meta.is_file() || !is_allowed_log_ext(&path) {
            continue;
        }
        if ensure_no_symlink_escape(root, &path).is_err() {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let rel_path = rel.to_string_lossy().replace('\\', "/");
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&rel_path)
            .to_string();
        let modified_ms = sym_meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let (is_crash, category) = classify_log(&rel_path);

        out.push(GameLogFile {
            filename,
            rel_path,
            size_bytes: sym_meta.len(),
            modified_ms,
            is_crash,
            category,
        });
    }

    Ok(())
}

fn read_tail_utf8(path: &Path, max_bytes: u64) -> Result<(String, u64, bool), String> {
    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open log file {}: {e}", path.display()))?;
    let total_size = file
        .metadata()
        .map_err(|e| format!("Failed to stat log file: {e}"))?
        .len();

    let truncated = total_size > max_bytes;
    if truncated {
        file.seek(SeekFrom::End(-(max_bytes as i64)))
            .map_err(|e| format!("Failed to seek log file: {e}"))?;
    }

    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read log file: {e}"))?;

    let mut text = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        if let Some(pos) = text.find('\n') {
            text = text[pos + 1..].to_string();
        }
    }

    Ok((text, total_size, truncated))
}

pub fn read_game_log(data_path: &Path, rel_path: &str) -> Result<GameLogContent, String> {
    let root = logs_dir(data_path);
    let path = resolve_safe_log_path(data_path, rel_path)?;
    ensure_no_symlink_escape(&root, &path)?;
    if !path.is_file() {
        return Err(format!("Log file not found: {rel_path}"));
    }

    let (content, size_bytes, truncated) = read_tail_utf8(&path, MAX_READ_LOG_BYTES)?;
    let line_count = content.lines().count();
    Ok(GameLogContent {
        rel_path: rel_path.replace('\\', "/"),
        content,
        size_bytes,
        truncated,
        line_count,
    })
}

struct ModMatchInfo<'a> {
    mod_ref: &'a LocalMod,
    modid_lower: String,
    compact_id: String,
    compact_name: String,
    compact_stem: String,
}

#[derive(Default)]
struct SuspectAccumulator {
    modid: String,
    mod_name: String,
    filename: Option<String>,
    enabled: Option<bool>,
    score: i32,
    reasons: Vec<String>,
    sample_lines: Vec<String>,
}

impl SuspectAccumulator {
    fn add_reason(&mut self, reason: String) {
        if !self.reasons.contains(&reason) && self.reasons.len() < 4 {
            self.reasons.push(reason);
        }
    }

    fn add_sample(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() || self.sample_lines.len() >= 4 {
            return;
        }
        let clipped = if trimmed.len() > 220 {
            format!("{}...", &trimmed[..217])
        } else {
            trimmed.to_string()
        };
        if !self.sample_lines.contains(&clipped) {
            self.sample_lines.push(clipped);
        }
    }
}

fn compact_alnum(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn is_builtin_tag(tag: &str) -> bool {
    matches!(
        tag,
        "error"
            | "fatal"
            | "warning"
            | "notification"
            | "event"
            | "debug"
            | "verbose"
            | "audit"
            | "chat"
            | "build"
            | "worldgen"
            | "game"
            | "survival"
            | "creative"
            | "vintagestory"
            | "client"
            | "server"
            | "core"
            | "dotnet"
    )
}

/// Extract `[tag]` brackets from a Vintage Story log line, e.g. `21.11.2026 [Error] [combatoverhaul] ...`.
fn extract_bracket_tags(line: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('[') {
        let after = &rest[start + 1..];
        let Some(end) = after.find(']') else {
            break;
        };
        let tag = after[..end].trim().to_lowercase();
        if !tag.is_empty() && tag.len() <= 64 && !tag.contains(' ') {
            tags.push(tag);
        }
        rest = &after[end + 1..];
    }
    tags
}

pub fn analyze_crash_logs(
    data_path: &Path,
    local_mods: &[LocalMod],
) -> Result<CrashAnalysisReport, String> {
    let all_logs = list_game_logs(data_path)?;
    if all_logs.is_empty() {
        return Ok(CrashAnalysisReport {
            crash_detected: false,
            headline: "No game logs found yet".to_string(),
            exception_summary: None,
            analyzed_files: vec![],
            suspects: vec![],
            recommendations: vec![
                "Launch Vintage Story at least once to generate logs in VintagestoryData/Logs."
                    .to_string(),
            ],
        });
    }

    // Prioritize crash logs and main client/server logs (up to 6 most recent relevant files).
    let mut target_logs: Vec<GameLogFile> = all_logs
        .iter()
        .filter(|l| {
            let low = l.rel_path.to_lowercase();
            l.is_crash
                || low.ends_with("client-main.log")
                || low.ends_with("client-main.txt")
                || low.ends_with("server-main.log")
                || low.ends_with("server-main.txt")
        })
        .cloned()
        .collect();

    target_logs.sort_by(|a, b| {
        b.is_crash
            .cmp(&a.is_crash)
            .then_with(|| b.modified_ms.cmp(&a.modified_ms))
    });
    target_logs.truncate(6);

    if target_logs.is_empty() {
        target_logs = all_logs.into_iter().take(4).collect();
    }

    let mod_infos: Vec<ModMatchInfo> = local_mods
        .iter()
        .filter(|m| !m.modid.trim().is_empty())
        .map(|m| {
            let stem = Path::new(&m.filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            ModMatchInfo {
                mod_ref: m,
                modid_lower: m.modid.trim().to_lowercase(),
                compact_id: compact_alnum(&m.modid),
                compact_name: compact_alnum(&m.name),
                compact_stem: compact_alnum(stem),
            }
        })
        .collect();

    let mut suspects: HashMap<String, SuspectAccumulator> = HashMap::new();
    let mut analyzed_files = Vec::new();
    let mut crash_detected = false;
    let mut exception_summary: Option<String> = None;

    let mut saw_missing_method = false;
    let mut saw_missing_dep = false;
    let mut saw_harmony = false;
    let mut saw_asset_error = false;
    let mut saw_oom_or_gl = false;
    let mut error_line_count = 0usize;

    let root = logs_dir(data_path);

    for log_file in &target_logs {
        let Ok(path) = resolve_safe_log_path(data_path, &log_file.rel_path) else {
            continue;
        };
        if ensure_no_symlink_escape(&root, &path).is_err() || !path.is_file() {
            continue;
        }
        let Ok((content, _, _)) = read_tail_utf8(&path, MAX_ANALYZE_BYTES) else {
            continue;
        };
        if content.trim().is_empty() {
            continue;
        }

        analyzed_files.push(log_file.rel_path.clone());
        if log_file.is_crash {
            crash_detected = true;
        }

        let file_weight = if log_file.is_crash { 3 } else { 1 };

        // Keep a small context window of recent error/exception lines for stack trace attribution.
        let mut in_exception_block = false;
        let mut block_lines_remaining = 0usize;

        for line in content.lines() {
            let lower = line.to_lowercase();
            let is_fatal = lower.contains("[fatal]") || lower.contains("unhandled exception");
            let is_error = is_fatal
                || lower.contains("[error]")
                || lower.contains("exception:")
                || lower.contains("failed to load mod")
                || lower.contains("could not load file or assembly");
            let is_stack_frame = line.trim_start().starts_with("at ");

            if is_fatal {
                crash_detected = true;
            }
            if is_error {
                error_line_count += 1;
                in_exception_block = true;
                block_lines_remaining = 25;
                if exception_summary.is_none()
                    && (lower.contains("exception") || is_fatal || lower.contains("failed to load"))
                {
                    let trimmed = line.trim();
                    exception_summary = Some(if trimmed.len() > 240 {
                        format!("{}...", &trimmed[..237])
                    } else {
                        trimmed.to_string()
                    });
                }
            } else if in_exception_block {
                if block_lines_remaining > 0 {
                    block_lines_remaining -= 1;
                } else {
                    in_exception_block = false;
                }
            }

            if lower.contains("missingmethodexception")
                || lower.contains("missingfieldexception")
                || lower.contains("typeloadexception")
                || lower.contains("method not found")
            {
                saw_missing_method = true;
            }
            if lower.contains("missing dependency")
                || lower.contains("could not load file or assembly")
                || lower.contains("requires mod")
            {
                saw_missing_dep = true;
            }
            if lower.contains("harmony") {
                saw_harmony = true;
            }
            if lower.contains("assets/") && is_error {
                saw_asset_error = true;
            }
            if lower.contains("outofmemoryexception")
                || lower.contains("opengl")
                || lower.contains("glerror")
            {
                saw_oom_or_gl = true;
            }

            if !(is_error || (in_exception_block && is_stack_frame) || log_file.is_crash) {
                continue;
            }

            let tags = extract_bracket_tags(line);
            let line_compact = compact_alnum(line);

            // 1. Direct `[modid]` tag attribution on error/fatal lines
            for tag in &tags {
                if is_builtin_tag(tag) {
                    continue;
                }
                let tag_compact = compact_alnum(tag);
                if let Some(info) = mod_infos
                    .iter()
                    .find(|m| m.modid_lower == *tag || m.compact_id == tag_compact)
                {
                    let entry = suspects.entry(info.modid_lower.clone()).or_insert_with(|| {
                        SuspectAccumulator {
                            modid: info.mod_ref.modid.clone(),
                            mod_name: info.mod_ref.name.clone(),
                            filename: Some(info.mod_ref.filename.clone()),
                            enabled: Some(info.mod_ref.enabled),
                            ..Default::default()
                        }
                    });
                    entry.score += if is_fatal { 35 } else { 15 } * file_weight;
                    entry.add_reason(format!(
                        "Logged {} in {}",
                        if is_fatal { "a fatal error" } else { "an error" },
                        log_file.rel_path
                    ));
                    entry.add_sample(line);
                } else if is_error && tag.len() >= 3 {
                    let entry = suspects.entry(tag.clone()).or_insert_with(|| SuspectAccumulator {
                        modid: tag.clone(),
                        mod_name: tag.clone(),
                        filename: None,
                        enabled: None,
                        ..Default::default()
                    });
                    entry.score += 10 * file_weight;
                    entry.add_reason(format!("Logged [{}] error in {}", tag, log_file.rel_path));
                    entry.add_sample(line);
                }
            }

            // 2. Stack trace / assembly / filename / asset path matching against installed mods
            if !line_compact.is_empty() && (is_error || is_stack_frame) {
                for info in &mod_infos {
                    let id_match = info.compact_id.len() >= 5 && line_compact.contains(&info.compact_id);
                    let name_match =
                        info.compact_name.len() >= 6 && line_compact.contains(&info.compact_name);
                    let stem_match =
                        info.compact_stem.len() >= 6 && line_compact.contains(&info.compact_stem);

                    if id_match || name_match || stem_match {
                        let entry = suspects.entry(info.modid_lower.clone()).or_insert_with(|| {
                            SuspectAccumulator {
                                modid: info.mod_ref.modid.clone(),
                                mod_name: info.mod_ref.name.clone(),
                                filename: Some(info.mod_ref.filename.clone()),
                                enabled: Some(info.mod_ref.enabled),
                                ..Default::default()
                            }
                        });
                        let delta = if is_stack_frame { 20 } else { 12 } * file_weight;
                        entry.score += delta;
                        entry.add_reason(format!(
                            "{} in {}",
                            if is_stack_frame {
                                "Appears in crash/error stack trace"
                            } else {
                                "Referenced in error message"
                            },
                            log_file.rel_path
                        ));
                        entry.add_sample(line);
                    }
                }
            }
        }
    }

    let mut ranked: Vec<SuspectAccumulator> = suspects.into_values().collect();
    ranked.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.mod_name.cmp(&b.mod_name)));
    ranked.truncate(8);

    let out_suspects: Vec<CrashSuspect> = ranked
        .into_iter()
        .map(|s| {
            let confidence = if s.score >= 50 {
                "high"
            } else if s.score >= 20 {
                "medium"
            } else {
                "low"
            };
            CrashSuspect {
                modid: s.modid,
                mod_name: s.mod_name,
                filename: s.filename,
                enabled: s.enabled,
                confidence: confidence.to_string(),
                reasons: s.reasons,
                sample_lines: s.sample_lines,
            }
        })
        .collect();

    let mut recommendations = Vec::new();
    if saw_missing_method {
        recommendations.push(
            "A MissingMethod/TypeLoad error usually means a mod was built for a different Vintage Story version. Check for mod updates or disable the suspect mod."
                .to_string(),
        );
    }
    if saw_missing_dep {
        recommendations.push(
            "A missing dependency or assembly was reported. Verify all required library mods are installed and enabled."
                .to_string(),
        );
    }
    if saw_harmony {
        recommendations.push(
            "Harmony patching errors often happen when two mods patch the same game method or one mod is outdated."
                .to_string(),
        );
    }
    if saw_asset_error {
        recommendations.push(
            "Asset/JSON errors were logged. These are commonly caused by outdated content mods or conflicting JSON patches."
                .to_string(),
        );
    }
    if saw_oom_or_gl {
        recommendations.push(
            "Memory or OpenGL errors were detected. Consider lowering view distance/VRAM settings or checking GPU drivers."
                .to_string(),
        );
    }
    if recommendations.is_empty() {
        if out_suspects.is_empty() && !crash_detected && error_line_count == 0 {
            recommendations.push(
                "No crashes or fatal errors were found in your recent logs.".to_string(),
            );
        } else {
            recommendations.push(
                "Try disabling the top suspect mod(s) below and relaunching, or check if an updated version is available on Mod DB."
                    .to_string(),
            );
        }
    }

    let headline = if crash_detected {
        if let Some(top) = out_suspects.first() {
            format!(
                "Crash detected — {} likely suspect{} (top: {})",
                out_suspects.len(),
                if out_suspects.len() == 1 { "" } else { "s" },
                top.mod_name
            )
        } else {
            "Crash detected in game logs (no specific mod identified)".to_string()
        }
    } else if !out_suspects.is_empty() {
        format!(
            "No crash log found, but {} mod{} logged errors",
            out_suspects.len(),
            if out_suspects.len() == 1 { "" } else { "s" }
        )
    } else if error_line_count > 0 {
        format!(
            "No crash detected ({} error line{} found in recent logs)",
            error_line_count,
            if error_line_count == 1 { "" } else { "s" }
        )
    } else {
        "All clear — no recent crashes or mod errors detected".to_string()
    };

    Ok(CrashAnalysisReport {
        crash_detected,
        headline,
        exception_summary,
        analyzed_files,
        suspects: out_suspects,
        recommendations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn make_local_mod(modid: &str, name: &str, filename: &str) -> LocalMod {
        LocalMod {
            filename: filename.to_string(),
            modid: modid.to_string(),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            authors: vec![],
            side: "both".to_string(),
            enabled: true,
            icon_base64: None,
            dependencies: HashMap::new(),
            latest_version: None,
            latest_download_url: None,
            latest_filename: None,
            update_available: false,
        }
    }

    #[test]
    fn list_and_read_logs_and_analyze_crash() {
        let dir = tempdir().unwrap();
        let logs = dir.path().join("Logs");
        fs::create_dir_all(&logs).unwrap();

        fs::write(
            logs.join("client-crash.log"),
            "21.11.2026 07:00:00 [Fatal] [combatoverhaul] System.MissingMethodException: Method not found: 'Vintagestory.API.Common.Item.Foo'.\n   at CombatOverhaul.WeaponSystem.OnAttack()\n",
        )
        .unwrap();
        fs::write(
            logs.join("client-main.log"),
            "21.11.2026 06:59:00 [Notification] Game started\n",
        )
        .unwrap();

        let listed = list_game_logs(dir.path()).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|l| l.is_crash && l.rel_path == "client-crash.log"));

        let content = read_game_log(dir.path(), "client-crash.log").unwrap();
        assert!(content.content.contains("MissingMethodException"));
        assert!(!content.truncated);

        let mods = vec![
            make_local_mod("combatoverhaul", "Combat Overhaul Fork", "combatoverhaul_1.0.0.zip"),
            make_local_mod("carryon", "Carry On", "carryon_1.0.0.zip"),
        ];

        let report = analyze_crash_logs(dir.path(), &mods).unwrap();
        assert!(report.crash_detected);
        assert_eq!(report.suspects.len(), 1);
        assert_eq!(report.suspects[0].modid, "combatoverhaul");
        assert_eq!(report.suspects[0].confidence, "high");
        assert!(report
            .recommendations
            .iter()
            .any(|r| r.contains("MissingMethod")));
    }

    #[test]
    fn rejects_path_traversal_in_read_game_log() {
        let dir = tempdir().unwrap();
        let logs = dir.path().join("Logs");
        fs::create_dir_all(&logs).unwrap();
        assert!(read_game_log(dir.path(), "../secret.log").is_err());
        assert!(read_game_log(dir.path(), "bad.exe").is_err());
    }
}
