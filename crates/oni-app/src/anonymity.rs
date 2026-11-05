//! `Diagnostics.token` and the last pass of every line: the log names a photo, a folder or a disk by a
//! session token, never by its name, so neither its file nor a sealed report says who wrote it.

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

/// The home folder, which every line of the log masks.
pub fn home() -> String {
    std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap_or_default()
}

/// Every marked name as `<name>`, every path as `<path>`, then the home folder as `~` wherever a path
/// could not be told from the words around it: the pass every line takes on its way to the log.
pub fn scrubbed(text: &str, home: &str) -> String {
    let text = spans_out(text, &[oni_i18n::NAME_MARKS]);
    let text = text.split('\n').map(scrubbed_line).collect::<Vec<_>>().join("\n");
    if home.is_empty() { text } else { text.replace(home, "~") }
}

fn scrubbed_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = path_start(rest) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let end = path_end(tail, at.checked_sub(1).map(|before| rest.as_bytes()[before]));
        out.push_str("<path>");
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// A notice as the log keeps it: every marked or quoted span and every file name `<name>`, since a
/// notice names what it speaks of and the log never does.
pub fn unnamed(text: &str) -> String {
    const SPANS: [(char, char); 5] = [oni_i18n::NAME_MARKS, ('\u{201c}', '\u{201d}'), ('\u{ab}', '\u{bb}'), ('"', '"'), ('\u{2018}', '\u{2019}')];
    let out = spans_out(text, &SPANS);
    out.split(' ').map(|word| if file_named(word) { "<name>".to_string() + trailing(word) } else { word.to_string() }).collect::<Vec<_>>().join(" ")
}

/// Each span between a pair of marks as `<name>`.
fn spans_out(text: &str, marks: &[(char, char)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((at, open, close)) = marks.iter().filter_map(|(o, c)| rest.find(*o).map(|i| (i, *o, *c))).min_by_key(|q| q.0) {
        let inner = &rest[at + open.len_utf8()..];
        let Some(end) = inner.find(close) else { break };
        out.push_str(&rest[..at]);
        out.push_str("<name>");
        rest = &inner[end + close.len_utf8()..];
    }
    out.push_str(rest);
    out
}

/// Whether a word is a file's name: a stem, a dot and a short extension, a clause's mark after it.
fn file_named(word: &str) -> bool {
    let word = word.trim_end_matches(trailing(word));
    word.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && (2..=4).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()) && ext.chars().any(|c| c.is_ascii_alphabetic()))
}

fn trailing(word: &str) -> &str {
    let kept = word.trim_end_matches([',', ':', ';', '.', ')', '!', '?']);
    &word[kept.len()..]
}

/// Where a path begins: `/`, `~/`, a drive letter or `\\`, at the line's start or after a space, a
/// quote or a bracket, curly ones included, so `1/8` or `p50/p99` stay.
fn path_start(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    (0..bytes.len()).find(|&i| {
        let after_gap = i == 0 || bytes[i - 1] >= 0x80 || matches!(bytes[i - 1], b' ' | b'"' | b'\'' | b'(' | b'[' | b'=' | b'\t' | b'\x60');
        let rooted = bytes[i] == b'/' && bytes.get(i + 1).is_some_and(|b| !b.is_ascii_whitespace());
        let home = bytes[i] == b'~' && bytes.get(i + 1) == Some(&b'/');
        let drive = bytes[i].is_ascii_alphabetic() && bytes.get(i + 1) == Some(&b':') && matches!(bytes.get(i + 2), Some(b'\\' | b'/'));
        let network = bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'\\');
        after_gap && (rooted || home || drive || network)
    })
}

/// Where a path ends: at its last closing quote when one opened it, else at a clause's separator, a
/// bracket it did not open, or the line's end. Its folders may hold spaces, commas and apostrophes.
fn path_end(text: &str, before: Option<u8>) -> usize {
    const STOPS: [&str; 7] = ["\"", "\u{60}", "\u{201d}", "\u{bb}", ": ", "; ", " · "];
    if let Some(quote) = before.filter(|b| matches!(b, b'"' | b'\'' | b'\x60'))
        && let Some(close) = text[1..].rfind(quote as char)
    {
        return close + 1;
    }
    let mut depth = 0;
    for (at, c) in text.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' if depth == 0 => return at,
            ')' | ']' => depth -= 1,
            _ => {}
        }
        if at > 0 && STOPS.iter().any(|s| text[at..].starts_with(s)) {
            return at;
        }
    }
    text.len()
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

    /// The places of the catalogue that hold a person's own word for something.
    const NAMING: [&str; 7] = ["{name}", "{names}", "{path}", "{folder}", "{disk}", "{file}", "{phones}"];

    /// Templates that never reach the log: captions, tooltips and fields set directly, or the app's own files.
    const NEVER_LOGGED: [&str; 22] = [
        "{folder} — click to jump to a folder above",
        "{name} — {state}",
        "Add {name}",
        "{name} · decoding",
        "{disk} not connected",
        "{path} is on “{disk}”, which is not connected.",
        "{path} is on a disk that is not connected.",
        "ONI is refused access to {path}.",
        "{path} is no longer there: moved, renamed or deleted outside ONI.",
        "Original on “{disk}”, which is not connected: {path}",
        "Original on a disk not connected: {path}",
        "ONI is refused access to the original at {path}",
        "Original not found at {path}",
        "{disk} Not Connected",
        "Paired with {name}.",
        "Connected: {phones}.",
        "On {name}",
        "{name}: cut short",
        "{name}: could not be decoded",
        "{name} — overcooked?",
        "{name} is in Downloads, its signature checked: open it to install.",
        "{file} cannot be read, so it is kept as it is: {error}",
    ];

    fn names_bare(template: &str) -> bool {
        let spans = [('\u{201c}', '\u{201d}')];
        let bare = spans_out(template, &spans);
        NAMING.iter().any(|place| bare.contains(place))
    }

    /// The templates naming something bare whose call marks no name.
    fn unmarked(templates: &[String], sources: &str) -> Vec<String> {
        let marks = |t: &String| sources.lines().any(|line| line.contains(&format!("\"{t}\"")) && line.contains("named("));
        templates.iter().filter(|t| names_bare(t) && !NEVER_LOGGED.contains(&t.as_str()) && !marks(t)).cloned().collect()
    }

    fn rust_under(dir: &std::path::Path, text: &mut String) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_under(&path, text);
            } else if path.extension().is_some_and(|e| e == "rs") {
                text.push_str(&std::fs::read_to_string(&path).unwrap());
            }
        }
    }

    #[test]
    fn every_template_naming_something_bare_marks_it_for_the_log() {
        let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let catalogue = std::fs::read_to_string(crates.join("oni-i18n/lang/fr/LC_MESSAGES/oni-ui.po")).unwrap();
        let templates: Vec<String> = catalogue.lines().filter_map(|l| l.strip_prefix("msgid \"")?.strip_suffix('"')).map(String::from).collect();
        let mut sources = String::new();
        for entry in std::fs::read_dir(crates).unwrap().flatten() {
            if entry.path().join("src").is_dir() {
                rust_under(&entry.path().join("src"), &mut sources);
            }
        }
        assert_eq!(unmarked(&templates, &sources), Vec::<String>::new(), "a template naming something bare reaches the log");
        for kept in NEVER_LOGGED {
            assert!(templates.iter().any(|t| t == kept), "{kept} is no longer in the catalogue");
        }
        // Twin: a new notice naming a file bare is caught, and marking its name clears it.
        let planted = vec!["{file} vanished".to_string()];
        assert_eq!(unmarked(&planted, "self.told(t, &tr!(\"{file} vanished\", file = name));"), planted);
        assert!(unmarked(&planted, "self.told(t, &tr!(\"{file} vanished\", file = oni_i18n::named(&name)));").is_empty());
    }

    #[test]
    fn a_path_goes_whole_whatever_its_folders_are_called() {
        let denied = "cannot read /Volumes/Archive Perso/Mariage, juin (Julie)/L'été 2025.nef: denied (os 13)";
        for (line, kept) in [
            (denied, "cannot read <path>: denied (os 13)"),
            ("failed (/Volumes/Archive Perso/Julie) then stopped", "failed (<path>) then stopped"),
            ("refused \"/Volumes/L'été/Julie\", twice", "refused \"<path>\", twice"),
            ("refused \u{201c}/Volumes/Archive/Julie\u{201d} here", "refused \u{201c}<path>\u{201d} here"),
            ("cannot read \\\\NAS-JEAN\\Photos\\Julie: denied", "cannot read <path>: denied"),
            ("/Users/jeanne/Julie: denied", "<path>: denied"),
            ("read at 1/8, p50/p99 3.1/4.0 ms, frame 1100x712 at 314,287", "read at 1/8, p50/p99 3.1/4.0 ms, frame 1100x712 at 314,287"),
        ] {
            assert_eq!(scrubbed(line, "/Users/jean"), kept);
        }
        // Twin: a path ended at its first comma, the former rule, leaves the rest of its folders.
        let (_, after_comma) = denied.split_once(", ").unwrap();
        assert!(after_comma.contains("Julie") && !scrubbed(denied, "").contains("Julie"));
    }

    #[test]
    fn a_marked_name_goes_where_a_bare_one_stays() {
        let marked = format!("{} connected", oni_i18n::named("Pixel de Jean, le 2"));
        assert_eq!(unnamed(&marked), "<name> connected");
        assert_eq!(scrubbed(&marked, ""), "<name> connected");
        // Twin: nothing tells a bare name from the words around it.
        assert_eq!(unnamed("Pixel de Jean connected"), "Pixel de Jean connected");
    }

    #[test]
    fn a_notice_keeps_its_words_and_loses_its_names() {
        let said = unnamed("P1073212.RW2, IMG_1.JPG: could not be read. The folder could not be made inside \u{201c}Mariage 2025\u{201d}: denied (os 13), 3.5 stops.");
        for leak in ["P1073212", "IMG_1", "Mariage"] {
            assert!(!said.contains(leak), "{leak} leaked: {said}");
        }
        assert_eq!(said, "<name>, <name>: could not be read. The folder could not be made inside <name>: denied (os 13), 3.5 stops.");
        // Twin: a notice naming nothing comes through whole, its numbers and its sentences' ends kept.
        let plain = "No frame of this roll could be measured: 2 of 3 frames. Try again.";
        assert_eq!(unnamed(plain), plain);
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
        let log = "[    0.001] log: /Users/someone/Support Folder/ONI/Logs/oni.log\n\
                   [    1.200] DSC_0001: dust not generated: cannot read /Volumes/Archive Perso/Décembre secret.tif: denied (os 13)\n\
                   [    2.000] read at 1/8, p50/p99 3.1/4.0 ms, \"C:\\Users\\someone\\Pictures\\Mariage.NEF\" refused\n\
                   [    3.000] ~/Documents/Roll #8 · gone";
        let clean = scrubbed(log, "/Users/someone");
        for leak in ["someone", "Archive", "Décembre", "Mariage", "Roll #8", "Support Folder"] {
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
