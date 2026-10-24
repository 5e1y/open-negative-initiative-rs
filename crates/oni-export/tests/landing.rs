//! Where an export lands: never on an original, always inside the chosen folder, and a failed
//! write keeps the file it would have replaced. Each against its twin.

use std::path::{Path, PathBuf};

use oni_colour::PaperCorrection;
use oni_export::file::{self, Ledger, Refused, Standing};
use oni_export::landing::{Clash, plan};
use oni_export::{ExifMetadata, Failure, Format, Image, Landing, Settings, destination, write};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-export-landing-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn frame() -> Result<Image, Failure> {
    Ok(Image { width: 8, height: 6, pixels: vec![0.4; 8 * 6 * 4] })
}

fn export(source: &Path, path: &Path, ledger: &mut Ledger, replacing: bool) -> Result<(), Failure> {
    let to = Landing { path, ledger, replacing };
    write(source, to, frame, &PaperCorrection::default(), &Settings::default(), &ExifMetadata::default(), None)
}

fn is_jpeg(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|b| b.starts_with(&[0xFF, 0xD8]))
}

/// No file but the ones named, no staged file left behind.
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn an_export_never_lands_on_another_original() {
    let dir = scratch("original");
    let support = dir.join("Support");
    let roll = dir.join("Roll");
    std::fs::create_dir_all(&roll).unwrap();
    let (nef, jpg) = (roll.join("P1.NEF"), roll.join("P1.JPG"));
    std::fs::write(&nef, b"raw").unwrap();
    std::fs::write(&jpg, b"the camera's own jpeg").unwrap();
    let ignores_case = roll.join("p1.jpg").exists();
    let ledger = Ledger::in_support(&support);

    let named = vec![destination(&roll, &nef, "", Format::Jpeg)];
    assert_eq!(named[0], roll.join("P1.jpg"));
    let found = plan(&named, &[nef.clone()], &ledger);
    let landed = found.renamed()[0].clone();
    if ignores_case {
        assert_eq!(found.clashes, [Clash::Original], "P1.jpg is P1.JPG on this volume");
        assert_eq!(landed, roll.join("P1 2.jpg"));
        // Past the plan, the write itself refuses the original, Replace or not.
        for replacing in [false, true] {
            assert_eq!(export(&nef, &named[0], &mut Ledger::in_support(&support), replacing), Err(Failure::WouldOverwriteOriginal));
        }
    } else {
        assert_eq!(found.clashes, [Clash::None], "P1.jpg is a file of its own on this volume");
    }
    export(&nef, &landed, &mut Ledger::in_support(&support), false).unwrap();
    assert_eq!(std::fs::read(&jpg).unwrap(), b"the camera's own jpeg", "the original kept byte for byte");
    assert!(is_jpeg(&landed));

    // A frame's own source, and a name the batch gives twice, never land either.
    let tif = roll.join("P1.TIF");
    std::fs::write(&tif, b"scan").unwrap();
    let twice = vec![roll.join("Q.jpg"), roll.join("q.JPG")];
    assert_eq!(plan(&twice, &[nef.clone(), tif.clone()], &ledger).clashes, [Clash::None, Clash::Twice]);
    assert_eq!(plan(&[tif.clone()], &[tif.clone()], &ledger).clashes, [Clash::Original]);
    assert_eq!(export(&tif, &tif, &mut Ledger::in_support(&support), true), Err(Failure::WouldOverwriteOriginal));
    assert_eq!(std::fs::read(&tif).unwrap(), b"scan");

    // An earlier export is offered for Replace while it stands as written, never once changed.
    let ledger = Ledger::in_support(&support);
    assert_eq!(file::standing(&landed, &ledger), Standing::Export);
    assert_eq!(plan(std::slice::from_ref(&landed), &[nef.clone()], &ledger).clashes, [Clash::Export]);
    export(&nef, &landed, &mut Ledger::in_support(&support), true).unwrap();
    assert_eq!(file::standing(&landed, &Ledger::in_support(&support)), Standing::Export, "a replaced export is still ONI's");
    std::fs::write(&landed, b"another app saved over it").unwrap();
    assert_eq!(file::standing(&landed, &Ledger::in_support(&support)), Standing::Original);
    assert_eq!(file::standing(&landed, &Ledger::none()), Standing::Original);
    assert!(!listing(&roll).iter().any(|n| n.ends_with(".tmp")), "{:?}", listing(&roll));

    // Twin: a folder without P1.JPG receives P1.jpg as named.
    let other = dir.join("Other");
    std::fs::create_dir_all(&other).unwrap();
    let nef = other.join("P1.NEF");
    std::fs::write(&nef, b"raw").unwrap();
    let named = vec![destination(&other, &nef, "", Format::Jpeg)];
    let found = plan(&named, &[nef.clone()], &Ledger::in_support(&support));
    assert_eq!(found.clashes, [Clash::None]);
    export(&nef, &found.renamed()[0], &mut Ledger::in_support(&support), false).unwrap();
    assert_eq!(listing(&other), ["P1.NEF", "P1.jpg"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_arriving_during_the_write_is_never_replaced() {
    let dir = scratch("arriving");
    let path = dir.join("A.jpg");
    std::fs::write(&path, b"arrived").unwrap();
    assert!(matches!(file::create(&path, b"ours"), Err(Refused::Taken)));
    assert!(matches!(file::put(&path, b"ours", &Ledger::none(), true), Err(Refused::Taken)));
    assert_eq!(std::fs::read(&path).unwrap(), b"arrived");
    assert_eq!(listing(&dir), ["A.jpg"], "nothing staged is left behind");
    // Twin: the same name free is written whole.
    std::fs::remove_file(&path).unwrap();
    file::create(&path, b"ours").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"ours");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_typed_name_stays_in_the_chosen_folder() {
    let folder = Path::new("/r/chosen");
    let source = Path::new("/r/roll/P1.NEF");
    for (typed, expected) in [
        ("..\\x", "..-x.jpg"),
        ("../../Best/Frame 3", "..-..-Best-Frame 3.jpg"),
        ("CON", "CON-.jpg"),
        ("nul", "nul-.jpg"),
        ("a?b", "a-b.jpg"),
        ("<|*\">", "-----.jpg"),
        ("Frame 3. ", "Frame 3.jpg"),
        ("..", "P1.jpg"),
        ("/etc/passwd", "-etc-passwd.jpg"),
    ] {
        let landed = destination(folder, source, typed, Format::Jpeg);
        assert_eq!(landed.parent(), Some(folder), "{typed}");
        assert_eq!(landed.file_name().unwrap().to_string_lossy(), expected, "{typed}");
    }
    // Twin: the same text joined as typed climbs out of the folder.
    let raw = folder.join("../../Best/Frame 3.jpg");
    assert_ne!(raw.parent(), Some(folder));
}

#[cfg(unix)]
#[test]
fn a_failed_export_keeps_the_previous_one() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("failed");
    let support = dir.join("Support");
    let out = dir.join("Out");
    std::fs::create_dir_all(&out).unwrap();
    let source = dir.join("P1.NEF");
    std::fs::write(&source, b"raw").unwrap();
    let path = out.join("P1.jpg");
    export(&source, &path, &mut Ledger::in_support(&support), false).unwrap();
    let before = std::fs::read(&path).unwrap();

    // A render that fails writes nothing.
    let mut ledger = Ledger::in_support(&support);
    let to = Landing { path: &path, ledger: &mut ledger, replacing: true };
    let failed = write(&source, to, || Err(Failure::Render), &PaperCorrection::default(), &Settings::default(), &ExifMetadata::default(), None);
    assert_eq!(failed, Err(Failure::Render));
    assert_eq!(std::fs::read(&path).unwrap(), before);

    // A folder that refuses the write keeps the earlier export, and the notice says why.
    std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o555)).unwrap();
    let mut bright = Settings::default();
    bright.jpeg_quality = 0.3;
    let mut ledger = Ledger::in_support(&support);
    let to = Landing { path: &path, ledger: &mut ledger, replacing: true };
    let refused = write(&source, to, frame, &PaperCorrection::default(), &bright, &ExifMetadata::default(), None);
    std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(refused, Err(Failure::Write(std::io::ErrorKind::PermissionDenied)));
    assert!(refused.unwrap_err().to_string().contains("not allowed"));
    assert_eq!(std::fs::read(&path).unwrap(), before, "the earlier export kept whole");
    assert_eq!(listing(&out), ["P1.jpg"], "nothing staged is left behind");

    // Twin: the same write in a folder that accepts it replaces the file.
    let mut ledger = Ledger::in_support(&support);
    let to = Landing { path: &path, ledger: &mut ledger, replacing: true };
    write(&source, to, frame, &PaperCorrection::default(), &bright, &ExifMetadata::default(), None).unwrap();
    assert_ne!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}
