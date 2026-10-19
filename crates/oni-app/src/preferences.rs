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
/// `@AppStorage("soundMuted")`: the key clicks silenced.
pub const SOUND_MUTED: &str = "soundMuted";
/// The RAW previews read at half size, a choice this build adds to compare speed and grain.
pub const HALF_SIZE_PREVIEWS: &str = "halfSizePreviews";

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
    pub fn set(&mut self, key: &str, value: Value) -> std::io::Result<()> {
        let mut values = Self::read(&self.file);
        values.insert(key.to_owned(), value);
        self.values = values;
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let text = serde_json::to_vec_pretty(&self.values).map_err(std::io::Error::other)?;
        let staged = self.file.with_extension("json.part");
        std::fs::write(&staged, text)?;
        std::fs::rename(&staged, &self.file)
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
