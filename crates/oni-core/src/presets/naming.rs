//! A preset's name and its file's: blanks folded, the first free default, a slug that fits the disk.

use super::*;

/// `CharacterSet.whitespaces`: tabs and space separators, never a line break.
pub(super) fn is_blank(c: char) -> bool {
    c == '\t' || (c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'))
}

/// The first free `Preset NN`, two digits widening past 99, case and edge spaces folded so
/// near-duplicates count as taken.
pub fn default_name(names: &[&str]) -> String {
    let taken: BTreeSet<String> = names.iter().map(|n| n.trim_matches(is_blank).to_lowercase()).collect();
    let mut number = 1;
    while taken.contains(&format!("preset {number:02}")) {
        number += 1;
    }
    format!("Preset {number:02}")
}

/// Longest stem kept from a name, so a pasted paragraph cannot hit the file system's 255-byte cap.
const MAX_STEM: usize = 64;

/// A readable stem plus the id's head; identity is the id inside, so a rename never orphans a file.
pub fn file_name(preset: &Preset) -> String {
    let short: String = preset.id.to_string().replace('-', "").chars().take(8).collect::<String>().to_lowercase();
    format!("{}-{short}.json", slug(&preset.name))
}

pub(super) fn slug(name: &str) -> String {
    let mut stem = String::new();
    for c in name.chars() {
        let c = if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' };
        if !(c == '-' && stem.ends_with('-')) {
            stem.push(c);
        }
    }
    let stem: String = stem.trim_matches('-').chars().take(MAX_STEM).collect();
    if stem.is_empty() { "preset".to_string() } else { stem }
}

/// The name trimmed, or the one on disk when it trims to nothing: `load` refuses a blank name,
/// so writing one would hide the preset and orphan its file.
pub fn writable_name(preset: &Preset, directory: &Path) -> String {
    let trimmed = preset.name.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    files(directory)
        .iter()
        .filter_map(|f| decode(&std::fs::read(f).ok()?))
        .find(|p| p.id == preset.id)
        .map_or_else(|| "Preset".to_string(), |p| p.name)
}
