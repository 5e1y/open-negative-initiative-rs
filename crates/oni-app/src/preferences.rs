//! What the Swift app keeps in `UserDefaults`, kept in one JSON file of the support folder under the
//! same keys, so a value read at launch is the one written by the last session.

use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// `EditorScene.openedKey`: the folder the archive showed when the app last opened one.
pub const OPENED_FOLDER: &str = "openedFolder";
/// `EditorScene.neutralDecodeNoticeShown`: the recalibration notice was dismissed once.
pub const NEUTRAL_DECODE_NOTICE_SHOWN: &str = "neutralDecodeNoticeShown";
/// `@AppStorage("galleryColumns")`: the gallery's tile size, a rung of its ladder.
pub const GALLERY_COLUMNS: &str = "galleryColumns";
/// `@AppStorage("soundMuted")`: the key clicks silenced; unset reads as silenced, the owner's default.
pub const SOUND_MUTED: &str = "soundMuted";
/// `@AppStorage("correctionBrushRadius")`: the De-dust brush's radius, kept across photos and launches.
pub const BRUSH_RADIUS: &str = "correctionBrushRadius";
/// The RAW decoded whole as a photo opens, a choice this build adds; off, LibRaw's half size, 3 to 5
/// times faster, and the editor reads it at every zoom.
pub const FULL_DECODE: &str = "fullDecode";
/// The zoom kept from one photo to the next, a choice this build adds; each opens fitted without it.
pub const KEEP_ZOOM: &str = "keepZoom";
/// The `.cube` the hand's Frontier look is read from, a choice this build adds.
pub const FRONTIER_LOOK: &str = "frontierLook";
/// The language the app speaks, `en`, `fr` or `es`, a choice this build adds; the system's without it.
pub const LANGUAGE: &str = "language";

pub struct Preferences {
    file: PathBuf,
    values: Map<String, Value>,
}

impl Preferences {
    pub fn file_path(support: &Path) -> PathBuf {
        support.join("preferences.json")
    }

    /// The support folder's preferences; a missing or unreadable file reads as none set.
    pub fn load(support: &Path) -> Preferences {
        let file = Self::file_path(support);
        let values = Self::read(&file);
        Preferences { file, values }
    }

    fn read(file: &Path) -> Map<String, Value> {
        std::fs::read(file)
            .ok()
            .and_then(|data| serde_json::from_slice::<Value>(&data).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    pub fn string(&self, key: &str) -> Option<&str> {
        self.values.get(key)?.as_str()
    }

    pub fn value(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.values.get(key)?.as_bool()
    }

    pub fn int(&self, key: &str) -> Option<i64> {
        self.values.get(key)?.as_i64()
    }

    /// Writes the whole file at once, through a sibling so a crash never leaves half a file. Read
    /// again first: another holder may have written a key since this one loaded.
    /// A failure is also written to the log, as most callers have nothing better to do with it.
    pub fn set(&mut self, key: &str, value: Value) -> std::io::Result<()> {
        let mut values = Self::read(&self.file);
        values.insert(key.to_owned(), value);
        self.values = values;
        let written = (|| {
            if let Some(folder) = self.file.parent() {
                std::fs::create_dir_all(folder)?;
            }
            let text = serde_json::to_vec_pretty(&self.values).map_err(std::io::Error::other)?;
            let staged = self.file.with_extension("json.part");
            std::fs::write(&staged, text)?;
            std::fs::rename(&staged, &self.file)
        })();
        if let Err(e) = &written {
            crate::timing::line(&format!("the preference {key} could not be kept: {e}"));
        }
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_written_is_read_back_by_the_next_session() {
        let dir = std::env::temp_dir().join(format!("oni-app-preferences-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut first = Preferences::load(&dir);
        assert_eq!(first.string(OPENED_FOLDER), None, "a fresh support folder names no folder");
        first.set(OPENED_FOLDER, Value::from("/Volumes/Scans/Roll 12")).unwrap();
        first.set("soundMuted", Value::from(true)).unwrap();
        let next = Preferences::load(&dir);
        assert_eq!(next.string(OPENED_FOLDER), Some("/Volumes/Scans/Roll 12"));
        assert_eq!(next.bool("soundMuted"), Some(true));
        // A holder loaded before another wrote keeps that key when it writes its own.
        let mut early = Preferences::load(&dir);
        Preferences::load(&dir).set("navigatorBase", Value::from("/Volumes/T7")).unwrap();
        early.set(OPENED_FOLDER, Value::from("/Volumes/Scans/Roll 13")).unwrap();
        assert_eq!(Preferences::load(&dir).string("navigatorBase"), Some("/Volumes/T7"));
        // Twin: the key the early holder wrote is the one read back.
        assert_eq!(Preferences::load(&dir).string(OPENED_FOLDER), Some("/Volumes/Scans/Roll 13"));
        // Twin: a torn file reads as nothing set rather than failing the launch.
        std::fs::write(Preferences::file_path(&dir), b"{\"openedFolder\": ").unwrap();
        assert_eq!(Preferences::load(&dir).string(OPENED_FOLDER), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
