//! Running external tools (git, gh, az) without flashing console windows.

use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Builds a command that never opens a console window on Windows.
pub fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// `az` is a batch file on Windows, so it must be called by its full name.
pub fn az_program() -> &'static str {
    if cfg!(windows) {
        "az.cmd"
    } else {
        "az"
    }
}

fn git_command(repo: &Path) -> Command {
    let mut cmd = command("git");
    cmd.arg("-C")
        .arg(repo)
        .args(["-c", "core.quotepath=off", "-c", "color.ui=never"])
        // Never take the index lock for read-only work. Agents may be running git at the same time.
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    cmd
}

fn output(mut cmd: Command, what: &str) -> Result<Output, String> {
    cmd.output().map_err(|e| format!("could not start {what}: {e}"))
}

fn stderr_text(out: &Output) -> String {
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if err.is_empty() {
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    } else {
        err
    }
}

/// Runs git and returns stdout as text. Fails on a non-zero exit code.
pub fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let bytes = git_bytes(repo, args)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Runs git and returns raw stdout. Fails on a non-zero exit code.
pub fn git_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut cmd = git_command(repo);
    cmd.args(args);
    let out = output(cmd, "git")?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(stderr_text(&out))
    }
}

/// Runs git and accepts the listed exit codes as success (for example `diff --no-index` returns 1).
pub fn git_allow(repo: &Path, args: &[&str], ok_codes: &[i32]) -> Result<String, String> {
    let mut cmd = git_command(repo);
    cmd.args(args);
    let out = output(cmd, "git")?;
    let code = out.status.code().unwrap_or(-1);
    if out.status.success() || ok_codes.contains(&code) {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(stderr_text(&out))
    }
}

/// Runs git for a network action and returns stdout and stderr together (git prints progress to stderr).
pub fn git_verbose(repo: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = git_command(repo);
    cmd.args(args);
    let out = output(cmd, "git")?;
    let mut text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !err.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    if out.status.success() {
        Ok(text)
    } else {
        Err(text)
    }
}

/// Runs git with text on stdin (for example a commit message with `-F -`). Returns stdout and stderr.
pub fn git_input(repo: &Path, args: &[&str], input: &str) -> Result<String, String> {
    use std::io::Write;
    let mut cmd = git_command(repo);
    cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("could not start git: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let mut text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !err.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    if out.status.success() {
        Ok(text)
    } else {
        Err(text)
    }
}

/// Runs any tool in a folder and returns stdout. Fails on a non-zero exit code.
pub fn tool(program: &str, cwd: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = command(program);
    cmd.current_dir(cwd).args(args);
    let out = output(cmd, program)?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(stderr_text(&out))
    }
}
