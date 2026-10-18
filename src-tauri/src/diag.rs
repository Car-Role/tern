//! Diagnostics for bug reports: a small log file, and facts about this computer and the tools Tern runs.

use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// The log starts over when it reaches this size; the previous one is kept as tern.old.log.
const MAX_LOG: u64 = 1024 * 1024;

static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn set_log_dir(dir: PathBuf) {
    let _ = fs::create_dir_all(&dir);
    *LOG_DIR.lock().unwrap() = Some(dir);
}

pub fn log_dir() -> Option<PathBuf> {
    LOG_DIR.lock().unwrap().clone()
}

fn log_path(dir: &Path) -> PathBuf {
    dir.join("tern.log")
}

/// Adds one line to the log: time, level, and the message (on one line).
pub fn append(level: &str, message: &str) {
    let Some(dir) = log_dir() else { return };
    append_in(&dir, level, message);
}

pub fn append_in(dir: &Path, level: &str, message: &str) {
    let path = log_path(dir);
    if fs::metadata(&path).map(|m| m.len() > MAX_LOG).unwrap_or(false) {
        let _ = fs::rename(&path, dir.join("tern.old.log"));
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = message.replace(['\r', '\n'], " ⏎ ");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{} {:5} {}", stamp(now), level.to_uppercase(), line);
    }
}

/// The last `n` lines of the log.
pub fn tail_in(dir: &Path, n: usize) -> Vec<String> {
    let text = fs::read_to_string(log_path(dir)).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].iter().map(|s| s.to_string()).collect()
}

pub fn tail(n: usize) -> Vec<String> {
    log_dir().map(|d| tail_in(&d, n)).unwrap_or_default()
}

/// Writes panics (crashes in the Rust part) to the log before the app stops.
pub fn log_panics() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        append("panic", &info.to_string());
        prev(info);
    }));
}

/// UTC time as `2026-10-17 18:04:05`, without a date library.
fn stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    pub os: String,
    pub arch: String,
    pub webview: String,
    /// Tool name and its version line, or "not found".
    pub tools: Vec<(String, String)>,
    pub log_dir: String,
}

fn first_line(program: &str, args: &[&str]) -> String {
    let mut c = Command::new(program);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // no console window
    }
    match c.output() {
        Ok(o) => {
            let text = if o.stdout.is_empty() { o.stderr } else { o.stdout };
            String::from_utf8_lossy(&text).lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").to_string()
        }
        Err(_) => "not found".into(),
    }
}

fn os_name() -> String {
    #[cfg(windows)]
    {
        let v = first_line("cmd", &["/c", "ver"]);
        if !v.is_empty() && v != "not found" {
            return v;
        }
    }
    #[cfg(target_os = "macos")]
    {
        return format!("macOS {}", first_line("sw_vers", &["-productVersion"]));
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(t) = fs::read_to_string("/etc/os-release") {
            if let Some(l) = t.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")) {
                return l.trim_matches('"').to_string();
            }
        }
    }
    #[allow(unreachable_code)]
    std::env::consts::OS.to_string()
}

pub fn facts(webview: String) -> Facts {
    // VS Code is `code.cmd` on Windows; az is `az.cmd`.
    let code = if cfg!(windows) { "code.cmd" } else { "code" };
    let az = if cfg!(windows) { "az.cmd" } else { "az" };
    let tools = vec![
        ("git".to_string(), first_line("git", &["--version"])),
        ("gh".to_string(), first_line("gh", &["--version"])),
        ("az".to_string(), first_line(az, &["--version"])),
        ("VS Code".to_string(), first_line(code, &["--version"])),
    ];
    Facts {
        os: os_name(),
        arch: std::env::consts::ARCH.to_string(),
        webview,
        tools,
        log_dir: log_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_times() {
        assert_eq!(stamp(0), "1970-01-01 00:00:00");
        assert_eq!(stamp(1_792_276_054), "2026-10-17 22:27:34");
        assert_eq!(stamp(951_782_400), "2000-02-29 00:00:00");
    }

    #[test]
    fn appends_rotates_and_tails() {
        let d = std::env::temp_dir().join(format!("tern-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        append_in(&d, "error", "first\nsecond line");
        append_in(&d, "info", "next");
        let t = tail_in(&d, 10);
        assert_eq!(t.len(), 2);
        assert!(t[0].contains("ERROR first ⏎ second line"), "{t:?}");
        // Over the limit: the log moves to tern.old.log and starts over.
        fs::write(log_path(&d), "x".repeat(MAX_LOG as usize + 10)).unwrap();
        append_in(&d, "warn", "fresh");
        assert!(d.join("tern.old.log").exists());
        assert_eq!(tail_in(&d, 5).len(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn finds_git() {
        let f = facts("test".into());
        assert!(f.tools[0].1.starts_with("git version"), "{:?}", f.tools);
        assert!(!f.os.is_empty());
    }
}
