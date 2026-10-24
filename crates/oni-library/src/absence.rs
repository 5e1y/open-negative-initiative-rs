//! Why an original cannot be read, told before any decode: its disk away, the file gone, access
//! refused, or a file with no byte; each has its own words, never a decoder's failure.

use crate::navigator::{Computer, DRIVES};
use crate::paths;
use std::io::ErrorKind;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Absence {
    /// The volume the path lies on is not mounted; its name as the Finder shows it.
    Unplugged { volume: String },
    /// The disk is there, the file is not: moved, renamed or deleted outside ONI.
    Gone,
    /// The system refuses ONI the file (`EACCES`, `EPERM`).
    Denied,
    /// No byte in it, as an interrupted copy leaves a file.
    Empty,
    /// Any other refusal of the disk, its message kept.
    Unreadable(String),
}

/// What keeps `path` from being read, `None` for a file ONI can open. Reads the disk: a stat, then
/// an open, since macOS's privacy rules let a stat through and refuse the read.
pub fn absence(computer: &Computer, path: &Path) -> Option<Absence> {
    crate::presence::looked(&path.to_string_lossy());
    let refused = match std::fs::metadata(path) {
        Ok(m) if m.is_file() && m.len() == 0 => return Some(Absence::Empty),
        Ok(m) if m.is_file() => std::fs::File::open(path).err()?,
        Ok(_) => std::io::Error::from(ErrorKind::NotFound),
        Err(e) => e,
    };
    Some(refusal(computer, &path.to_string_lossy(), &refused))
}

/// What a refusal of `path` by the disk means: its volume away, gone from a disk that answers,
/// denied, or another failure. Reads the disk: the volume's root.
pub fn refusal(computer: &Computer, path: &str, refused: &std::io::Error) -> Absence {
    if let Some(root) = volume_root(computer, path) {
        crate::presence::looked(&root);
        if !Path::new(&root).is_dir() {
            let volume = paths::name(&root).trim_end_matches('/').to_string();
            return Absence::Unplugged { volume: if volume.is_empty() { root } else { volume } };
        }
    }
    match refused.kind() {
        ErrorKind::PermissionDenied => Absence::Denied,
        ErrorKind::NotFound | ErrorKind::NotADirectory => Absence::Gone,
        _ => Absence::Unreadable(refused.to_string()),
    }
}

/// The root of the volume `path` lies on when that volume may come and go: `/Volumes/T7`, or a
/// drive other than the home's on Windows; `None` on the boot disk.
pub fn volume_root(computer: &Computer, path: &str) -> Option<String> {
    if computer.volumes.is_empty() || !computer.is_on_volume(path) {
        return None;
    }
    if computer.volumes == DRIVES {
        return path.get(..2).map(|d| format!("{d}/"));
    }
    let rest = path.strip_prefix(computer.volumes.as_str())?.trim_start_matches('/');
    let name = rest.split('/').next().filter(|n| !n.is_empty())?;
    Some(paths::join(&computer.volumes, name))
}

/// The folder nearest `path` that is still on the disk, `path`'s own parent first: what Show the
/// Folder opens for an original that is gone.
pub fn nearest_folder(path: &str) -> String {
    let mut at = paths::parent(path);
    while !Path::new(&at).is_dir() {
        let up = paths::parent(&at);
        if up == at {
            break;
        }
        at = up;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    /// A home and a folder of volumes inside the check's own scratch, so no test reads `/Volumes`.
    fn machine(scratch: &Scratch) -> (Computer, String) {
        let root = scratch.path().to_string_lossy().into_owned();
        let volumes = paths::join(&root, "Volumes");
        std::fs::create_dir_all(paths::join(&volumes, "T7/Roll")).unwrap();
        std::fs::create_dir_all(paths::join(&root, "home/Scans")).unwrap();
        (Computer { home: paths::join(&root, "home"), volumes }, root)
    }

    #[test]
    fn each_reason_an_original_cannot_be_read_is_told_apart() {
        let scratch = Scratch::new("absence");
        let (c, root) = machine(&scratch);
        let on_t7 = paths::join(&c.volumes, "T7/Roll/P1.NEF");
        std::fs::write(&on_t7, b"raw").unwrap();
        assert_eq!(absence(&c, Path::new(&on_t7)), None, "a file there is no absence");
        let home = paths::join(&root, "home/Scans/P2.NEF");
        assert_eq!(absence(&c, Path::new(&home)), Some(Absence::Gone), "the home's disk is there, the file is not");
        std::fs::write(&home, b"").unwrap();
        assert_eq!(absence(&c, Path::new(&home)), Some(Absence::Empty));
        let gone_on_t7 = paths::join(&c.volumes, "T7/Roll/P3.NEF");
        assert_eq!(absence(&c, Path::new(&gone_on_t7)), Some(Absence::Gone), "twin: T7 mounted, the file moved");
        std::fs::rename(paths::join(&c.volumes, "T7"), paths::join(&root, "T7 away")).unwrap();
        let unplugged = Some(Absence::Unplugged { volume: "T7".into() });
        assert_eq!(absence(&c, Path::new(&on_t7)), unplugged, "the volume's root is gone: its disk is away");
        assert_eq!(volume_root(&c, &home), None, "the home is on no volume that comes and goes");
        assert_eq!(volume_root(&Computer::none(), &on_t7), None, "no volumes folder, no volume");
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_closed_to_oni_reads_denied_and_not_gone() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("absence-denied");
        let (c, root) = machine(&scratch);
        let folder = paths::join(&root, "home/Scans");
        let file = paths::join(&folder, "P1.NEF");
        std::fs::write(&file, b"raw").unwrap();
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o000)).unwrap();
        let closed = absence(&c, Path::new(&file));
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(closed, Some(Absence::Denied));
        assert_eq!(absence(&c, Path::new(&file)), None, "twin: opened again, it reads");
    }

    #[test]
    fn show_the_folder_climbs_to_the_nearest_folder_still_there() {
        let scratch = Scratch::new("absence-nearest");
        let root = scratch.path().to_string_lossy().into_owned();
        std::fs::create_dir_all(paths::join(&root, "Scans")).unwrap();
        assert_eq!(nearest_folder(&paths::join(&root, "Scans/Roll 12/P1.NEF")), paths::join(&root, "Scans"));
        assert_eq!(nearest_folder(&paths::join(&root, "Scans/P1.NEF")), paths::join(&root, "Scans"), "twin: its own folder");
    }
}
