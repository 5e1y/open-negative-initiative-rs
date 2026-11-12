use super::*;
use crate::preferences::{self, Preferences};
use crate::thumbnails;
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// An empty folder of this check's own, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Scratch {
        let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let folder = std::env::temp_dir().join(format!("oni-app-migration-{label}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        Scratch(folder)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn aged(path: &Path, by: u64) {
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(by)).unwrap();
}

const DATED: &str = "30046636-2026:09:14 16:47:13-e4e6a64ce12647a6";
const OLDER: &str = "25331200-2025:01:02 03:04:05-77";

/// The old names of two dated tiles, the portable name of the second written `later` seconds after
/// its old name, and a file that is no tile.
fn tiles(folder: &Path, later: i64) {
    std::fs::create_dir_all(folder).unwrap();
    std::fs::write(folder.join(format!("{DATED}.jpg")), b"dated").unwrap();
    std::fs::write(folder.join(format!("{OLDER}.jpg")), b"old name").unwrap();
    std::fs::write(folder.join(thumbnails::file_name(OLDER)), b"redrawn").unwrap();
    aged(&folder.join(format!("{OLDER}.jpg")), (120 + later) as u64);
    aged(&folder.join(thumbnails::file_name(OLDER)), 120);
    std::fs::write(folder.join("notes: kept.txt"), b"not a tile").unwrap();
}

#[test]
fn a_tile_under_its_old_name_takes_the_portable_one_unless_a_newer_stands_there() {
    // Windows never wrote a colon in a name, nor takes one.
    if cfg!(windows) {
        return;
    }
    let scratch = Scratch::new("tiles");
    let folder = scratch.0.join("Thumbnails");
    tiles(&folder, 60);
    assert_eq!(tiles::renamed(&folder), 1);
    assert_eq!(std::fs::read(folder.join(thumbnails::file_name(DATED))).unwrap(), b"dated");
    assert!(!folder.join(format!("{DATED}.jpg")).exists() && !folder.join(format!("{OLDER}.jpg")).exists());
    assert_eq!(std::fs::read(folder.join(thumbnails::file_name(OLDER))).unwrap(), b"redrawn", "the newer tile wins");
    assert!(folder.join("notes: kept.txt").exists(), "a file that is no tile stays");
    assert_eq!(tiles::renamed(&folder), 0, "a second pass finds nothing to rename");
    // Twin: the old-named tile the newer one, the rename overwrites, and the bytes tell.
    let twin = scratch.0.join("Twin");
    tiles(&twin, -60);
    assert_eq!(tiles::renamed(&twin), 2);
    assert_eq!(std::fs::read(twin.join(thumbnails::file_name(OLDER))).unwrap(), b"old name");
}

#[test]
fn a_tile_is_found_only_under_its_portable_name() {
    if cfg!(windows) {
        return;
    }
    let scratch = Scratch::new("held");
    let support = scratch.0.join("Support");
    std::fs::create_dir_all(thumbnails::directory(&support)).unwrap();
    std::fs::write(thumbnails::directory(&support).join(format!("{DATED}.jpg")), b"dated").unwrap();
    // Twin: with the tiles not renamed, the old name is not found, so the tile would be drawn again.
    assert!(!thumbnails::is_held(&support, DATED));
    brought(&support, None);
    assert!(thumbnails::is_held(&support, DATED), "the rename is what keeps the tile");
}

fn fixture(scratch: &Scratch, edit: impl Fn(String) -> String) -> PathBuf {
    let xml = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/former-preferences.plist")).unwrap();
    let (source, binary) = (scratch.0.join("former.xml"), scratch.0.join("former.plist"));
    std::fs::write(&source, edit(xml)).unwrap();
    let converted = std::process::Command::new("/usr/bin/plutil").args(["-convert", "binary1", "-o"]).arg(&binary).arg(&source).status().unwrap();
    assert!(converted.success());
    binary
}

#[cfg(target_os = "macos")]
#[test]
fn the_former_preferences_land_once_and_only_where_this_line_never_ran() {
    let scratch = Scratch::new("defaults");
    let support = scratch.0.join("Support");
    let plist = fixture(&scratch, |xml| xml);
    assert_eq!(defaults::import(&support, &plist), 8);
    let read = Preferences::load(&support);
    let settings = read.string(oni_export::settings::PREFERENCE_KEY).and_then(oni_export::Settings::from_raw_value).unwrap();
    assert_eq!(settings.format(), oni_export::Format::Tiff16);
    assert_eq!(read.string(crate::app::export_card::ARTIST), Some("Ada Lovelace"));
    assert_eq!(read.string(crate::app::export_card::COPYRIGHT), Some("© Ada Lovelace"));
    assert_eq!(read.string(crate::app::export_card::LAST_PROFILE), Some("/Library/ColorSync/Profiles/Paper.icc"));
    assert_eq!(read.value(preferences::BRUSH_RADIUS).and_then(Value::as_f64), Some(11.6409));
    assert_eq!((read.bool(preferences::SOUND_MUTED), read.string(crate::updates::LAST_LAUNCHED_KEY)), (Some(false), Some("1.3.1")));
    assert_eq!(read.int(preferences::GALLERY_COLUMNS), Some(4));
    assert!(read.value("NSWindow Frame Main").is_none(), "a key not asked stays out");
    assert_eq!(defaults::import(&support, &plist), 0, "nothing taken twice");
    // Where a build of this line ran, its preferences file or its last log stands: none is taken, so
    // a key it never set keeps reading as its default.
    for (ran, mark) in [("Ran", Preferences::file_path(Path::new(""))), ("Logged", crate::timing::log_path(Path::new("")).with_file_name(crate::timing::PREVIOUS_LOG))] {
        let support = scratch.0.join(ran);
        std::fs::create_dir_all(support.join(&mark).parent().unwrap()).unwrap();
        std::fs::write(support.join(&mark), b"{}").unwrap();
        assert_eq!(defaults::import(&support, &plist), 0, "{ran}");
        assert!(Preferences::load(&support).value(oni_export::settings::PREFERENCE_KEY).is_none(), "{ran}: the export as it was");
    }
    // Twin: the values taken laid where this line ran change the export it had.
    let ran = scratch.0.join("Ran");
    let mut prefs = Preferences::load(&ran);
    for (key, value) in defaults::taken(&plist) {
        prefs.set(key, value).unwrap();
    }
    assert!(Preferences::load(&ran).value(oni_export::settings::PREFERENCE_KEY).is_some());
}

#[cfg(target_os = "macos")]
#[test]
fn a_former_value_onis_readers_refuse_is_left_out() {
    let scratch = Scratch::new("defaults-checked");
    let support = scratch.0.join("Support");
    let plist = fixture(&scratch, |xml| {
        let xml = xml.replace(r#"{"compressTIFF":true,"format":"tiff16","hdr":false,"jpegQuality":1,"resizeEdge":"long","resizePixels":4000,"space":"adobergb"}"#, "Web");
        xml.replace("<integer>4</integer>", "<integer>40</integer>").replace("<string>1.3.1</string>", "<string></string>")
    });
    assert_eq!(defaults::import(&support, &plist), 5);
    let read = Preferences::load(&support);
    assert!(read.value(oni_export::settings::PREFERENCE_KEY).is_none(), "not the export's document");
    assert!(read.value(preferences::GALLERY_COLUMNS).is_none() && read.value(crate::updates::LAST_LAUNCHED_KEY).is_none());
    // Twin: the file holds each value, which only the check keeps out.
    assert_eq!(defaults::extracted(&plist, oni_export::settings::PREFERENCE_KEY, "string"), Some(Value::from("Web")));
    assert_eq!(defaults::extracted(&plist, preferences::GALLERY_COLUMNS, "integer"), Some(Value::from(40)));
    // A file absent imports nothing and says nothing.
    assert_eq!(defaults::import(&scratch.0.join("Other"), &scratch.0.join("absent.plist")), 0);
}

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

#[test]
fn a_second_launch_writes_nothing() {
    let scratch = Scratch::new("ledger");
    let support = scratch.0.join("Support");
    std::fs::create_dir_all(&support).unwrap();
    std::fs::write(support.join("favourites.json"), br#"{"version":1,"paths":["\/Volumes\/T7\/Scans"]}"#).unwrap();
    std::fs::write(support.join("projects.json"), br#"{"version":1,"projects":[]}"#).unwrap();
    assert_eq!(brought(&support, None), Brought { tell: true, ..Brought::default() });
    told(&support);
    assert_eq!(ledger::read(&support).unwrap().support, SUPPORT);
    let files = [ledger::path(&support), oni_library::places::Places::file_path(&support), oni_library::project::file_path(&support)];
    let stamps: Vec<SystemTime> = files.iter().map(|f| modified(f)).collect();
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(brought(&support, None), Brought::default());
    assert_eq!(files.iter().map(|f| modified(f)).collect::<Vec<_>>(), stamps, "nothing written again");
    // Twin: a ledger that forgot the step runs it again and writes the ledger.
    ledger::write(&support, &Ledger { told: oni_settings::document::VERSION, ..Ledger::default() }).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    brought(&support, None);
    assert_ne!(modified(&files[0]), stamps[0]);
}

#[test]
fn a_failed_import_stays_due_writes_nothing_and_is_said() {
    let scratch = Scratch::new("due");
    let support = scratch.0.join("Support");
    // Folders at the files' names: the disk refuses to read them, as a permission withheld does.
    std::fs::create_dir_all(support.join("favourites.json")).unwrap();
    std::fs::create_dir_all(support.join("projects.json")).unwrap();
    let due = Brought { places_due: true, projects_due: true, notice: Some(NOT_BROUGHT), tell: false };
    assert_eq!(brought(&support, None), Brought { tell: true, ..due }, "the new format said once, to a folder holding files");
    told(&support);
    assert_eq!(ledger::read(&support).unwrap().support, 0, "the ledger holds the steps due");
    assert!(!oni_library::places::Places::file_path(&support).exists() && !oni_library::project::file_path(&support).exists());
    assert_eq!(brought(&support, None), due, "tried and said at each launch");
    let readable = |support: &Path| {
        std::fs::remove_dir(support.join("favourites.json")).unwrap();
        std::fs::remove_dir(support.join("projects.json")).unwrap();
        std::fs::write(support.join("favourites.json"), br#"{"version":1,"paths":["/Volumes/T7/Scans"]}"#).unwrap();
        std::fs::write(support.join("projects.json"), br#"{"version":1,"projects":[]}"#).unwrap();
    };
    readable(&support);
    assert_eq!(brought(&support, None), Brought::default());
    assert_eq!(oni_library::places::Places::load(&support).unwrap().paths(), ["/Volumes/T7/Scans"], "brought once it reads");
    assert_eq!(ledger::read(&support).unwrap().support, SUPPORT);
    // Twin: a ledger advanced over the failure, as when a refused file counted as none, never brings it.
    let twin = scratch.0.join("Twin");
    std::fs::create_dir_all(twin.join("favourites.json")).unwrap();
    std::fs::create_dir_all(twin.join("projects.json")).unwrap();
    brought(&twin, None);
    ledger::write(&twin, &Ledger { support: SUPPORT, ..Ledger::default() }).unwrap();
    readable(&twin);
    brought(&twin, None);
    assert_eq!(oni_library::places::Places::load(&twin), None);
}

#[test]
fn an_old_file_that_does_not_parse_is_said_once() {
    let scratch = Scratch::new("unparsed");
    let support = scratch.0.join("Support");
    std::fs::create_dir_all(&support).unwrap();
    std::fs::write(support.join("favourites.json"), b"{\"version\":1,\"paths\":[").unwrap();
    assert_eq!(brought(&support, None), Brought { notice: Some(UNPARSED), tell: true, ..Brought::default() });
    told(&support);
    assert_eq!(brought(&support, None), Brought::default(), "said once: nothing will ever parse in it");
}

#[test]
fn an_older_build_writing_its_files_after_the_import_is_said_once() {
    let scratch = Scratch::new("older");
    let support = scratch.0.join("Support");
    std::fs::create_dir_all(&support).unwrap();
    let favourites = support.join("favourites.json");
    std::fs::write(&favourites, br#"{"version":1,"paths":["/Volumes/T7/Scans"]}"#).unwrap();
    aged(&favourites, 60);
    assert_eq!(brought(&support, None), Brought { tell: true, ..Brought::default() });
    told(&support);
    assert_eq!(brought(&support, None), Brought::default(), "the old file untouched, nothing said");
    // 1.5.6 stars a folder after the update.
    std::fs::write(&favourites, br#"{"version":1,"paths":["/Volumes/T7/Scans","/Users/a/Rolls"]}"#).unwrap();
    let said = brought(&support, None);
    assert_eq!(said, Brought { notice: Some(OLDER_SINCE), ..Brought::default() });
    assert_eq!(oni_library::places::Places::load(&support).unwrap().paths(), ["/Volumes/T7/Scans"], "not carried over");
    assert_eq!(brought(&support, None), Brought::default(), "said once");
    // Twin: the ledger's mark of the old files' last write is what says it once.
    let mut ledger = ledger::read(&support).unwrap();
    ledger.former_written = ledger.former_written.map(|t| t - 30.0);
    ledger::write(&support, &ledger).unwrap();
    assert_eq!(brought(&support, None), said);
}

#[test]
fn the_new_format_is_told_once_shown_and_never_to_an_empty_support_folder() {
    let scratch = Scratch::new("told");
    let (held, empty) = (scratch.0.join("Held"), scratch.0.join("Empty"));
    std::fs::create_dir_all(held.join("Thumbnails")).unwrap();
    std::fs::create_dir_all(&empty).unwrap();
    assert!(brought(&held, None).tell);
    // A launch that never showed the sheet, a bench's or one that ended first, tells again.
    assert!(brought(&held, None).tell, "not shown, still owed");
    told(&held);
    assert!(!brought(&held, None).tell, "once shown, never again");
    assert!(!brought(&empty, None).tell, "a first launch has nothing to be told");
    assert_eq!(ledger::read(&empty).unwrap().told, oni_settings::document::VERSION, "nor later");
    // Twin: a ledger that never recorded the telling tells again.
    ledger::write(&held, &Ledger { told: 0, ..ledger::read(&held).unwrap() }).unwrap();
    assert!(brought(&held, None).tell);
}

/// `cameras.json` as the former version left it: a camera fitted, one it could not fit.
fn table(support: &Path) {
    std::fs::create_dir_all(support.join("Transition")).unwrap();
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let table = serde_json::json!({
        "version": 1,
        "cameras": [{ "make": "Nikon", "model": "Z f", "matrix": identity, "frames": [], "residual": 0.0 }],
        "unmatched": [{ "make": "Canon", "model": "EOS 350D", "reason": "the two decoders could not be matched" }],
    });
    std::fs::write(support.join("Transition/cameras.json"), serde_json::to_vec(&table).unwrap()).unwrap();
}

#[test]
fn a_camera_the_table_fitted_keeps_its_matrix_and_any_other_reads_as_none() {
    let scratch = Scratch::new("table");
    table(&scratch.0);
    let known = facts::known(&scratch.0);
    assert!(known.answer("Nikon", "Z f").is_some_and(|m| m[0][0] == 1.0), "the fitted camera's matrix");
    // No build here decoded these through a matrix: what 1.5.2 to 1.5.6 showed, never a lock.
    assert_eq!(known.answer("Canon", "EOS 350D"), None, "measured here, no fit");
    assert_eq!(known.answer("Sony", "ILCE-7RM4"), None, "never named here");
    // No table at all, Windows or a Mac the former version never ran on: none, every camera.
    let bare = facts::known(&scratch.0.join("Nowhere"));
    assert_eq!(bare.answer("Nikon", "Z f"), None);
    // Twin: the table read wrong, its fitted camera would lose its matrix too.
    assert_ne!(known.answer("Nikon", "Z f"), bare.answer("Nikon", "Z f"));
}

#[test]
fn an_original_kept_in_the_cloud_is_asked_nothing() {
    use oni_settings::migrate::{Facts, Refusal};
    let scratch = Scratch::new("cloud");
    // No such file: a fact read off it would say its camera unknown or its file plain, never `Away`.
    let photo = scratch.0.join("DSC_0001.NEF");
    let lookup = facts::lookup(facts::Known::default(), |_| true);
    let rest = oni_settings::PipelineSettings::default();
    let mut seen = Vec::new();
    lookup(&photo, None, &mut |f: &Facts| seen.extend([f.mosaic.get().map(|_| ()), (f.matrix)().map(|_| ()), (f.scan)(&rest).map(|_| ())]));
    assert_eq!(seen, [Err(Refusal::Away), Err(Refusal::Away), Err(Refusal::Away)]);
    // Twin: on the disk, the same facts are read off the file.
    let lookup = facts::lookup(facts::Known::default(), |_| false);
    let mut read = Vec::new();
    lookup(&photo, None, &mut |f: &Facts| read.extend([f.mosaic.get().map(|_| ()), (f.scan)(&rest).map(|_| ())]));
    assert_eq!(read, [Ok(()), Err(Refusal::ScanUnknown)]);
}
