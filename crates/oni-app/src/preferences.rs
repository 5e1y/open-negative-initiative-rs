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

enum Read {
    Absent,
    Parsed(Map<String, Value>),
    Unreadable,
}

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
        let values = match Self::read(&file) {
            Ok(Read::Parsed(values)) => values,
            _ => Map::new(),
        };
        Preferences { file, values }
    }

    /// The file as it stands; an error when the disk refuses to read it.
    fn read(file: &Path) -> std::io::Result<Read> {
        match std::fs::read(file) {
            Ok(data) => Ok(match serde_json::from_slice::<Value>(&data) {
                Ok(Value::Object(values)) => Read::Parsed(values),
                _ => Read::Unreadable,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Read::Absent),
            Err(e) => Err(e),
        }
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

    /// The whole file written at once, read again first as another holder may have written a key;
    /// a failure is also logged, as most callers have nothing better to do with it.
    pub fn set(&mut self, key: &str, value: Value) -> std::io::Result<()> {
        let written = self.write(key, value);
        if let Err(e) = &written {
            crate::timing::line(&format!("the preference {key} could not be kept: {e}"));
        }
        written
    }

    /// A file that does not parse is set aside whole and the keys this holder knows go on; one the
    /// disk refuses to read is never replaced.
    fn write(&mut self, key: &str, value: Value) -> std::io::Result<()> {
        // One write at a time in the process, so a holder on another thread never loses this one's key.
        static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _turn = WRITING.lock().unwrap_or_else(|e| e.into_inner());
        let mut values = match Self::read(&self.file)? {
            Read::Parsed(values) => values,
            Read::Unreadable => {
                self.set_aside()?;
                self.values.clone()
            }
            Read::Absent => Map::new(),
        };
        values.insert(key.to_owned(), value);
        self.values = values;
        if let Some(folder) = self.file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let text = serde_json::to_vec_pretty(&self.values).map_err(std::io::Error::other)?;
        oni_library::write_atomically(&self.file, &text)
    }

    /// The unreadable file renamed beside itself, so whatever it held can still be read by a hand.
    fn set_aside(&self) -> std::io::Result<()> {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        std::fs::rename(&self.file, self.file.with_file_name(format!("preferences.json.unreadable-{stamp}")))
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

    fn set_aside(dir: &Path) -> Vec<Vec<u8>> {
        let mut found: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        found.retain(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("preferences.json.unreadable-")));
        found.iter().map(|p| std::fs::read(p).unwrap()).collect()
    }

    #[test]
    fn a_damaged_preferences_file_is_kept_aside_not_lost() {
        let dir = std::env::temp_dir().join(format!("oni-app-preferences-damaged-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut holder = Preferences::load(&dir);
        holder.set(OPENED_FOLDER, Value::from("/Volumes/Scans/Roll 12")).unwrap();
        holder.set(LANGUAGE, Value::from("fr")).unwrap();
        let torn = b"{\"openedFolder\": \"/Volumes/Sca";
        std::fs::write(Preferences::file_path(&dir), torn).unwrap();
        holder.set(GALLERY_COLUMNS, Value::from(5)).unwrap();
        assert_eq!(set_aside(&dir), [torn.to_vec()], "the torn bytes are kept beside, whole");
        let next = Preferences::load(&dir);
        assert_eq!(next.string(OPENED_FOLDER), Some("/Volumes/Scans/Roll 12"), "a key the holder knew goes on");
        assert_eq!((next.string(LANGUAGE), next.int(GALLERY_COLUMNS)), (Some("fr"), Some(5)));
        // Twin: a file that reads is never set aside.
        Preferences::load(&dir).set(KEEP_ZOOM, Value::from(true)).unwrap();
        assert_eq!(set_aside(&dir).len(), 1);
        // Twin: a file the disk refuses to read is never replaced.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file = Preferences::file_path(&dir);
            let before = std::fs::read(&file).unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
            let refused = Preferences::load(&dir).set(KEEP_ZOOM, Value::from(false));
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(refused.is_err());
            assert_eq!(std::fs::read(&file).unwrap(), before);
        }
        let leftovers = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".tmp")).count();
        assert_eq!(leftovers, 0, "no temporary is left beside");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
