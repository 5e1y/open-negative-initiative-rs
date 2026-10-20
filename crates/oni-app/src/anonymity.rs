//! `Diagnostics.token` and the seal's last pass: the log names a photo, a folder or a disk by a session
//! token, never by its name, so a sealed report says nothing of the person who sent it.

use sha2::{Digest, Sha256};
use std::hash::{BuildHasher, RandomState};
use std::sync::OnceLock;

/// Drawn per run and never stored, so no two reports share a token.
fn salt() -> u64 {
    static SALT: OnceLock<u64> = OnceLock::new();
    *SALT.get_or_init(|| RandomState::new().hash_one(std::process::id()))
}

/// `#` and six hex digits: the same path gives the same token all run, and nobody can read it back.
pub fn token(path: &str) -> String {
    let hash = Sha256::new().chain_update(salt().to_le_bytes()).chain_update(path.as_bytes()).finalize();
    format!("#{}", hash[..3].iter().map(|b| format!("{b:02x}")).collect::<String>())
}

/// The home folder as `~`, then every path left in the text as `<path>`, for a line written before a
/// token could stand in, or an error that quoted a path.
pub fn scrubbed(text: &str, home: &str) -> String {
    let text = if home.is_empty() { text.to_string() } else { text.replace(home, "~") };
    text.split('\n').map(scrubbed_line).collect::<Vec<_>>().join("\n")
}

fn scrubbed_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = path_start(rest) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let end = path_end(tail);
        out.push_str("<path>");
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// Where a path begins: `/`, `~/` or a drive letter, at the line's start or after a space, a quote or
/// a bracket, so `1/8` or `p50/p99` stay.
fn path_start(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    (0..bytes.len()).find(|&i| {
        let after_gap = i == 0 || matches!(bytes[i - 1], b' ' | b'"' | b'\'' | b'(' | b'[' | b'=' | b'\t' | b'\x60');
        let rooted = bytes[i] == b'/' && bytes.get(i + 1).is_some_and(|b| !b.is_ascii_whitespace());
        let home = bytes[i] == b'~' && bytes.get(i + 1) == Some(&b'/');
        let drive = bytes[i].is_ascii_alphabetic() && bytes.get(i + 1) == Some(&b':') && matches!(bytes.get(i + 2), Some(b'\\' | b'/'));
        after_gap && (rooted || home || drive)
    })
}

/// Where a path ends: a quote, a bracket, a separator of clauses, or the line's end. A path may hold
/// spaces, so a lone space never ends it.
fn path_end(text: &str) -> usize {
    const STOPS: [&str; 10] = ["\"", "'", ")", "]", "\u{60}", ", ", "; ", ": ", " (", " · "];
    STOPS.iter().filter_map(|s| text[1..].find(s).map(|i| i + 1)).min().unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The calls that write the log whose arguments name a file: a stem, a name or a displayed path.
    fn naming_calls(source: &str) -> Vec<String> {
        let mut found = Vec::new();
        for (at, _) in source.match_indices("timing::") {
            let call = &source[at..];
            let call = &call[..call.find(");").unwrap_or(call.len())];
            if (call.starts_with("timing::line(") || call.starts_with("timing::render("))
                && ["stem(", "paths::name(", ".display()", "file_name("].iter().any(|n| call.contains(n))
            {
                found.push(call.lines().next().unwrap_or_default().to_string());
            }
        }
        found
    }

    #[test]
    fn no_line_of_the_log_names_a_file() {
        let mut found = Vec::new();
        let mut dirs = vec![std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() && !path.ends_with("bench") {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("bench.rs") && !path.ends_with("anonymity.rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    found.extend(naming_calls(&text).into_iter().map(|c| format!("{}: {c}", path.display())));
                }
            }
        }
        assert!(found.is_empty(), "log lines naming a file: {found:#?}");
        let planted = "crate::timing::line(&format!(\"{}: decoded\", stem(&path)));";
        assert_eq!(naming_calls(planted).len(), 1, "twin: a stem written to the log is caught");
    }

    #[test]
    fn a_path_is_a_short_token_the_same_each_time() {
        let folder = "/Volumes/Archive Perso/2025 Décembre/tiff";
        assert_eq!(token(folder), token(folder));
        assert_eq!(token(folder).len(), 7);
        assert!(!token(folder).contains("Décembre"));
        assert_ne!(token(folder), token(&format!("{folder}2")), "twin: a sibling folder gets a token of its own");
    }

    #[test]
    fn the_scrub_leaves_no_path_and_keeps_the_numbers() {
        let log = "[    0.001] log: /Users/someone/Library/Application Support/ONI/Logs/oni.log\n\
                   [    1.200] DSC_0001: dust not generated: cannot read /Volumes/Archive Perso/Décembre secret.tif: denied (os 13)\n\
                   [    2.000] read at 1/8, p50/p99 3.1/4.0 ms, \"C:\\Users\\someone\\Pictures\\Mariage.NEF\" refused\n\
                   [    3.000] ~/Documents/Roll #8 · gone";
        let clean = scrubbed(log, "/Users/someone");
        for leak in ["someone", "Archive", "Décembre", "Mariage", "Roll #8", "Library"] {
            assert!(!clean.contains(leak), "{leak} leaked: {clean}");
        }
        assert!(clean.contains("read at 1/8, p50/p99 3.1/4.0 ms"), "{clean}");
        assert!(clean.contains("<path>: denied (os 13)"), "{clean}");
        assert!(clean.contains("<path> · gone"), "{clean}");
        assert_eq!(clean.lines().count(), 4);
        // Twin: the home masked alone, the old pass, still names the disk and the photo.
        let masked = log.replace("/Users/someone", "~");
        assert!(masked.contains("Décembre") && masked.contains("Mariage"));
    }
}
