//! Runs tasks on a pseudo-terminal (ConPTY on Windows), streams their output to the window, and stops them.
//! A pseudo-terminal makes programs act as they do in a real terminal: colors, progress bars, prompts,
//! arrow keys, and Ctrl+C all work, and the window can send every key press.

use crate::run;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Where output and exit news go. The app sends them to the window; tests collect them.
pub type OnOutput = Arc<dyn Fn(u64, &str) + Send + Sync>;
pub type OnExit = Arc<dyn Fn(RunInfo) + Send + Sync>;

/// Output kept per run for reopening the panel. Older output is dropped first.
const MAX_OUTPUT: usize = 2 * 1024 * 1024;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RunInfo {
    pub id: u64,
    pub repo: String,
    pub task_id: String,
    pub name: String,
    pub command: String,
    pub cwd: String,
    /// Unix milliseconds.
    pub started: i64,
    pub ended: Option<i64>,
    pub code: Option<i32>,
    pub running: bool,
    pub stopped: bool,
}

struct Run {
    info: RunInfo,
    pid: Option<u32>,
    /// The terminal and its input. Dropped when the task ends, which also ends the reader.
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
    output: Arc<Mutex<String>>,
}

/// The terminal size a task starts with. The window sends the real size as soon as it shows the task.
const START_SIZE: PtySize = PtySize { rows: 30, cols: 120, pixel_width: 0, pixel_height: 0 };

#[derive(Default)]
pub struct Runner {
    runs: Mutex<HashMap<u64, Run>>,
    next: AtomicU64,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn push_output(buf: &Mutex<String>, text: &str) {
    let mut b = buf.lock().unwrap();
    b.push_str(text);
    if b.len() > MAX_OUTPUT {
        let mut cut = b.len() - MAX_OUTPUT / 2;
        while !b.is_char_boundary(cut) {
            cut += 1;
        }
        let cut = b[cut..].find('\n').map(|n| cut + n + 1).unwrap_or(cut);
        b.drain(..cut);
    }
}

/// ConPTY asks where the cursor is (`ESC [6n`) when a task starts, and waits for the answer.
const CURSOR_QUESTION: &str = "\x1b[6n";

/// Reads the terminal and sends text to the window. Keeps a split UTF-8 character until its other half arrives.
/// `answer` replies to ConPTY's first cursor question. That question is removed from the output,
/// so the window's terminal does not answer it a second time.
fn pump(on_output: OnOutput, id: u64, mut pipe: impl Read, buf: Arc<Mutex<String>>, mut answer: Option<Box<dyn FnOnce() + Send>>) {
    let mut chunk = [0u8; 8192];
    let mut rest: Vec<u8> = Vec::new();
    loop {
        let n = match pipe.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        rest.extend_from_slice(&chunk[..n]);
        let valid = match std::str::from_utf8(&rest) {
            Ok(_) => rest.len(),
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            Err(_) => rest.len(),
        };
        let mut text = String::from_utf8_lossy(&rest[..valid]).into_owned();
        rest.drain(..valid);
        if answer.is_some() {
            if let Some(at) = text.find(CURSOR_QUESTION) {
                text.replace_range(at..at + CURSOR_QUESTION.len(), "");
                if let Some(reply) = answer.take() {
                    reply();
                }
            }
        }
        if !text.is_empty() {
            push_output(&buf, &text);
            on_output(id, &text);
        }
    }
    if !rest.is_empty() {
        let text = String::from_utf8_lossy(&rest).into_owned();
        push_output(&buf, &text);
        on_output(id, &text);
    }
}

/// Stops a process and every process it started.
fn kill_tree(pid: u32) {
    if cfg!(windows) {
        let _ = run::command("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).output();
    } else {
        let _ = run::command("kill").args(["-TERM", &format!("-{pid}")]).output();
        let _ = run::command("kill").args(["-TERM", &pid.to_string()]).output();
    }
}

impl Runner {
    pub fn start(
        self: &Arc<Self>,
        on_output: OnOutput,
        on_exit: OnExit,
        repo: &str,
        task_id: &str,
        name: &str,
        command: &str,
        cwd_rel: &str,
    ) -> Result<RunInfo, String> {
        let cwd = if cwd_rel.is_empty() { Path::new(repo).to_path_buf() } else { Path::new(repo).join(cwd_rel) };
        if !cwd.is_dir() {
            return Err(format!("folder not found: {}", cwd.display()));
        }
        let mut cmd = if cfg!(windows) {
            // cmd expands %TERN_TASK% and then runs it exactly as typed, quotes and all.
            let mut c = CommandBuilder::new("cmd.exe");
            c.args(["/D", "/S", "/C", "%TERN_TASK%"]);
            c.env("TERN_TASK", command);
            c
        } else {
            let mut c = CommandBuilder::new("sh");
            c.args(["-c", command]);
            c
        };
        cmd.cwd(&cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "Tern");
        cmd.env("FORCE_COLOR", "1");
        cmd.env("PYTHONUNBUFFERED", "1");
        let pair = native_pty_system().openpty(START_SIZE).map_err(|e| format!("could not open a terminal: {e}"))?;
        let mut child = pair.slave.spawn_command(cmd).map_err(|e| format!("could not start: {e}"))?;
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let info = RunInfo {
            id,
            repo: repo.to_string(),
            task_id: task_id.to_string(),
            name: name.to_string(),
            command: command.to_string(),
            cwd: cwd_rel.to_string(),
            started: now_ms(),
            ended: None,
            code: None,
            running: true,
            stopped: false,
        };
        let output = Arc::new(Mutex::new(String::new()));
        let place = if cwd_rel.is_empty() { String::new() } else { format!("    (in {cwd_rel})") };
        let header = format!("\x1b[2m> {command}{place}\x1b[0m\r\n");
        push_output(&output, &header);

        self.runs.lock().unwrap().insert(
            id,
            Run {
                info: info.clone(),
                pid: child.process_id(),
                master: Some(pair.master),
                writer: Some(writer),
                killer: child.clone_killer(),
                output: output.clone(),
            },
        );
        // Sent before the readers start, so the command line is always the first line.
        on_output(id, &header);

        // The reader says when the terminal closes, so the exit news comes after the last output.
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        {
            let (out, buf) = (on_output.clone(), output.clone());
            let me = Arc::clone(self);
            // The cursor sits on the line after the command line (which can wrap at the start width).
            let header_len = format!("> {command}{place}").chars().count();
            let row = 2 + header_len.saturating_sub(1) / START_SIZE.cols as usize;
            let answer: Box<dyn FnOnce() + Send> = Box::new(move || {
                let _ = me.input(id, &format!("\x1b[{row};1R"));
            });
            std::thread::spawn(move || {
                pump(out, id, reader, buf, if cfg!(windows) { Some(answer) } else { None });
                let _ = done_tx.send(());
            });
        }
        let me = Arc::clone(self);
        std::thread::spawn(move || {
            let code = child.wait().ok().map(|s| s.exit_code() as i32);
            // ConPTY keeps the reader open until the terminal is closed. Let the last output arrive,
            // then close the terminal so the reader ends.
            let _ = done_rx.recv_timeout(std::time::Duration::from_millis(300));
            let (master, writer) = {
                let mut runs = me.runs.lock().unwrap();
                runs.get_mut(&id).map(|r| (r.master.take(), r.writer.take())).unwrap_or((None, None))
            };
            drop(writer);
            drop(master);
            let _ = done_rx.recv_timeout(std::time::Duration::from_secs(2));
            let done = {
                let mut runs = me.runs.lock().unwrap();
                runs.get_mut(&id).map(|r| {
                    r.info.running = false;
                    r.info.ended = Some(now_ms());
                    r.info.code = code;
                    r.info.clone()
                })
            };
            if let Some(info) = done {
                on_exit(info);
            }
        });
        Ok(info)
    }

    pub fn stop(&self, id: u64) -> Result<(), String> {
        let pid = {
            let mut runs = self.runs.lock().unwrap();
            let run = runs.get_mut(&id).ok_or("this run is gone")?;
            if !run.info.running {
                return Ok(());
            }
            run.info.stopped = true;
            match run.pid {
                Some(pid) => pid,
                None => return run.killer.kill().map_err(|e| e.to_string()),
            }
        };
        kill_tree(pid);
        Ok(())
    }

    /// Sends key presses or pasted text to the task, as a terminal would.
    pub fn input(&self, id: u64, text: &str) -> Result<(), String> {
        let mut runs = self.runs.lock().unwrap();
        let run = runs.get_mut(&id).ok_or("this run is gone")?;
        let writer = run.writer.as_mut().ok_or("this task is not running")?;
        writer.write_all(text.as_bytes()).and_then(|_| writer.flush()).map_err(|e| e.to_string())
    }

    /// Tells the task the size of its terminal, so lines wrap and full-screen programs fit.
    pub fn resize(&self, id: u64, cols: u16, rows: u16) -> Result<(), String> {
        let runs = self.runs.lock().unwrap();
        let run = runs.get(&id).ok_or("this run is gone")?;
        match &run.master {
            Some(m) => m.resize(PtySize { rows: rows.max(2), cols: cols.max(10), pixel_width: 0, pixel_height: 0 }).map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    pub fn output(&self, id: u64) -> Result<String, String> {
        let runs = self.runs.lock().unwrap();
        let run = runs.get(&id).ok_or("this run is gone")?;
        let text = run.output.lock().unwrap().clone();
        Ok(text)
    }

    pub fn list(&self) -> Vec<RunInfo> {
        let mut v: Vec<RunInfo> = self.runs.lock().unwrap().values().map(|r| r.info.clone()).collect();
        v.sort_by_key(|r| r.id);
        v
    }

    /// Forgets a finished run and its output.
    pub fn forget(&self, id: u64) {
        let mut runs = self.runs.lock().unwrap();
        if runs.get(&id).map(|r| !r.info.running).unwrap_or(false) {
            runs.remove(&id);
        }
    }

    /// Stops every running task. Called when Tern closes, so no dev server keeps running unseen.
    pub fn stop_all(&self) {
        let pids: Vec<u32> = self.runs.lock().unwrap().values().filter(|r| r.info.running).filter_map(|r| r.pid).collect();
        for pid in pids {
            kill_tree(pid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    fn start(runner: &Arc<Runner>, command: &str) -> (RunInfo, mpsc::Receiver<String>, mpsc::Receiver<RunInfo>) {
        let (otx, orx) = mpsc::channel();
        let (etx, erx) = mpsc::channel();
        let otx = Mutex::new(otx);
        let etx = Mutex::new(etx);
        let dir = std::env::temp_dir();
        let info = runner
            .start(
                Arc::new(move |_, t| {
                    let _ = otx.lock().unwrap().send(t.to_string());
                }),
                Arc::new(move |i| {
                    let _ = etx.lock().unwrap().send(i);
                }),
                &dir.to_string_lossy(),
                "t",
                "t",
                command,
                "",
            )
            .unwrap();
        (info, orx, erx)
    }

    #[test]
    fn runs_a_command_and_reports_the_exit_code() {
        let runner = Arc::new(Runner::default());
        let (info, orx, erx) = start(&runner, "echo hello \"quoted\" && exit 3");
        let done = erx.recv_timeout(Duration::from_secs(20)).expect("no exit");
        assert_eq!(done.id, info.id);
        assert_eq!(done.code, Some(3));
        assert!(!done.running);
        let text: String = orx.try_iter().collect();
        assert!(text.contains("hello \"quoted\""), "{text}");
        assert!(runner.output(info.id).unwrap().contains("hello"));
    }

    #[test]
    fn stops_a_long_running_command() {
        let runner = Arc::new(Runner::default());
        let long = if cfg!(windows) { "ping -n 30 127.0.0.1" } else { "sleep 30" };
        let (info, _orx, erx) = start(&runner, long);
        std::thread::sleep(Duration::from_millis(500));
        runner.stop(info.id).unwrap();
        let done = erx.recv_timeout(Duration::from_secs(10)).expect("did not stop");
        assert!(done.stopped);
        assert!(!done.running);
    }

    #[test]
    fn sends_input_to_the_task() {
        let runner = Arc::new(Runner::default());
        let cmd = if cfg!(windows) { "set /p NAME= && call echo got %NAME%" } else { "read NAME; echo got $NAME" };
        let (info, orx, erx) = start(&runner, cmd);
        // Like a person typing: the terminal reads keys once the task has started.
        std::thread::sleep(Duration::from_millis(1000));
        runner.input(info.id, "tern\r").unwrap();
        let exited = erx.recv_timeout(Duration::from_secs(20));
        let text: String = orx.try_iter().collect();
        assert!(exited.is_ok(), "no exit: {text:?}");
        assert!(text.contains("got tern"), "{text}");
    }
}

