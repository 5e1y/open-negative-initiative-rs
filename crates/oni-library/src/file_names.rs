//! One file name every system ONI runs on can hold in one folder, whatever was typed.

use oni_i18n::tr;

/// The characters Windows refuses in a name, the separators of both systems among them.
const REFUSED: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// The device names Windows opens in place of a file, whatever the extension after them.
const DEVICES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "COM¹", "COM²",
    "COM³", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³", "CONIN$",
    "CONOUT$",
];

/// `name` as one portable component: refused and control characters become `-`, trailing dots and
/// spaces go (Windows drops them), a device name gains a `-`; empty when nothing is left.
pub fn portable(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if REFUSED.contains(&c) || c.is_control() { '-' } else { c }).collect();
    let cleaned = cleaned.trim_end_matches(['.', ' ']);
    if cleaned.chars().all(|c| c == '.') {
        return String::new();
    }
    let head = cleaned.split('.').next().unwrap_or_default();
    if DEVICES.contains(&head.trim_end_matches(' ').to_uppercase().as_str()) {
        return format!("{head}-{}", &cleaned[head.len()..]);
    }
    cleaned.to_owned()
}

/// Why a typed folder name cannot be used, `None` when it can: a separator would move the folder, a
/// leading dot hides it, and Windows (`windows`) refuses its own characters, devices and endings.
pub fn refused(name: &str, windows: bool) -> Option<String> {
    let separator = name.chars().find(|&c| c == '/' || (windows && c == '\\'));
    if let Some(c) = separator {
        return Some(tr!("A folder's name cannot hold “{c}”, which separates folders.", c = c));
    }
    if name.starts_with('.') {
        return Some(tr("A name starting with a dot would hide the folder.").into());
    }
    if !windows {
        return None;
    }
    if let Some(c) = name.chars().find(|&c| REFUSED.contains(&c) || c.is_control()) {
        return Some(tr!("Windows refuses “{c}” in a name.", c = c.escape_default()));
    }
    if name.ends_with(['.', ' ']) {
        return Some(tr("Windows drops a dot or a space at the end of a name.").into());
    }
    let head = name.split('.').next().unwrap_or_default().trim_end_matches(' ');
    DEVICES.contains(&head.to_uppercase().as_str()).then(|| tr!("Windows keeps “{name}” for a device.", name = head))
}

/// `folder` joined with `name` only when the result is one file directly inside `folder`.
pub fn inside(folder: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    let mut parts = std::path::Path::new(name).components();
    let one = matches!((parts.next(), parts.next()), (Some(std::path::Component::Normal(_)), None));
    let joined = folder.join(name);
    (one && joined.parent() == Some(folder)).then_some(joined)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn a_typed_name_becomes_one_portable_component() {
        assert_eq!(portable("..\\x"), "..-x");
        assert_eq!(portable("../../Best/Frame 3"), "..-..-Best-Frame 3");
        assert_eq!(portable("a?b*c\"d<e>f|g:h"), "a-b-c-d-e-f-g-h");
        assert_eq!(portable("tab\there\u{0}"), "tab-here-");
        assert_eq!(portable("Frame 3. . "), "Frame 3");
        assert_eq!(portable(".."), "");
        assert_eq!(portable("CON"), "CON-");
        assert_eq!(portable("nul.x"), "nul-.x");
        assert_eq!(portable("Com1 "), "Com1-");
        assert_eq!(portable("lpt²"), "lpt²-");
        // Twin: a name that only starts like a device, and a plain name, are kept as typed.
        assert_eq!(portable("CONTACT"), "CONTACT");
        assert_eq!(portable("Portra 400 — 03"), "Portra 400 — 03");
    }

    #[test]
    fn a_rename_with_a_slash_says_why() {
        let slash = refused("12/06/2026", false).unwrap_or_default();
        assert!(slash.contains("“/”") && slash.contains("separates folders"), "{slash}");
        assert!(refused(".old", false).is_some_and(|s| s.contains("dot would hide")));
        assert!(refused("a\\b", true).is_some_and(|s| s.contains("“\\”")));
        assert!(refused("a?b", true).is_some_and(|s| s.contains("“?”")));
        assert!(refused("Roll 3.", true).is_some_and(|s| s.contains("end of a name")));
        assert!(refused("con.d", true).is_some_and(|s| s.contains("“con”")));
        // Twin: a plain name, a dot inside it, and Windows' rules on a Mac pass untouched.
        for kept in ["Portra 400 — +1", "Roll 3.5", "a?b", "CON", "a\\b"] {
            assert_eq!(refused(kept, false), None, "{kept}");
        }
        assert_eq!(refused("CONTACT", true), None);
    }

    #[test]
    fn a_joined_name_stays_directly_in_its_folder() {
        let folder = Path::new("/r/out");
        assert_eq!(inside(folder, "a.jpg"), Some(folder.join("a.jpg")));
        assert_eq!(inside(folder, ".."), None);
        assert_eq!(inside(folder, "x/y.jpg"), None);
        assert_eq!(inside(folder, "/etc/a.jpg"), None);
        assert_eq!(inside(folder, ""), None);
        // Twin: what `portable` makes of the same text always stays inside.
        for typed in ["..", "x/y.jpg", "/etc/a.jpg", "..\\x", "C:a"] {
            let name = format!("{}.jpg", portable(typed));
            assert_eq!(inside(folder, &name).as_deref().and_then(Path::parent), Some(folder), "{typed}");
        }
    }
}
