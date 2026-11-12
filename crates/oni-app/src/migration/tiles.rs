//! The thumbnails brought to `thumbnails::file_name`'s spelling, once: the former app and early builds
//! kept a dated print's colons, and a tile not found under its name is drawn again, hours for a library.

use crate::thumbnails::file_name;
use std::path::Path;
use std::time::SystemTime;

/// Each tile named another way renamed to its print's portable name, unless a tile as new or newer
/// stands there already, in which case it goes; how many were renamed. Windows never took a colon.
pub fn renamed(folder: &Path) -> usize {
    if cfg!(windows) {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(folder) else { return 0 };
    let mut renamed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(stem) = name.to_str().and_then(|n| n.strip_suffix(".jpg")) else { continue };
        if stem.bytes().all(portable) {
            continue;
        }
        let (old, new) = (entry.path(), folder.join(file_name(stem)));
        match (modified(&new), modified(&old)) {
            (Some(there), Some(here)) if there >= here => {
                let _ = std::fs::remove_file(&old);
            }
            _ => renamed += usize::from(std::fs::rename(&old, &new).is_ok()),
        }
    }
    renamed
}

/// A byte `file_name` writes as it is; `_` too, as it starts each byte it spells in hex.
fn portable(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-. _".contains(&byte)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}
