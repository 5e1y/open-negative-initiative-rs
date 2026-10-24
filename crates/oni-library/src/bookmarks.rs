//! What this app alone keeps of each favourite beyond its path, by path: the volume it lives on and
//! its file id. A file of its own, as the Swift app rewrites `favourites.json` with its two fields.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The volume a favourite was last seen on: its UUID where the system gives one, and where it mounted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    pub mount: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<Volume>,
    #[serde(rename = "fileId", default, skip_serializing_if = "Option::is_none")]
    pub file_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmarks {
    pub version: i64,
    #[serde(default)]
    pub places: BTreeMap<String, Place>,
}

impl Default for Bookmarks {
    fn default() -> Self {
        Bookmarks { version: 1, places: BTreeMap::new() }
    }
}

impl Bookmarks {
    pub fn file_path(support: &Path) -> PathBuf {
        support.join("bookmarks.json")
    }

    pub fn load(support: &Path) -> Bookmarks {
        crate::read_json(&Self::file_path(support)).unwrap_or_default()
    }

    pub fn save(&self, support: &Path) -> std::io::Result<()> {
        crate::write_atomically(&Self::file_path(support), &serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)
    }

    /// One place per favourite, read again by `read` where its folder answers, else kept as last
    /// read: a disk away, or another volume now under the same path, never erases the identity known.
    pub fn noted(&self, favourites: &[String], read: impl Fn(&str) -> Option<Place>) -> Bookmarks {
        let mut places = BTreeMap::new();
        for path in favourites {
            let known = self.places.get(path);
            let now = read(path);
            let moved_volume = |now: &Place| {
                let uuid = |p: &Place| p.volume.as_ref().and_then(|v| v.uuid.clone());
                known.is_some_and(|k| uuid(k).is_some() && uuid(k) != uuid(now))
            };
            let place = match now {
                Some(now) if !moved_volume(&now) => Some(now),
                _ => known.cloned(),
            };
            if let Some(place) = place {
                places.insert(path.clone(), place);
            }
        }
        Bookmarks { version: self.version, places }
    }
}

/// Serialises the writes of `record`, each a read, a merge and a write of the one file.
static RECORDING: Mutex<()> = Mutex::new(());

/// Notes each favourite's place off the interface thread, as a volume asleep can hold a `statfs`.
pub fn record(support: PathBuf, favourites: Vec<String>) {
    std::thread::spawn(move || {
        let _turn = RECORDING.lock();
        // A support folder removed meanwhile, a check's own, is not made again.
        if !support.is_dir() {
            return;
        }
        let _ = Bookmarks::load(&support).noted(&favourites, place).save(&support);
    });
}

/// The volume and file id of `path` as the system reports them now, `None` when it does not answer.
pub fn place(path: &str) -> Option<Place> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(Place { volume: volume(path), file_id: file_id(&metadata) })
}

#[cfg(unix)]
fn file_id(metadata: &std::fs::Metadata) -> Option<u64> {
    Some(std::os::unix::fs::MetadataExt::ino(metadata))
}

#[cfg(not(unix))]
fn file_id(_: &std::fs::Metadata) -> Option<u64> {
    None
}

/// `statfs` names the mount, `getattrlist` on the mount its UUID, which survives a rename or "T7 1".
#[cfg(target_os = "macos")]
fn volume(path: &str) -> Option<Volume> {
    use std::ffi::{CStr, CString, c_void};
    let text = CString::new(path).ok()?;
    // SAFETY: `statfs` fills the zeroed struct it is handed, read only on success.
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(text.as_ptr(), &mut fs) } != 0 {
        return None;
    }
    // SAFETY: the kernel writes a NUL-terminated mount path into the fixed array.
    let mount = unsafe { CStr::from_ptr(fs.f_mntonname.as_ptr()) }.to_string_lossy().into_owned();
    #[repr(C)]
    struct Answer {
        length: u32,
        uuid: [u8; 16],
    }
    let mut list = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: libc::ATTR_VOL_INFO | libc::ATTR_VOL_UUID,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let mut answer = Answer { length: 0, uuid: [0; 16] };
    let root = CString::new(mount.clone()).ok()?;
    // SAFETY: both pointers are to live structs of the sizes given; the answer is read only on success.
    let got = unsafe {
        libc::getattrlist(
            root.as_ptr(),
            (&mut list as *mut libc::attrlist).cast::<c_void>(),
            (&mut answer as *mut Answer).cast::<c_void>(),
            std::mem::size_of::<Answer>(),
            0,
        )
    };
    let uuid = (got == 0 && answer.length as usize >= std::mem::size_of::<Answer>() && answer.uuid != [0; 16])
        .then(|| uuid_text(&answer.uuid));
    Some(Volume { uuid, mount })
}

#[cfg(not(target_os = "macos"))]
fn volume(_: &str) -> Option<Volume> {
    None
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn uuid_text(bytes: &[u8; 16]) -> String {
    let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

#[cfg(test)]
mod tests;
