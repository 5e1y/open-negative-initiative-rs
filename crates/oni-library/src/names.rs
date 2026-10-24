//! The one check on a typed folder or roll name, shared by every gesture that names a folder:
//! refused in a sentence before the disk is asked, never left to a system error.

use oni_i18n::tr;

/// The system whose rules a name and a label follow, a parameter so a check reads either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
}

impl Platform {
    pub const THIS: Platform = if cfg!(windows) { Platform::Windows } else { Platform::Mac };
}

/// The characters Windows refuses in a name, the separators of both systems among them.
pub(crate) const REFUSED: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// The device names Windows opens in place of a file, whatever the extension after them.
pub(crate) const DEVICES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "COM¹", "COM²",
    "COM³", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹", "LPT²", "LPT³", "CONIN$",
    "CONOUT$",
];

/// Why a typed folder name cannot be used, `None` when it can: a separator would move the folder, a
/// leading dot hides it, and Windows refuses its own characters, devices and endings.
pub fn refused(name: &str, platform: Platform) -> Option<String> {
    let windows = platform == Platform::Windows;
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

#[cfg(test)]
mod tests {
    use super::*;
    use Platform::{Mac, Windows};

    #[test]
    fn a_rename_with_a_slash_says_why() {
        let slash = refused("12/06/2026", Mac).unwrap_or_default();
        assert!(slash.contains("“/”") && slash.contains("separates folders"), "{slash}");
        assert!(refused(".old", Mac).is_some_and(|s| s.contains("dot would hide")));
        assert!(refused("a\\b", Windows).is_some_and(|s| s.contains("“\\”")));
        assert!(refused("a?b", Windows).is_some_and(|s| s.contains("“?”")));
        // Twin: a plain name, a dot inside it, and Windows' characters on a Mac pass untouched.
        for kept in ["Portra 400 — +1", "Roll 3.5", "a?b", "a\\b"] {
            assert_eq!(refused(kept, Mac), None, "{kept}");
        }
    }

    #[test]
    fn a_trailing_dot_or_con_is_refused_in_words() {
        let dot = refused("Roll 8.", Windows).unwrap_or_default();
        assert_eq!(dot, "Windows drops a dot or a space at the end of a name.");
        assert_eq!(refused("Roll 8 ", Windows).as_deref(), Some(dot.as_str()));
        for device in ["CON", "con.d", "Nul.txt", "COM1", "lpt²"] {
            let said = refused(device, Windows).unwrap_or_default();
            assert!(said.starts_with("Windows keeps “") && said.ends_with("” for a device."), "{device}: {said}");
        }
        assert_eq!(refused("a:b", Windows).as_deref(), Some("Windows refuses “:” in a name."));
        assert_eq!(refused("tab\there", Windows).as_deref(), Some("Windows refuses “\\t” in a name."));
        // Twin: a name only starting like a device, a dot inside, or the same names on a Mac pass.
        for kept in ["CONTACT", "Console", "Roll 8.5", "COM10"] {
            assert_eq!(refused(kept, Windows), None, "{kept}");
        }
        for kept in ["Roll 8.", "CON", "con.d", "a:b"] {
            assert_eq!(refused(kept, Mac), None, "{kept}");
        }
    }
}
