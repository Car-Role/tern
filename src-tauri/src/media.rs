//! Files Tern shows as something other than code: images and PDFs (as bytes), big text for tables
//! and trees, and the growing end of a log file.

use crate::git::safe_join;
use crate::run;
use crate::tickets::{base64, sniff};
use serde::Serialize;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Images and PDFs bigger than this are not loaded into the window.
const MAX_BYTES: u64 = 40 * 1024 * 1024;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileBytes {
    /// A `data:` URL, or None when the file does not exist at that version.
    pub data_url: Option<String>,
    pub mime: String,
    pub size: u64,
    pub too_large: bool,
}

/// The type of a file from its first bytes, then its name.
pub fn mime_of(bytes: &[u8], rel: &str) -> String {
    // The PDF header may follow up to 1 KB of other bytes. A file named .pdf without it is not a PDF.
    if bytes[..bytes.len().min(1024)].windows(4).any(|w| w == b"%PDF") {
        return "application/pdf".into();
    }
    if bytes.len() >= 4 && bytes[..4] == [0, 0, 1, 0] {
        return "image/x-icon".into();
    }
    if let Some(m) = sniff(bytes) {
        return m.into();
    }
    let ext = rel.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
    .into()
}

/// A file's bytes as a data URL: from disk (`rev` None) or from a commit (`rev` like "HEAD" or a commit ID).
pub fn file_bytes(repo: &str, rel: &str, rev: Option<&str>) -> Result<FileBytes, String> {
    let bytes = match rev {
        None => {
            let full = safe_join(repo, rel)?;
            let Ok(meta) = fs::metadata(&full) else {
                return Ok(FileBytes { data_url: None, mime: mime_of(&[], rel), size: 0, too_large: false });
            };
            if meta.len() > MAX_BYTES {
                return Ok(FileBytes { data_url: None, mime: mime_of(&[], rel), size: meta.len(), too_large: true });
            }
            fs::read(&full).map_err(|e| format!("{rel}: {e}"))?
        }
        Some(rev) => {
            let spec = format!("{rev}:{rel}");
            let size = run::git(Path::new(repo), &["cat-file", "-s", &spec]).ok().and_then(|s| s.trim().parse::<u64>().ok());
            match size {
                None => return Ok(FileBytes { data_url: None, mime: mime_of(&[], rel), size: 0, too_large: false }),
                Some(n) if n > MAX_BYTES => return Ok(FileBytes { data_url: None, mime: mime_of(&[], rel), size: n, too_large: true }),
                Some(_) => run::git_bytes(Path::new(repo), &["cat-file", "blob", &spec])?,
            }
        }
    };
    let mime = mime_of(&bytes, rel);
    let size = bytes.len() as u64;
    Ok(FileBytes { data_url: Some(format!("data:{mime};base64,{}", base64(&bytes))), mime, size, too_large: false })
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BigText {
    pub text: Option<String>,
    pub size: u64,
    pub too_large: bool,
}

/// Text up to `max` bytes, for table and tree views of files too big for the code view.
pub fn read_text(repo: &str, rel: &str, max: u64) -> Result<BigText, String> {
    let full = safe_join(repo, rel)?;
    let size = fs::metadata(&full).map_err(|e| format!("{rel}: {e}"))?.len();
    if size > max {
        return Ok(BigText { text: None, size, too_large: true });
    }
    let bytes = fs::read(&full).map_err(|e| format!("{rel}: {e}"))?;
    Ok(BigText { text: Some(String::from_utf8_lossy(&bytes).into_owned()), size, too_large: false })
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tail {
    pub text: String,
    /// Where the next read should start.
    pub offset: u64,
    pub size: u64,
    /// True when the window should drop what it has: the first read, or the file got shorter (rotated or cleared).
    pub reset: bool,
    /// True when the start of the file was skipped because it is long.
    pub cut: bool,
}

/// The new end of a growing file. The first read (offset 0) returns the last `max` bytes, from a line start.
/// A split UTF-8 character at the end waits for the next read.
pub fn read_tail(repo: &str, rel: &str, offset: u64, max: u64) -> Result<Tail, String> {
    let full = safe_join(repo, rel)?;
    let mut f = fs::File::open(&full).map_err(|e| format!("{rel}: {e}"))?;
    let size = f.metadata().map_err(|e| e.to_string())?.len();
    let reset = offset == 0 || size < offset;
    let mut start = if reset { size.saturating_sub(max) } else { offset };
    // Read at most `max` bytes at a time, even when a lot was added.
    if size - start > max {
        start = size - max;
    }
    f.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut buf = Vec::with_capacity((size - start) as usize);
    f.take(size - start).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let mut skipped = 0usize;
    let cut = reset && start > 0;
    if cut {
        // Start at the next full line.
        if let Some(i) = buf.iter().position(|&b| b == b'\n') {
            skipped = i + 1;
        }
    }
    // Keep a split UTF-8 character for the next read: find the last character's lead byte,
    // and stop before it when not all of its bytes are here yet.
    let mut end = buf.len();
    let mut i = end;
    let mut cont = 0;
    while i > skipped && cont < 3 && (buf[i - 1] & 0xC0) == 0x80 {
        i -= 1;
        cont += 1;
    }
    if i > skipped {
        let lead = buf[i - 1];
        let need = if lead >= 0xF0 { 4 } else if lead >= 0xE0 { 3 } else if lead >= 0xC0 { 2 } else { 1 };
        if need > 1 && end - (i - 1) < need {
            end = i - 1;
        }
    }
    let text = String::from_utf8_lossy(&buf[skipped..end]).into_owned();
    Ok(Tail { text, offset: start + end as u64, size, reset, cut })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("tern-media-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn knows_file_types() {
        assert_eq!(mime_of(b"%PDF-1.7\n", "x.bin"), "application/pdf");
        assert_eq!(mime_of(b"junk\r\n%PDF-1.4", "x.pdf"), "application/pdf");
        assert_eq!(mime_of(b"hello, not a pdf", "notes.pdf"), "application/octet-stream");
        assert_eq!(mime_of(&[0x89, b'P', b'N', b'G', 0], "a.txt"), "image/png");
        assert_eq!(mime_of(b"<svg xmlns='http://www.w3.org/2000/svg'/>", "a.svg"), "image/svg+xml");
        assert_eq!(mime_of(&[0, 0, 1, 0, 1], "favicon.ico"), "image/x-icon");
        assert_eq!(mime_of(b"", "photo.JPEG"), "image/jpeg");
    }

    #[test]
    fn reads_bytes_from_disk_and_from_commits() {
        let d = tmp("bytes");
        let repo = d.to_string_lossy().into_owned();
        let g = |args: &[&str]| run::git(&d, args).unwrap();
        g(&["init", "-q", "-b", "main"]);
        g(&["config", "user.email", "t@t"]);
        g(&["config", "user.name", "t"]);
        let png = [0x89u8, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0];
        fs::write(d.join("a.png"), png).unwrap();
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "first"]);
        fs::write(d.join("a.png"), b"%PDF-1.4 now a pdf").unwrap();
        let disk = file_bytes(&repo, "a.png", None).unwrap();
        assert_eq!(disk.mime, "application/pdf");
        let old = file_bytes(&repo, "a.png", Some("HEAD")).unwrap();
        assert_eq!(old.mime, "image/png");
        assert_eq!(old.size, 10);
        assert!(old.data_url.unwrap().starts_with("data:image/png;base64,iVBORw0K"));
        assert!(file_bytes(&repo, "missing.png", None).unwrap().data_url.is_none());
        assert!(file_bytes(&repo, "missing.png", Some("HEAD")).unwrap().data_url.is_none());
        assert!(file_bytes(&repo, "../outside.png", None).is_err(), "paths outside the repo are refused");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn follows_a_growing_log() {
        let d = tmp("tail");
        let repo = d.to_string_lossy().into_owned();
        let log = d.join("app.log");
        fs::write(&log, "one\ntwo\n").unwrap();
        let t = read_tail(&repo, "app.log", 0, 1024).unwrap();
        assert_eq!((t.text.as_str(), t.reset, t.cut), ("one\ntwo\n", true, false));

        // More lines, including a character split across two writes.
        let mut f = fs::OpenOptions::new().append(true).open(&log).unwrap();
        f.write_all("three é".as_bytes()).unwrap();
        f.write_all(&"✓".as_bytes()[..2]).unwrap();
        f.flush().unwrap();
        let t2 = read_tail(&repo, "app.log", t.offset, 1024).unwrap();
        assert_eq!(t2.text, "three é");
        assert!(!t2.reset);
        f.write_all(&"✓".as_bytes()[2..]).unwrap();
        f.write_all(b"\n").unwrap();
        f.flush().unwrap();
        let t3 = read_tail(&repo, "app.log", t2.offset, 1024).unwrap();
        assert_eq!(t3.text, "✓\n");

        // The log is cleared (rotated): start over.
        fs::write(&log, "fresh\n").unwrap();
        let t4 = read_tail(&repo, "app.log", t3.offset, 1024).unwrap();
        assert_eq!((t4.text.as_str(), t4.reset), ("fresh\n", true));

        // A long log: only the end, starting at a full line.
        let big: String = (0..5000).map(|i| format!("line {i}\n")).collect();
        fs::write(&log, &big).unwrap();
        let t5 = read_tail(&repo, "app.log", 0, 200).unwrap();
        assert!(t5.cut);
        assert!(t5.text.starts_with("line ") && t5.text.ends_with("line 4999\n"), "{}", t5.text);
        assert!(t5.text.len() <= 200);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn reads_big_text_up_to_a_limit() {
        let d = tmp("big");
        let repo = d.to_string_lossy().into_owned();
        fs::write(d.join("a.csv"), "a,b\n1,2\n").unwrap();
        assert_eq!(read_text(&repo, "a.csv", 100).unwrap().text.as_deref(), Some("a,b\n1,2\n"));
        assert!(read_text(&repo, "a.csv", 4).unwrap().too_large);
        // A 9 MB table, over the code view's 5 MB limit: the table view still gets it, quickly.
        let big: String = (0..300_000).map(|i| format!("{i},\"Name, {i}\",{}\n", i as f64 * 1.5)).collect();
        fs::write(d.join("big.csv"), &big).unwrap();
        let t = std::time::Instant::now();
        let r = read_text(&repo, "big.csv", 100 * 1024 * 1024).unwrap();
        assert_eq!(r.text.as_deref().map(str::len), Some(big.len()));
        assert!(t.elapsed().as_secs() < 3, "took {:?}", t.elapsed());
        // Not valid UTF-8: still readable, with replacement characters.
        fs::write(d.join("latin1.csv"), [b"caf".as_slice(), &[0xe9], b",1\n"].concat()).unwrap();
        assert!(read_text(&repo, "latin1.csv", 100).unwrap().text.unwrap().starts_with("caf"));
        let _ = fs::remove_dir_all(&d);
    }
}
