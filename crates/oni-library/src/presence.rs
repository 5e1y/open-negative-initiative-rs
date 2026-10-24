//! Whether a folder or a photo is there: present, offline (its disk away or access refused) or
//! missing (gone from a disk that answers), told by `absence`'s reasons. Every look reads the disk,
//! so none runs on the interface thread.

use crate::absence::{self, Absence};
use crate::navigator::Computer;
use crate::paths::holds;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Presence {
    #[default]
    Present,
    Offline(Away),
    /// Not found while its disk answers: moved, renamed or deleted outside ONI.
    Missing,
}

/// Why an offline path cannot be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Away {
    /// Its volume is not mounted, or the disk does not answer.
    Disconnected,
    /// The disk answers and refuses: `EACCES` or `EPERM`.
    Denied,
}

impl Presence {
    pub fn is_present(self) -> bool {
        self == Presence::Present
    }
}

type Hook = Box<dyn Fn(&str) + Send + Sync>;

static LOOKING: RwLock<Option<Hook>> = RwLock::new(None);

/// Hands every look at the disk to `hook` first, so a check can refuse one on a thread that must not read.
pub fn on_look(hook: Option<Hook>) {
    if let Ok(mut held) = LOOKING.write() {
        *held = hook;
    }
}

/// Called before each read of `path` the archive makes to tell what is there.
pub fn looked(path: &str) {
    if let Ok(held) = LOOKING.read()
        && let Some(hook) = held.as_ref()
    {
        hook(path);
    }
}

/// The path looked at on the disk, a folder or a file alike. Reads the disk.
pub fn of(path: &str, computer: &Computer) -> Presence {
    looked(path);
    match std::fs::metadata(path) {
        Ok(_) => Presence::Present,
        Err(e) => Presence::from(&absence::refusal(computer, path, &e)),
    }
}

/// `of`'s answer from a refusal the disk already gave, as a survey meets one. Reads the volume's root.
pub fn refused(path: &str, refused: &std::io::Error, computer: &Computer) -> Presence {
    Presence::from(&absence::refusal(computer, path, refused))
}

impl From<&Absence> for Presence {
    /// The reason an original cannot be read, as the three states every folder and photo shows.
    fn from(absence: &Absence) -> Presence {
        match absence {
            Absence::Unplugged { .. } | Absence::Unreadable(_) => Presence::Offline(Away::Disconnected),
            Absence::Denied => Presence::Offline(Away::Denied),
            Absence::Gone => Presence::Missing,
            Absence::Empty => Presence::Present,
        }
    }
}

/// Each known path under a volume that left turns offline, from the paths alone.
pub fn left<'a>(known: impl Iterator<Item = (&'a String, &'a mut Presence)>, volumes: &[String]) {
    for (path, presence) in known {
        if volumes.iter().any(|v| holds(v, path)) {
            *presence = Presence::Offline(Away::Disconnected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    use crate::paths;

    fn rig() -> (Scratch, Computer, String) {
        let scratch = Scratch::new("presence");
        let root = paths::normal(&scratch.path().to_string_lossy()).into_owned();
        let (home, volumes) = (paths::join(&root, "Home"), paths::join(&root, "Volumes"));
        for folder in ["Home/Scans", "Volumes/T7/Film"] {
            std::fs::create_dir_all(scratch.path().join(folder)).unwrap();
        }
        std::fs::write(scratch.path().join("Volumes/T7/Film/A.NEF"), b"a").unwrap();
        (scratch, Computer { home, volumes }, root)
    }

    #[test]
    fn a_path_is_present_offline_or_missing_as_its_disk_answers() {
        let (scratch, computer, root) = rig();
        let photo = paths::join(&root, "Volumes/T7/Film/A.NEF");
        assert_eq!(of(&photo, &computer), Presence::Present);
        // Gone from a disk that answers, a folder or a file, even two folders deep: missing.
        assert_eq!(of(&paths::join(&root, "Home/Scans/Roll 12/B.NEF"), &computer), Presence::Missing);
        assert_eq!(of(&paths::join(&root, "Volumes/T7/Film/B.NEF"), &computer), Presence::Missing);
        // Its volume unmounted: offline, never missing.
        std::fs::rename(scratch.path().join("Volumes/T7"), scratch.path().join("T7 away")).unwrap();
        assert_eq!(of(&photo, &computer), Presence::Offline(Away::Disconnected));
        // Twin: without the volumes' folder known, the same path reads as missing.
        assert_eq!(of(&photo, &Computer::none()), Presence::Missing);
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_the_disk_refuses_is_denied_and_never_missing() {
        use std::os::unix::fs::PermissionsExt;
        let (scratch, computer, root) = rig();
        let locked = scratch.path().join("Home/Scans");
        std::fs::create_dir_all(locked.join("Roll")).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let inside = paths::join(&root, "Home/Scans/Roll");
        let denied = of(&inside, &computer);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(denied, Presence::Offline(Away::Denied));
        // Twin: readable again, the same folder is there.
        assert_eq!(of(&inside, &computer), Presence::Present);
    }

    #[test]
    fn a_volume_leaving_turns_only_its_own_paths_offline() {
        let mut known: std::collections::HashMap<String, Presence> =
            [("/Volumes/T7/a.NEF", Presence::Present), ("/Volumes/T70/b.NEF", Presence::Present), ("/Users/a/c.NEF", Presence::Missing)]
                .map(|(p, s)| (p.to_owned(), s))
                .into();
        left(known.iter_mut(), &["/Volumes/T7".to_owned()]);
        assert_eq!(known["/Volumes/T7/a.NEF"], Presence::Offline(Away::Disconnected));
        // Twin: a volume whose name only starts like it, and the home, keep their state.
        assert_eq!((known["/Volumes/T70/b.NEF"], known["/Users/a/c.NEF"]), (Presence::Present, Presence::Missing));
    }
}
