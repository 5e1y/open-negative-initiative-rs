use super::*;
use crate::gallery_history::GalleryHistory;
use crate::rgb_passes;
use crate::scratch::Scratch;
use oni_settings::PipelineSettings;
use oni_settings::frame::RgbPasses;
use oni_settings::sidecar::{self, WriteOptions};
use std::collections::HashSet;

fn touch(path: &str, bytes: &[u8]) {
    std::fs::create_dir_all(paths::parent(path)).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn exists(path: &str) -> bool {
    Path::new(path).exists()
}

fn side(path: &str) -> String {
    crate::paths::normal(&sidecar::url_for(Path::new(path)).to_string_lossy()).into_owned()
}

fn root(scratch: &Scratch) -> String {
    crate::paths::normal(&scratch.path().to_string_lossy()).into_owned()
}

fn names(units: &[Unit]) -> Vec<String> {
    units.iter().flat_map(|u| u.moves.iter().map(|m| paths::name(&m.to))).collect()
}

fn listed(folder: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn a_roll_folder_takes_its_photos_and_gives_them_back() {
    let scratch = Scratch::new("roll-folder");
    let parent = root(&scratch);
    let photos: Vec<String> = ["a.RW2", "b.tif"].iter().map(|n| paths::join(&parent, n)).collect();
    for p in &photos {
        touch(p, p.as_bytes());
        touch(&side(p), b"{}");
    }
    let day = crate::relink::date("2026:01:14 12:00:00").unwrap();
    std::fs::create_dir(paths::join(&parent, "2026-01-14")).unwrap();
    let planned = plan(&photos, day, &parent);
    assert_eq!((planned.name.as_str(), planned.picked, planned.files, planned.absent), ("2026-01-14 2", 2, 4, 0));

    let made = carry_out(&planned, &rename_new).unwrap();
    let folder = made.disk.folder.clone().unwrap();
    assert_eq!(made.disk.moves.len(), 2);
    assert!(made.disk.moves.iter().all(|m| exists(&m.to) && exists(&side(&m.to))));
    assert_eq!(folders::roll(&folder).map(|f| f.id), Some(planned.film.id));
    assert!(!photos.iter().any(|p| exists(p)));

    let back = take_back(&made.disk, &rename_new);
    assert_eq!((back.made.len(), back.said.as_deref()), (2, None));
    assert!(photos.iter().all(|p| exists(p) && exists(&side(p))) && !exists(&folder));

    let redone = lay(&made.disk, &rename_new);
    assert!(redone.made.len() == 2 && !redone.refused);
    assert_eq!(folders::roll(&folder).map(|f| f.id), Some(planned.film.id));
    // Twin: a file put in the folder since keeps it, the photos still go back.
    let kept = paths::join(&folder, "notes.txt");
    touch(&kept, b"mine");
    let again = take_back(&made.disk, &rename_new);
    assert!(exists(&folder) && exists(&kept) && again.made.len() == 2);
}

#[test]
fn new_roll_takes_the_photos_of_one_folder_only() {
    assert_eq!(one_folder(&["/S/a.tif".into(), "/S/b.tif".into()]), Ok("/S".to_string()));
    let refused = one_folder(&["/S/a.tif".into(), "/S/Roll 2/b.tif".into(), "/T/c.tif".into()]).unwrap_err();
    assert!(refused.contains("these lie in 3 folders"), "{refused}");
}

#[test]
fn a_taken_name_is_numbered_as_the_finder_numbers_a_copy() {
    let scratch = Scratch::new("roll-folder-names");
    let root = root(&scratch);
    let p = |r: &str| paths::join(&root, r);
    for dir in ["Dest/Trip", "Elsewhere", "Other", "Trip"] {
        std::fs::create_dir_all(p(dir)).unwrap();
    }
    for file in ["Dest/x.RW2", "Elsewhere/x.RW2", "Elsewhere/y.RW2", "Other/y.RW2", "Trip/t.RW2"] {
        touch(&p(file), file.as_bytes());
    }
    touch(&side(&p("Trip/t.RW2")), b"{}");
    let items = [p("Elsewhere/x.RW2"), p("Elsewhere/y.RW2"), p("Other/y.RW2"), p("Trip")];
    let planned = plan_units(&items, &p("Dest"));
    assert_eq!(names(&planned), ["x 2.RW2", "y.RW2", "y 2.RW2", "Trip 2"]);
    // Twin: kept on their own names, two collide and one is taken.
    let plain: Vec<String> = items.iter().map(|i| paths::join(&p("Dest"), &paths::name(i))).collect();
    assert!(plain.iter().collect::<HashSet<_>>().len() < plain.len() && plain.iter().any(|x| exists(x)));
    let carried = carry(&planned, &rename_new);
    assert_eq!(carried.moves().len(), 4);
    assert!(exists(&p("Dest/Trip 2/t.RW2")) && exists(&side(&p("Dest/Trip 2/t.RW2"))));
    let disk = Disk { moves: carried.moves(), units: carried.sizes(), ..Disk::default() };
    let back = take_back(&disk, &rename_new);
    assert!(back.made.len() == 4 && exists(&p("Dest/x.RW2")) && items.iter().all(|i| exists(i)));
}

fn triplet(root: &str) -> [String; 3] {
    let p = |r: &str| paths::join(root, r);
    let triplet = [p("Passes/b.ARW"), p("Passes/g.ARW"), p("Passes/r.ARW")];
    for f in &triplet {
        touch(f, f.as_bytes());
    }
    let passes = RgbPasses { red: "r.ARW".into(), green: "g.ARW".into(), blue: "b.ARW".into(), registration: None };
    let lead = PipelineSettings { rgb_passes: Some(passes), ..PipelineSettings::default() };
    sidecar::write(&lead, Path::new(&triplet[0]), WriteOptions::default()).unwrap();
    for f in &triplet[1..] {
        let pass = PipelineSettings { pass_of: Some("b.ARW".into()), ..PipelineSettings::default() };
        sidecar::write(&pass, Path::new(f), WriteOptions::default()).unwrap();
    }
    triplet
}

#[test]
fn a_triplet_moves_whole_on_its_names_or_stays_whole() {
    let scratch = Scratch::new("roll-folder-passes");
    let root = root(&scratch);
    let p = |r: &str| paths::join(&root, r);
    let triplet = triplet(&root);
    let order = vec![triplet[2].clone(), triplet[1].clone(), triplet[0].clone()];
    assert_eq!(rgb_passes::expanded(&[triplet[0].clone()]), order);
    touch(&p("Dest/g.ARW"), b"taken");
    assert!(plan_units(&triplet, &p("Dest")).is_empty(), "one taken name keeps the three where they are");
    touch(&side(&p("Taken/r.ARW")), b"{}");
    assert!(plan_units(&triplet, &p("Taken")).is_empty(), "a sidecar's name taken keeps them too");
    // Twin: with every name free, the three move on their own names, as one frame.
    let clear = plan_units(&triplet[..1], &p("Clear"));
    assert_eq!(clear.len(), 1);
    assert_eq!(names(&clear), ["r.ARW", "g.ARW", "b.ARW"]);
}

/// The green pass's rename is refused halfway: the passes and sidecars already moved come back,
/// and nothing of the frame is left in the roll.
#[test]
fn a_triplet_moves_whole_or_not_at_all() {
    let scratch = Scratch::new("roll-folder-halfway");
    let root = root(&scratch);
    let triplet = triplet(&root);
    let into = paths::join(&root, "Roll");
    std::fs::create_dir_all(&into).unwrap();
    let units = plan_units(&triplet, &into);
    assert_eq!(files(&units), 6, "three passes and their three sidecars");
    let locked = |from: &Path, to: &Path| {
        if from.file_name().is_some_and(|n| n == "g.ARW") {
            return Err(io::Error::new(io::ErrorKind::ResourceBusy, "in use"));
        }
        rename_new(from, to)
    };
    let carried = carry(&units, &locked);
    assert!(carried.made.is_empty());
    assert_eq!(carried.stayed.len(), 1);
    assert!(said(&carried).unwrap().contains("g.ARW: another app is using it"), "{:?}", said(&carried));
    assert!(triplet.iter().all(|f| exists(f) && exists(&side(f))), "the three and their sidecars are home");
    assert_eq!(listed(&into), Vec::<String>::new(), "nothing of the frame in the roll");
    // Twin: a rename that takes every file moves the three with their sidecars.
    let whole = carry(&units, &rename_new);
    assert_eq!(whole.moves().len(), 3);
    assert_eq!(listed(&into).len(), 6);
}

/// A sidecar already where a photo's would go is a taken name: the photo is numbered past it when
/// planned, and stays when its place is fixed, the stray file untouched either way.
#[test]
fn a_stray_sidecar_at_the_destination_is_a_taken_name() {
    let scratch = Scratch::new("roll-folder-stray");
    let root = root(&scratch);
    let (a, b) = (paths::join(&root, "a.RW2"), paths::join(&root, "b.RW2"));
    for p in [&a, &b] {
        touch(p, b"frame");
        touch(&side(p), format!("{{\"{}\": 1}}", paths::name(p)).as_bytes());
    }
    let into = paths::join(&root, "Roll");
    let stray = side(&paths::join(&into, "a.RW2"));
    touch(&stray, b"{\"stray\": 1}");
    assert_eq!(names(&plan_units(std::slice::from_ref(&a), &into)), ["a 2.RW2"], "planned past the stray");
    let fixed = [Unit { moves: vec![Move { from: a.clone(), to: paths::join(&into, "a.RW2") }] }, Unit { moves: vec![Move { from: b.clone(), to: paths::join(&into, "b.RW2") }] }];
    let carried = carry(&fixed, &rename_new);
    assert_eq!(carried.moves().len(), 1, "b moves, a stays");
    assert!(exists(&a) && std::fs::read(side(&a)).unwrap() == b"{\"a.RW2\": 1}", "a stayed with its settings");
    assert_eq!(std::fs::read(&stray).unwrap(), b"{\"stray\": 1}", "the stray file is untouched");
    assert!(carried.stayed[0].1.contains("is already there"));
    // Twin: the plain rename replaces the stray without a word, which is what the look before it prevents.
    touch(&paths::join(&root, "c.RW2.json"), b"{\"c\": 1}");
    std::fs::rename(paths::join(&root, "c.RW2.json"), &stray).unwrap();
    assert_eq!(std::fs::read(&stray).unwrap(), b"{\"c\": 1}");
}

#[test]
fn rename_new_never_replaces_a_file() {
    let scratch = Scratch::new("roll-folder-excl");
    let root = root(&scratch);
    let (from, to, free) = (paths::join(&root, "a"), paths::join(&root, "B"), paths::join(&root, "c"));
    touch(&from, b"a");
    touch(&to, b"b");
    let refused = rename_new(Path::new(&from), Path::new(&to)).unwrap_err();
    assert_eq!(refused.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&to).unwrap(), b"b");
    // Twin: a free name takes the file.
    rename_new(Path::new(&from), Path::new(&free)).unwrap();
    assert!(exists(&free) && !exists(&from));
}

/// darktable's, Lightroom's and RawTherapee's sidecars and the JPEG of a RAW+JPEG pair go with the
/// RAW, numbered as it is, and come back with it.
#[test]
fn a_raw_takes_its_jpeg_and_the_sidecars_of_other_apps() {
    let scratch = Scratch::new("roll-folder-companions");
    let root = root(&scratch);
    let p = |r: &str| paths::join(&root, r);
    for f in ["DSC_1.NEF", "DSC_1.NEF.json", "DSC_1.NEF.xmp", "DSC_1.XMP", "DSC_1.NEF.pp3", "DSC_1.JPG", "DSC_1.JPG.json", "DSC_2.NEF", "other.xmp"] {
        touch(&p(f), f.as_bytes());
    }
    touch(&p("Roll/DSC_1.JPG"), b"taken");
    let units = plan_units(&[p("DSC_1.NEF"), p("DSC_2.NEF")], &p("Roll"));
    assert_eq!(names(&units), ["DSC_1 2.NEF", "DSC_1 2.JPG", "DSC_2.NEF"], "the pair numbered together");
    assert_eq!(files(&units), 8);
    let both = plan_units(&[p("DSC_1.JPG"), p("DSC_1.NEF")], &p("Roll"));
    assert_eq!((both.len(), files(&both)), (1, 7), "the JPEG picked too is the same frame, its RAW joining it");
    let carried = carry(&units, &rename_new);
    assert_eq!(carried.moves().len(), 3);
    for f in ["DSC_1 2.NEF", "DSC_1 2.NEF.json", "DSC_1 2.NEF.xmp", "DSC_1 2.XMP", "DSC_1 2.NEF.pp3", "DSC_1 2.JPG", "DSC_1 2.JPG.json"] {
        assert!(exists(&p(&format!("Roll/{f}"))), "{f} went with its RAW");
    }
    assert!(exists(&p("other.xmp")), "a sidecar of no photo picked stays");
    let back = take_back(&Disk { moves: carried.moves(), units: carried.sizes(), ..Disk::default() }, &rename_new);
    assert_eq!(back.made.len(), 3);
    assert!(["DSC_1.NEF.xmp", "DSC_1.XMP", "DSC_1.NEF.pp3", "DSC_1.JPG.json"].iter().all(|f| exists(&p(f))), "they came back with it");
    // Twin: a TIFF has no pair, and a stem's `.xmp` stays with no RAW to follow.
    touch(&p("scan.tif"), b"t");
    touch(&p("scan.xmp"), b"x");
    touch(&p("scan.jpg"), b"j");
    assert_eq!(names(&plan_units(&[p("scan.tif")], &p("Roll"))), ["scan.tif"]);
    assert_eq!(files(&plan_units(&[p("scan.tif")], &p("Roll"))), 1);
}

/// A folder made meanwhile under the roll's name is never taken over: the redo moves nothing,
/// writes no marker, and its step stays to redo.
#[test]
fn redo_into_a_folder_made_meanwhile_is_refused() {
    let scratch = Scratch::new("roll-folder-redo");
    let parent = root(&scratch);
    let photo = paths::join(&parent, "a.tif");
    touch(&photo, b"a");
    let planned = plan(std::slice::from_ref(&photo), crate::relink::date("2026:01:14 12:00:00").unwrap(), &parent);
    let made = carry_out(&planned, &rename_new).unwrap();
    let mut history = GalleryHistory::default();
    history.record_disk(made.disk.clone(), "New Roll");
    let step = history.undo_step().unwrap();
    assert!(take_back(step.disk.as_ref().unwrap(), &rename_new).said.is_none());
    let folder = made.disk.folder.clone().unwrap();
    std::fs::create_dir(&folder).unwrap();
    touch(&paths::join(&folder, "mine.txt"), b"mine");
    let step = history.redo_step().unwrap();
    let redo = lay(step.disk.as_ref().unwrap(), &rename_new);
    assert!(redo.refused && redo.made.is_empty());
    assert!(redo.said.as_deref().unwrap().contains("a folder “2026-01-14” was made"), "{:?}", redo.said);
    assert!(exists(&photo) && !exists(&folders::marker(&folder).to_string_lossy()), "nothing moved, no marker written");
    history.give_back(false);
    assert!(history.can_redo(), "the redo is offered again");
    // Twin: the roll's own folder, left by an undo that found a file in it, takes the photo again.
    let mine = Path::new(&folder).join("mine.txt");
    std::fs::remove_file(mine).unwrap();
    std::fs::remove_dir(&folder).unwrap();
    let step = history.redo_step().unwrap();
    assert_eq!(lay(step.disk.as_ref().unwrap(), &rename_new).made.len(), 1);
    touch(&paths::join(&folder, "notes.txt"), b"n");
    assert!(take_back(&made.disk, &rename_new).said.is_none() && exists(&folder));
    let notes = Path::new(&folder).join("notes.txt");
    std::fs::remove_file(notes).unwrap();
    let again = lay(&made.disk, &rename_new);
    assert!(!again.refused && again.made.len() == 1, "{:?}", again.said);
}

/// A disk image formatted exFAT like the T7, attached in the scratch folder, detached on drop.
#[cfg(target_os = "macos")]
struct Exfat(std::path::PathBuf);

#[cfg(target_os = "macos")]
impl Exfat {
    fn attach(scratch: &Scratch) -> Option<Exfat> {
        use std::process::Command;
        let (image, mount) = (scratch.path().join("t7.dmg"), scratch.path().join("T7"));
        let run = |c: &mut Command| c.status().is_ok_and(|s| s.success());
        let made = run(Command::new("hdiutil").args(["create", "-quiet", "-size", "8m", "-fs", "ExFAT", "-volname", "T7", "-layout", "NONE"]).arg(&image));
        std::fs::create_dir_all(&mount).ok()?;
        let attached = made && run(Command::new("hdiutil").args(["attach", "-quiet", "-nobrowse", "-noverify", "-mountpoint"]).arg(&mount).arg(&image));
        attached.then_some(Exfat(mount))
    }
}

#[cfg(target_os = "macos")]
impl Drop for Exfat {
    fn drop(&mut self) {
        let _ = std::process::Command::new("hdiutil").args(["detach", "-quiet", "-force"]).arg(&self.0).status();
    }
}

#[cfg(target_os = "macos")]
fn tag(path: &str) {
    let (name, value) = (c"com.oni.check", b"oni");
    let path = std::ffi::CString::new(path).unwrap();
    // SAFETY: NUL-terminated strings and a buffer of the length given, all outliving the call.
    unsafe { libc::setxattr(path.as_ptr(), name.as_ptr(), value.as_ptr().cast(), value.len(), 0, 0) };
}

/// On exFAT macOS lays a `._` twin beside each file it tags, the roll's marker included: the undo
/// still finds the folder empty of anything of the user's and removes it with its twins.
#[cfg(target_os = "macos")]
#[test]
fn undo_on_exfat_removes_the_folder_and_its_appledouble() {
    let scratch = Scratch::new("roll-folder-exfat");
    let Some(disk) = Exfat::attach(&scratch) else {
        eprintln!("hdiutil refused an exFAT image: the undo on exFAT is not exercised");
        return;
    };
    let parent = crate::paths::canonical(&disk.0);
    let photo = paths::join(&parent, "a.tif");
    touch(&photo, b"a");
    touch(&side(&photo), b"{}");
    let planned = plan(std::slice::from_ref(&photo), crate::relink::date("2026:01:14 12:00:00").unwrap(), &parent);
    let made = carry_out(&planned, &rename_new).unwrap();
    let folder = made.disk.folder.clone().unwrap();
    // Each file tagged as macOS tags what an app writes, which lays the `._` twins seen on the T7.
    for f in [folders::marker(&folder).to_string_lossy().into_owned(), made.disk.moves[0].to.clone()] {
        tag(&f);
    }
    let back = take_back(&made.disk, &rename_new);
    assert_eq!(back.made.len(), 1);
    assert!(exists(&photo) && exists(&side(&photo)), "the photo and its sidecar are home");
    assert!(!exists(&folder), "the folder went with its marker and their twins");
    // Twin: the same listing read literally, the marker alone, keeps the folder.
    let laid = lay(&made.disk, &rename_new);
    assert_eq!(laid.made.len(), 1);
    let left = carry(&[Unit { moves: made.disk.moves.iter().map(|m| Move { from: m.to.clone(), to: m.from.clone() }).collect() }], &rename_new);
    assert_eq!(left.moves().len(), 1);
    let names = listed(&folder);
    assert!(names.iter().any(|n| n.starts_with("._")), "exFAT lays a `._` twin: {names:?}");
    assert!(!names.iter().all(|n| n == folders::ROLL_MARKER), "the literal check finds the folder not empty");
    assert!(names.iter().all(|n| n == folders::ROLL_MARKER || leftover(n)));
    assert!(remove_if_left(&folder, made.disk.film.as_ref()) && !exists(&folder));
}

#[test]
fn a_locked_file_is_retried_then_named() {
    use crate::names::Platform::{Mac, Windows};
    use std::sync::Mutex;
    let sharing = || io::Error::from_raw_os_error(32);
    // A hold lifted on the third try: two pauses, then the move.
    let tries = Mutex::new(0);
    let waited = Mutex::new(Vec::new());
    let lifts = |_: &Path, _: &Path| {
        let mut n = tries.lock().unwrap();
        *n += 1;
        if *n < 3 { Err(sharing()) } else { Ok(()) }
    };
    let pause = |d: std::time::Duration| waited.lock().unwrap().push(d);
    assert!(retried(Path::new("/a"), Path::new("/b"), &lifts, &pause, Windows).is_ok());
    assert_eq!((*tries.lock().unwrap(), waited.lock().unwrap().clone()), (3, PAUSES[..2].to_vec()));

    // A hold never lifted: every pause spent, then the frame stays and its file is named.
    let scratch = Scratch::new("roll-held");
    let parent = root(&scratch);
    let photo = paths::join(&parent, "DSC_0001.NEF");
    touch(&photo, b"raw");
    let units = plan_units(std::slice::from_ref(&photo), &paths::join(&parent, "Roll"));
    std::fs::create_dir(paths::join(&parent, "Roll")).unwrap();
    let held = |f: &Path, t: &Path| retried(f, t, &|_: &Path, _: &Path| Err(io::Error::from(io::ErrorKind::ResourceBusy)), &|_| {}, Windows);
    let carried = carry(&units, &held);
    let said = said(&carried).unwrap_or_default();
    assert!(carried.made.is_empty() && exists(&photo), "nothing moved");
    assert_eq!(said, "“DSC_0001.NEF” stayed where it was: DSC_0001.NEF: another app is using it.");

    // Twin: a refusal that is no hold is said at once, and a Mac reads no Windows code as one.
    for (error, platform) in [(io::Error::from(io::ErrorKind::NotFound), Windows), (sharing(), Mac), (io::Error::from_raw_os_error(5), Mac)] {
        let calls = Mutex::new(0);
        let refuses = |_: &Path, _: &Path| {
            *calls.lock().unwrap() += 1;
            Err(io::Error::from_raw_os_error(error.raw_os_error().unwrap_or(2)))
        };
        let _ = retried(Path::new("/a"), Path::new("/b"), &refuses, &|_| panic!("no pause"), platform);
        assert_eq!(*calls.lock().unwrap(), 1, "{error:?} on {platform:?}");
    }
    // Twin: Windows' denial on a folder with a file open inside is tried again.
    let denials = Mutex::new(0);
    let denies = |_: &Path, _: &Path| {
        *denials.lock().unwrap() += 1;
        Err(io::Error::from_raw_os_error(5))
    };
    assert!(retried(Path::new("/a"), Path::new("/b"), &denies, &|_| {}, Windows).is_err());
    assert_eq!(*denials.lock().unwrap(), PAUSES.len() + 1);
}
