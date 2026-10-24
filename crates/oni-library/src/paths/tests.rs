use super::*;

#[test]
fn a_root_holds_its_tree_and_never_a_sibling_sharing_its_prefix() {
    assert!(holds("/r", "/r"));
    assert!(holds("/r", "/r/a/b"));
    assert!(holds("/r/", "/r/a"));
    // Twin: a bare prefix test would take the sibling.
    assert!("/r/ab".starts_with("/r/a"));
    assert!(!holds("/r/a", "/r/ab"));
    assert_eq!(relative("/r/a/b", "/r"), "a/b");
    assert_eq!(relative("/r", "/r"), "r");
    assert_eq!(relative("/s/x", "/r"), "x");
}

#[test]
fn cutting_follows_nsstring() {
    assert_eq!(parent("/a/b"), "/a");
    assert_eq!(parent("/a"), "/");
    assert_eq!(parent("/"), "/");
    assert_eq!(name("/a/b.RW2"), "b.RW2");
    assert_eq!(stem("/a/b.RW2"), "b");
    assert_eq!(extension("/a/Trip"), "");
    assert_eq!(join("/a", "/b.RW2"), "/a/b.RW2");
    // Twin: a bare path push lets an absolute name leave the folder.
    let mut bare = std::path::PathBuf::from("/a");
    bare.push("/b.RW2");
    assert_eq!(bare, Path::new("/b.RW2"));
}

/// Windows hands `\\` and `/` alike; the archive keeps `/`, so one folder is one string.
#[cfg(windows)]
#[test]
fn a_windows_path_reads_one_way_whatever_its_separators() {
    assert_eq!(normal(r"C:\Scans\Roll 8"), "C:/Scans/Roll 8");
    assert!(holds(r"C:\Scans", "C:/Scans/Roll 8/A.NEF") && !holds("C:/Scans", r"C:\Scansion"));
    assert_eq!(join(r"C:\Scans", "Roll 8"), "C:/Scans/Roll 8");
    assert_eq!((parent(r"C:\Scans\Roll 8"), name(r"C:\Scans\A.NEF")), ("C:/Scans".to_string(), "A.NEF".to_string()));
    assert_eq!(relative(r"C:\Scans\Roll 8\Day 2", "C:/Scans"), "Roll 8/Day 2");
}

/// Twin: on a Mac `\\` is a character a name may hold, never a separator.
#[cfg(not(windows))]
#[test]
fn a_backslash_stays_in_a_mac_name() {
    assert_eq!(normal(r"/Scans/a\b"), r"/Scans/a\b");
    assert_eq!(name(r"/Scans/a\b"), r"a\b");
}

#[test]
fn a_rename_carries_what_lies_below_cut_by_names() {
    assert_eq!(moved("/r/a/b.NEF", "/r/a", "/r/z"), "/r/z/b.NEF");
    assert_eq!(moved("/r/a", "/r/a", "/r/z"), "/r/z");
    assert_eq!(moved("/r/ab", "/r/a", "/r/z"), "/r/ab");
    assert_eq!(below("/", "/a/b"), Some("a/b".into()));
    // Twin: cut by the root's bytes, a decomposed root takes a letter of its composed path's rest.
    let (nfd, nfc) = ("/r/E\u{301}te\u{301}", "/r/\u{c9}t\u{e9}/b.NEF");
    assert_eq!(nfc.get(nfd.len()..), Some(".NEF"));
    if cfg!(target_os = "macos") {
        assert_eq!(moved(nfc, nfd, "/r/Hiver"), "/r/Hiver/b.NEF");
        assert_eq!(relative(nfc, nfd), "b.NEF");
    }
}

/// A share, a mapped drive and a `subst` drive each keep one spelling, the one the person went through.
#[test]
fn a_windows_share_or_mapped_drive_has_one_spelling() {
    let behind = |root: &str| match root {
        "Z:\\" => Some(r"\\?\UNC\nas\film\".to_string()),
        "S:\\" => Some(r"\\?\C:\Real\Scans".to_string()),
        _ => None,
    };
    let spelt = |resolved: &str, asked: &str| windows_spelling(resolved, asked, &behind);
    assert_eq!(spelt(r"\\?\UNC\nas\film\Roll 12\a.NEF", "//nas/film/Roll 12/a.NEF"), "//nas/film/Roll 12/a.NEF");
    assert_eq!(spelt(r"\\?\UNC\nas\film\Roll 12\a.NEF", "Z:/Roll 12/a.NEF"), "Z:/Roll 12/a.NEF");
    assert_eq!(spelt(r"\\?\UNC\nas\film", "z:/"), "Z:/");
    assert_eq!(spelt(r"\\?\C:\Real\Scans\Roll 8", "S:/Roll 8"), "S:/Roll 8");
    assert_eq!(spelt(r"\\?\C:\Scans\Roll 8", "C:/Scans/Roll 8"), "C:/Scans/Roll 8");
    // Twin: the share's own spelling stays when the drive behind is unknown, never a third one.
    assert_eq!(spelt(r"\\?\UNC\nas\film\Roll 12", "Y:/Roll 12"), "//nas/film/Roll 12");
    assert_ne!(spelt(r"\\?\UNC\nas\film\Roll 12", "Z:/Roll 12"), r"//?/UNC/nas/film/Roll 12");
}

/// exFAT and a folder copied from HFS+ list `Été` decomposed; a favourite, a typed path or the Swift
/// app's spell it composed. Both are one folder in the archive.
#[cfg(target_os = "macos")]
#[test]
fn a_path_spelt_nfc_holds_its_nfd_listing() {
    use crate::favourites::Favourites;
    let scratch = crate::scratch::Scratch::new("nfd");
    let root = normal(&scratch.path().to_string_lossy()).into_owned();
    let (nfd, nfc) = ("E\u{301}te\u{301}", "\u{c9}t\u{e9}");
    std::fs::create_dir_all(scratch.path().join(nfd).join("Roll 3")).unwrap();
    std::fs::write(scratch.path().join(nfd).join("Roll 3").join("a.NEF"), [0u8]).unwrap();
    let raw = std::fs::read_dir(scratch.path()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>();
    assert_eq!(raw, [nfd], "the disk lists the decomposed name");
    let row = crate::navigator::children(&root).unwrap().remove(0);
    let typed = format!("{root}/{nfc}");
    assert_eq!(row, typed, "the row is spelt as the person types it");
    let image = canonical(&scratch.path().join(nfd).join("Roll 3").join("a.NEF"));
    assert!(holds(&typed, &image) && holds(&format!("{root}/{nfd}"), &image) && same(&format!("{root}/{nfd}"), &row));
    let mut stars = Favourites { version: 1, paths: vec![format!("{root}/{nfd}"), typed.clone()] }.respelt();
    assert_eq!(stars.paths, [typed.clone()], "one folder starred once, in one spelling");
    assert!(stars.contains(&row));
    stars.follow(&format!("{root}/{nfd}"), &format!("{root}/Hiver"));
    assert_eq!(stars.paths, [format!("{root}/Hiver")]);
    // Twin: compared byte for byte, the decomposed listing is outside its composed spelling.
    let listed = format!("{root}/{nfd}/Roll 3/a.NEF");
    assert!(!listed.starts_with(&format!("{typed}/")) && format!("{root}/{nfd}") != typed);
}

#[test]
fn case_folds_only_where_the_volume_ignores_it() {
    let blind = ignores_case("/");
    assert_eq!(holds("/Scans/roll 8", "/Scans/Roll 8/a.NEF"), blind);
    assert_eq!(same("/Scans/roll 8", "/Scans/ROLL 8"), blind);
    assert_eq!(holds("/Scans/\u{e9}t\u{e9}", "/Scans/\u{c9}T\u{c9}/a.NEF"), blind);
    // Twin: folded or not, a sibling sharing the prefix stays outside.
    assert!(!holds("/Scans/roll 8", "/Scans/Roll 80/a.NEF") && !holds("/Scans/roll", "/Scans/Roll 8"));
}
