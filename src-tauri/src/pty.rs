//! Real terminals: a shell on a pseudo-terminal (ConPTY on Windows), shown with xterm.js.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub type OnData = Arc<dyn Fn(u64, &str) + Send + Sync>;
pub type OnExit = Arc<dyn Fn(u64, Option<u32>) + Send + Sync>;

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
}

#[derive(Default)]
pub struct Terminals {
    sessions: Mutex<HashMap<u64, Session>>,
    next: AtomicU64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub id: u64,
    pub shell: String,
}

fn on_path(exe: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|d| d.join(exe).is_file())
}

/// PowerShell 7 when installed, else Windows PowerShell. On other systems, the user's shell.
fn default_shell() -> String {
    if cfg!(windows) {
        if on_path("pwsh.exe") { "pwsh.exe".into() } else { "powershell.exe".into() }
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
    }
}

impl Terminals {
    pub fn open(&self, cwd: &str, cols: u16, rows: u16, on_data: OnData, on_exit: OnExit) -> Result<Opened, String> {
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| format!("could not open a terminal: {e}"))?;
        let shell = default_shell();
        let mut cmd = CommandBuilder::new(&shell);
        if shell.contains("powershell") || shell.contains("pwsh") {
            cmd.arg("-NoLogo");
        }
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "Tern");
        let mut child: Box<dyn Child + Send + Sync> =
            pair.slave.spawn_command(cmd).map_err(|e| format!("could not start {shell}: {e}"))?;
        drop(pair.slave);

        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let killer = child.clone_killer();
        self.sessions.lock().unwrap().insert(id, Session { master: pair.master, writer, killer });

        std::thread::spawn(move || {
            let mut buf = [0u8; 16384];
            let mut rest: Vec<u8> = Vec::new();
            loop {
                let n = match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                rest.extend_from_slice(&buf[..n]);
                // Keep a UTF-8 character that is split across two reads.
                let valid = match std::str::from_utf8(&rest) {
                    Ok(_) => rest.len(),
                    Err(e) if e.error_len().is_none() => e.valid_up_to(),
                    Err(_) => rest.len(),
                };
                let text = String::from_utf8_lossy(&rest[..valid]).into_owned();
                rest.drain(..valid);
                if !text.is_empty() {
                    on_data(id, &text);
                }
            }
        });
        std::thread::spawn(move || {
            let code = child.wait().ok().map(|s| s.exit_code());
            on_exit(id, code);
        });
        Ok(Opened { id, shell })
    }

    pub fn write(&self, id: u64, data: &str) -> Result<(), String> {
        let mut s = self.sessions.lock().unwrap();
        let session = s.get_mut(&id).ok_or("this terminal is closed")?;
        session.writer.write_all(data.as_bytes()).and_then(|_| session.writer.flush()).map_err(|e| e.to_string())
    }

    pub fn resize(&self, id: u64, cols: u16, rows: u16) -> Result<(), String> {
        let s = self.sessions.lock().unwrap();
        let session = s.get(&id).ok_or("this terminal is closed")?;
        session
            .master
            .resize(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())
    }

    pub fn close(&self, id: u64) {
        if let Some(mut session) = self.sessions.lock().unwrap().remove(&id) {
            let _ = session.killer.kill();
        }
    }

    pub fn close_all(&self) {
        let ids: Vec<u64> = self.sessions.lock().unwrap().keys().copied().collect();
        for id in ids {
            self.close(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn runs_a_shell_and_echoes_input() {
        let terms = Terminals::default();
        let (tx, rx) = mpsc::channel::<String>();
        let tx = Mutex::new(tx);
        let (etx, erx) = mpsc::channel::<Option<u32>>();
        let etx = Mutex::new(etx);
        let opened = terms
            .open(
                &std::env::temp_dir().to_string_lossy(),
                100,
                30,
                Arc::new(move |_, d| {
                    let _ = tx.lock().unwrap().send(d.to_string());
                }),
                Arc::new(move |_, c| {
                    let _ = etx.lock().unwrap().send(c);
                }),
            )
            .unwrap();
        terms.resize(opened.id, 120, 40).unwrap();
        let mut seen = String::new();
        let mut sent = false;
        let until = Instant::now() + Duration::from_secs(30);
        while Instant::now() < until && !seen.contains("scout-42") {
            if let Ok(d) = rx.recv_timeout(Duration::from_millis(200)) {
                // ConPTY asks where the cursor is and waits for the answer. xterm.js answers in the app.
                if d.contains("\x1b[6n") {
                    terms.write(opened.id, "\x1b[1;1R").unwrap();
                }
                seen.push_str(&d);
            }
            if !sent && seen.len() > 8 {
                terms.write(opened.id, "Write-Output \"scout-$(6*7)\"\r").unwrap();
                sent = true;
            }
        }
        assert!(seen.contains("scout-42"), "shell output was: {seen}");
        terms.close(opened.id);
        assert!(erx.recv_timeout(Duration::from_secs(10)).is_ok(), "shell did not exit after close");
    }
}
