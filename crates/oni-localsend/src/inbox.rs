//! Writing received files: a hidden part file in the destination, renamed to a free name once verified.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncWriteExt, BufWriter};

const WRITE_BUFFER: usize = 1 << 20;
const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

/// Keeps only the last path component and characters every target file system accepts.
pub fn sanitize_file_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or("");
    let cleaned: String =
        last.chars().map(|c| if c.is_control() || "<>:\"|?*".contains(c) { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']);
    if cleaned.is_empty() || cleaned.chars().all(|c| c == '.') {
        return "file".into();
    }
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_uppercase();
    let device = RESERVED.contains(&stem.as_str())
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    if device { format!("_{cleaned}") } else { cleaned.into() }
}

/// A file being received. Dropped before `finish`, it deletes itself, so a failed upload leaves nothing.
pub(crate) struct PartFile {
    path: PathBuf,
    writer: Option<BufWriter<tokio::fs::File>>,
    hasher: Sha256,
    pub(crate) written: u64,
}

impl PartFile {
    pub(crate) async fn create(dir: &Path) -> io::Result<Self> {
        let path = dir.join(format!(".oni-{}.part", crate::random_id()));
        let file = tokio::fs::OpenOptions::new().write(true).create_new(true).open(&path).await?;
        Ok(Self { path, writer: Some(BufWriter::with_capacity(WRITE_BUFFER, file)), hasher: Sha256::new(), written: 0 })
    }

    pub(crate) async fn write(&mut self, chunk: &[u8]) -> io::Result<()> {
        self.hasher.update(chunk);
        self.written += chunk.len() as u64;
        self.writer.as_mut().expect("open until finish").write_all(chunk).await
    }

    /// Flushes to disk and returns the lowercase hex SHA-256 of everything written.
    pub(crate) async fn seal(&mut self) -> io::Result<String> {
        let writer = self.writer.as_mut().expect("open until finish");
        writer.flush().await?;
        writer.get_ref().sync_all().await?;
        Ok(crate::identity::hex(&self.hasher.clone().finalize()))
    }

    /// Moves the sealed file to `dir/name`, or `name (n)` when taken; `lock` serialises the free-name check.
    pub(crate) async fn finish(mut self, dir: &Path, name: &str, lock: Arc<Mutex<()>>) -> io::Result<PathBuf> {
        drop(self.writer.take());
        let part = std::mem::take(&mut self.path);
        let (dir, name) = (dir.to_path_buf(), name.to_string());
        tokio::task::spawn_blocking(move || {
            let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
            let target = free_path(&dir, &name);
            std::fs::rename(&part, &target).map(|_| target).inspect_err(|_| {
                let _ = std::fs::remove_file(&part);
            })
        })
        .await
        .map_err(io::Error::other)?
    }
}

impl Drop for PartFile {
    fn drop(&mut self) {
        drop(self.writer.take());
        if !self.path.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn free_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (1..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|c| !c.exists()).expect("unbounded")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_lose_every_directory_part() {
        for (input, want) in [
            ("../../evil.txt", "evil.txt"),
            ("/etc/passwd", "passwd"),
            ("..\\..\\win.ini", "win.ini"),
            ("C:\\Windows\\x.dll", "x.dll"),
            ("..", "file"),
            ("dir/", "dir"),
            ("", "file"),
            ("a:b?.dng", "a_b_.dng"),
            ("con.txt", "_con.txt"),
            ("COM1", "_COM1"),
            ("COMET.txt", "COMET.txt"),
            ("trailing. ", "trailing"),
            ("P1072392.RW2", "P1072392.RW2"),
        ] {
            assert_eq!(sanitize_file_name(input), want, "{input}");
        }
    }

    #[test]
    fn free_path_suffixes_before_the_extension() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(free_path(dir.path(), "a.dng"), dir.path().join("a.dng"));
        std::fs::write(dir.path().join("a.dng"), b"").unwrap();
        std::fs::write(dir.path().join("a (1).dng"), b"").unwrap();
        assert_eq!(free_path(dir.path(), "a.dng"), dir.path().join("a (2).dng"));
        std::fs::write(dir.path().join(".rc"), b"").unwrap();
        assert_eq!(free_path(dir.path(), ".rc"), dir.path().join(".rc (1)"));
    }
}
