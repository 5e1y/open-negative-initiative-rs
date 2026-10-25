//! The words and keys that differ between macOS and Windows, chosen here alone from the platform
//! handed in, so a check reads Windows' labels on a Mac; every other source writes neither.

use oni_i18n::tr;
pub use oni_library::names::Platform;

/// The file manager's own verb: the Finder on macOS, the Explorer on Windows.
pub fn reveal(platform: Platform) -> &'static str {
    match platform {
        Platform::Mac => tr("Show in Finder"),
        Platform::Windows => tr("Show in Explorer"),
    }
}

pub fn reveal_photos(platform: Platform, count: usize) -> String {
    match platform {
        Platform::Mac => oni_i18n::tr!("Show in Finder ({count} photos)", count = count),
        Platform::Windows => oni_i18n::tr!("Show in Explorer ({count} photos)", count = count),
    }
}

pub fn reveal_log(platform: Platform) -> &'static str {
    match platform {
        Platform::Mac => tr("Reveal Log in Finder"),
        Platform::Windows => tr("Reveal Log in Explorer"),
    }
}

/// The modifiers a label writes before a key, as each system's own menus write them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keys {
    pub command: &'static str,
    pub command_shift: &'static str,
    pub option_command: &'static str,
    /// The Shift key named alone, as in "⇧ with − or +".
    pub shift: &'static str,
}

pub fn keys(platform: Platform) -> Keys {
    match platform {
        Platform::Mac => Keys { command: "⌘", command_shift: "⌘⇧", option_command: "⌥⌘", shift: "⇧" },
        Platform::Windows => Keys { command: "Ctrl+", command_shift: "Ctrl+Shift+", option_command: "Ctrl+Alt+", shift: "Shift" },
    }
}

/// The window's labels take this system's keys.
pub fn publish_keys(window: &crate::MainWindow) {
    use slint::ComponentHandle;
    let k = keys(Platform::THIS);
    let words = window.global::<crate::KeyWords>();
    words.set_command(k.command.into());
    words.set_command_shift(k.command_shift.into());
    words.set_option_command(k.option_command.into());
    words.set_shift(k.shift.into());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    const MAC_WORDS: [&str; 4] = ["Finder", "⌘", "⌥", "⇧"];

    /// The string literals of a label source that write a Mac word, comments and checks aside.
    fn mac_words(source: &str) -> Vec<String> {
        let code = source.split("#[cfg(test)]\nmod tests {").next().unwrap_or_default();
        let mut found = Vec::new();
        for line in code.lines().filter(|l| !l.trim_start().starts_with("//")) {
            let mut rest = line;
            while let Some(open) = rest.find('"') {
                let after = &rest[open + 1..];
                let mut end = None;
                let mut escaped = false;
                for (i, c) in after.char_indices() {
                    match c {
                        '\\' if !escaped => escaped = true,
                        '"' if !escaped => {
                            end = Some(i);
                            break;
                        }
                        _ => escaped = false,
                    }
                }
                let Some(end) = end else { break };
                let literal = &after[..end];
                // A system identifier (`com.apple.FinderInfo`) is read from the disk, never shown.
                if !literal.starts_with("com.apple.") && MAC_WORDS.iter().any(|w| literal.contains(w)) {
                    found.push(literal.to_owned());
                }
                rest = &after[end + 1..];
            }
        }
        found
    }

    /// Every source a shown text comes from: the Rust of the crates that word the window, and the
    /// interface's files; the design board's samples and this file aside, and checks.
    fn label_sources() -> Vec<PathBuf> {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut out = Vec::new();
        let mut stack: Vec<PathBuf> = ["oni-app/src", "oni-core/src", "oni-library/src", "oni-export/src", "oni-ui/ui"].iter().map(|r| crates.join(r)).collect();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if path.is_dir() {
                    if name != "board" && name != "tests" {
                        stack.push(path);
                    }
                } else if (name.ends_with(".rs") || name.ends_with(".slint")) && name != "tests.rs" && !path.ends_with("oni-app/src/platform.rs") {
                    out.push(path);
                }
            }
        }
        out
    }

    #[test]
    fn windows_labels_say_explorer_and_ctrl() {
        let k = keys(Platform::Windows);
        let windows = [reveal(Platform::Windows).to_owned(), reveal_photos(Platform::Windows, 3), reveal_log(Platform::Windows).to_owned(), k.command.into(), k.command_shift.into(), k.option_command.into(), k.shift.into()];
        for word in &windows {
            assert!(!MAC_WORDS.iter().any(|w| word.contains(w)), "{word}");
        }
        assert_eq!((reveal(Platform::Windows), k.command), ("Show in Explorer", "Ctrl+"));
        assert!(reveal_log(Platform::Windows).contains("Explorer") && reveal_photos(Platform::Windows, 3).contains("Explorer"));
        // The Mac keeps its own words.
        assert_eq!((reveal(Platform::Mac), keys(Platform::Mac).command), ("Show in Finder", "⌘"));
        // Every other label source writes no Mac word, so Windows never shows one.
        let sources = label_sources();
        assert!(sources.len() > 100, "the sources were found: {}", sources.len());
        let mut left = Vec::new();
        for path in sources {
            let text = std::fs::read_to_string(&path).unwrap();
            left.extend(mac_words(&text).into_iter().map(|w| format!("{}: {w}", path.display())));
        }
        assert!(left.is_empty(), "Mac words outside platform.rs:\n{}", left.join("\n"));
        // Twin: a Mac word planted in a label is caught, in Rust and in Slint; a comment is not.
        let planted = "    tr(\"Show in Finder\");\n    help: @tr(\"Zoom in (⌘+)\");\n    // ⌘ opens\n";
        assert_eq!(mac_words(planted), ["Show in Finder", "Zoom in (⌘+)"]);
        assert!(mac_words("    xattr(\"com.apple.FinderInfo\");\n").is_empty(), "a system identifier is not a label");
        assert!(mac_words("fn a() {}\n#[cfg(test)]\nmod tests {\n    \"⌘\"\n}\n").is_empty(), "a check's own literals are not labels");
    }
}
