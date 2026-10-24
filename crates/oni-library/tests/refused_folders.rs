//! A folder the disk refuses (`EACCES`, a disk owned by another user, a folder locked) is told apart
//! from an empty folder and from one gone: the navigator, the survey and the presence each say so.

#[cfg(unix)]
mod refused {
    use oni_library::navigator::{self, Computer};
    use oni_library::presence::{self, Away, Presence};
    use oni_library::{library, roots};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    struct Locked {
        root: PathBuf,
        folder: PathBuf,
    }

    impl Drop for Locked {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.folder, std::fs::Permissions::from_mode(0o755));
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    /// `Home/Locked` holding a subfolder and a photo, at mode 000; `Home/Empty`; `Home/Nowhere` never made.
    /// `None` under root, which reads through any mode: the check has nothing to see there.
    fn locked() -> Option<Locked> {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("oni-library-refused-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = PathBuf::from(library::canonical(&root));
        let folder = root.join("Home/Locked");
        std::fs::create_dir_all(folder.join("Inner")).unwrap();
        std::fs::write(folder.join("A.tif"), b"tif").unwrap();
        std::fs::create_dir_all(root.join("Home/Empty")).unwrap();
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o000)).unwrap();
        let made = Locked { root, folder };
        if std::fs::read_dir(&made.folder).is_ok() {
            println!("SKIPPED a_folder_the_disk_refuses_is_neither_empty_nor_gone: running as root reads through mode 000");
            return None;
        }
        Some(made)
    }

    #[test]
    fn a_folder_the_disk_refuses_is_neither_empty_nor_gone() {
        let Some(made) = locked() else { return };
        let (refused, empty, gone) = (text(&made.folder), text(&made.root.join("Home/Empty")), text(&made.root.join("Home/Nowhere")));
        let computer = Computer::none();

        // The navigator lists nothing for it, which is not an empty listing.
        assert_eq!(navigator::children(&refused), None);
        let survey = roots::surveyed(std::slice::from_ref(&refused), &|| true, &computer);
        assert_eq!(survey.unreadable, [refused.clone()]);
        assert_eq!(survey.away.get(&refused), Some(&Presence::Offline(Away::Denied)), "denied, not gone");
        assert_eq!(survey.presence(&format!("{refused}/A.tif")), Presence::Offline(Away::Denied));
        // The folder itself is there; what it holds is refused, never missing.
        assert_eq!(presence::of(&refused, &computer), Presence::Present);
        assert_eq!(presence::of(&format!("{refused}/A.tif"), &computer), Presence::Offline(Away::Denied));

        // Twin: a readable empty folder says empty, and is no refusal.
        assert_eq!(navigator::children(&empty), Some(Vec::new()));
        let bare = roots::surveyed(std::slice::from_ref(&empty), &|| true, &computer);
        assert!(bare.unreadable.is_empty() && bare.images.is_empty() && bare.presence(&empty).is_present());
        // Twin: a folder gone is missing, not refused.
        assert_eq!(navigator::children(&gone), None);
        let lost = roots::surveyed(std::slice::from_ref(&gone), &|| true, &computer);
        assert_eq!(lost.away.get(&gone), Some(&Presence::Missing));
        // Twin: the same folder opened again lists its subfolder and its photo.
        std::fs::set_permissions(&made.folder, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(navigator::children(&refused), Some(vec![format!("{refused}/Inner")]));
        let open = roots::surveyed(std::slice::from_ref(&refused), &|| true, &computer);
        assert!(open.unreadable.is_empty() && open.images == [format!("{refused}/A.tif")]);
    }
}
