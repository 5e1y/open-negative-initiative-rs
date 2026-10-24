//! One file name every system ONI runs on can hold in one folder, whatever was typed.

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
