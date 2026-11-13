//! The sweep over a scratch library: it ends with every sidecar in this format, its dates kept, none
//! brought twice across a stop; a disk away is pending until it answers, a place gone is dropped, a file
//! refused is said once and owes no sweep; an opened folder's new rows are gated as they are read.

use super::ledger;
use super::sweep::{self, Place};
use oni_library::navigator::Computer;
use oni_library::roots::Roots;
use oni_settings::migrate::{self, Standing};
use oni_settings::sidecar;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Corpus sidecars of format 19 and 20 that ask no fact of their original.
const SOURCES: [&str; 2] = ["19-rust/040-P1073163.RW2.json", "20-rust/002-DSC_6438.NEF.json"];

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Scratch {
        let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("oni-app-sweep-{label}-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(dir.join("Support")).unwrap();
        std::fs::create_dir_all(dir.join("Caches")).unwrap();
        Scratch(dir)
    }

    fn place(&self) -> Place {
        let volumes = self.0.join("Volumes");
        std::fs::create_dir_all(&volumes).unwrap();
        Place { support: self.0.join("Support"), caches: self.0.join("Caches"), computer: Computer { home: String::new(), volumes: oni_library::paths::canonical(&volumes) } }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn source(n: usize) -> Vec<u8> {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-settings/tests/corpus");
    std::fs::read(corpus.join(SOURCES[n % SOURCES.len()])).unwrap()
}

/// `folders` folders of `each` photos under `root`, each an empty original beside a sidecar of an
/// earlier format, dated an hour ago; their sidecars' paths.
fn library(root: &Path, folders: usize, each: usize) -> Vec<PathBuf> {
    let (a, b) = (source(0), source(1));
    let hour_ago = SystemTime::now() - Duration::from_secs(3600);
    let mut out = Vec::new();
    for f in 0..folders {
        let folder = root.join(format!("Roll {f:03}"));
        std::fs::create_dir_all(&folder).unwrap();
        for i in 0..each {
            let original = folder.join(format!("DSC_{i:04}.NEF"));
            std::fs::write(&original, b"").unwrap();
            let side = sidecar::url_for(&original);
            std::fs::write(&side, if i % 2 == 0 { &a } else { &b }).unwrap();
            std::fs::File::options().write(true).open(&side).unwrap().set_modified(hour_ago).unwrap();
            out.push(side);
        }
    }
    out
}

fn rooted(support: &Path, roots: &[&Path]) {
    let mut r = Roots::default();
    for root in roots {
        r.add(&root.to_string_lossy());
    }
    std::fs::write(Roots::file_path(support), serde_json::to_vec(&r).unwrap()).unwrap();
}

/// The sweep run to its end; its count of sidecars brought.
fn swept(place: &Place) -> usize {
    let Some(sweep) = sweep::start(place.clone()) else { return 0 };
    let start = Instant::now();
    while sweep.running() {
        assert!(start.elapsed() < Duration::from_secs(300), "the sweep ends");
        std::thread::sleep(Duration::from_millis(5));
    }
    sweep.progress.migrated.load(std::sync::atomic::Ordering::Relaxed)
}

fn current(files: &[PathBuf]) -> usize {
    files.iter().filter(|f| migrate::standing(&std::fs::read(f).unwrap()) == Standing::Current).count()
}

#[test]
fn a_sweep_brings_every_sidecar_keeps_its_date_and_runs_once() {
    let scratch = Scratch::new("library");
    let root = scratch.0.join("Library");
    let (folders, each) = std::env::var("ONI_SWEEP_FILES").ok().and_then(|n| n.parse::<usize>().ok()).map_or((20, 20), |n| (n.div_ceil(38), 38));
    let files = library(&root, folders, each);
    let dates: Vec<SystemTime> = files.iter().map(|f| std::fs::metadata(f).unwrap().modified().unwrap()).collect();
    let place = scratch.place();
    rooted(&place.support, &[&root]);
    let started = Instant::now();
    assert_eq!(swept(&place), files.len());
    let took = started.elapsed();
    println!("sweep: {} sidecars in {} folders, {:.2} s, {:.3} ms a file", files.len(), folders, took.as_secs_f64(), took.as_secs_f64() * 1000.0 / files.len() as f64);
    assert_eq!(current(&files), files.len(), "every sidecar in this format");
    let now: Vec<SystemTime> = files.iter().map(|f| std::fs::metadata(f).unwrap().modified().unwrap()).collect();
    assert_eq!(now, dates, "no date moved: the index stamps and the tiles hold");
    let ledger = ledger::read(&place.support).unwrap();
    assert!(ledger.sidecars == oni_settings::document::VERSION && ledger.pending.is_empty() && ledger.left.is_empty());
    // Ended, no sweep is owed: a second launch reads the ledger and walks nothing.
    let idle = Instant::now();
    assert!(sweep::start(place).is_none());
    println!("second launch: no sweep owed, decided in {:.3} ms", idle.elapsed().as_secs_f64() * 1000.0);
    // Twin: a write that lets the date move, as a plain atomic write does, is seen.
    oni_settings::atomic::write_atomically(&files[0], &std::fs::read(&files[0]).unwrap()).unwrap();
    assert_ne!(std::fs::metadata(&files[0]).unwrap().modified().unwrap(), dates[0]);
}

#[test]
fn a_sweep_stopped_midway_is_taken_again_and_brings_none_twice() {
    let scratch = Scratch::new("stopped");
    let root = scratch.0.join("Library");
    let files = library(&root, 12, 30);
    let place = scratch.place();
    rooted(&place.support, &[&root]);
    let sweep = sweep::start(place.clone()).unwrap();
    while sweep.progress.migrated.load(std::sync::atomic::Ordering::Relaxed) < 40 && sweep.running() {
        std::thread::sleep(Duration::from_millis(1));
    }
    let first = {
        let progress = sweep.progress.clone();
        drop(sweep);
        progress.migrated.load(std::sync::atomic::Ordering::Relaxed)
    };
    assert!(first > 0 && first < files.len(), "stopped part way: {first}");
    assert!(ledger::read(&place.support).unwrap().sidecars < oni_settings::document::VERSION, "a stopped sweep never says it ended");
    let second = swept(&place);
    assert_eq!(first + second, files.len(), "each sidecar brought once");
    assert_eq!(current(&files), files.len());
}

#[test]
fn a_disk_away_is_pending_and_brought_once_it_answers_and_a_place_gone_is_dropped() {
    let scratch = Scratch::new("away");
    let place = scratch.place();
    let volumes = PathBuf::from(&place.computer.volumes);
    let (here, away, gone) = (scratch.0.join("Here"), volumes.join("T7").join("Rolls"), scratch.0.join("Gone"));
    let present = library(&here, 2, 3);
    rooted(&place.support, &[&here, &away, &gone]);
    swept(&place);
    assert_eq!(current(&present), present.len());
    let ledger = ledger::read(&place.support).unwrap();
    // The ledger keeps a place in the archive's spelling, `/` on Windows.
    let away_spelt = oni_library::paths::normal(&away.to_string_lossy()).into_owned();
    assert_eq!(ledger.pending, [away_spelt], "the disk away pending, the place gone from a disk present dropped");
    // A file of an earlier format laid in a folder already swept: the next sweep takes the pending place alone.
    let since = library(&here.join("Since"), 1, 1);
    // The disk back: the next sweep brings it and empties the pending list.
    let later = library(&away, 2, 3);
    assert_eq!(swept(&place), later.len());
    assert_eq!(current(&later), later.len());
    assert!(ledger::read(&place.support).unwrap().pending.is_empty());
    assert_eq!(current(&since), 0, "the library is not walked again once a whole sweep ended");
    assert!(sweep::start(place.clone()).is_none(), "nothing left, no sweep owed");
    // Twin: a ledger that names the disk pending owes a sweep, which takes it.
    let more = library(&volumes.join("T8"), 1, 2);
    ledger::update(&place.support, |l| l.pending = vec![volumes.join("T8").to_string_lossy().into_owned()]).unwrap();
    assert_eq!(swept(&place), more.len());
}

#[cfg(unix)]
#[test]
fn a_file_refused_is_said_once_owes_no_sweep_and_is_taken_again_once_it_changed() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new("refused");
    let root = scratch.0.join("Library");
    let files = library(&root, 2, 2);
    let locked = root.join("Roll 001");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let place = scratch.place();
    rooted(&place.support, &[&root]);
    let sweep = sweep::start(place.clone()).unwrap();
    while sweep.running() {
        std::thread::sleep(Duration::from_millis(5));
    }
    let refused = sweep.progress.refused.load(std::sync::atomic::Ordering::Relaxed);
    drop(sweep);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!((refused, current(&files)), (2, 2), "two left as they are, each once");
    let ledger = ledger::read(&place.support).unwrap();
    let left: Vec<_> = ledger.left.values().map(|l| (l.why, l.said)).collect();
    assert_eq!(left, [(ledger::Why::Unwritable, false), (ledger::Why::Unwritable, false)], "listed with their reason, not said yet");
    assert!(ledger.pending.is_empty(), "a folder of a disk present is never pending");
    assert!(sweep::start(place.clone()).is_none(), "a file left as it was owes no sweep: the library is not walked at each launch");
    // A pending place walked again leaves the unchanged files as they are, still said once.
    ledger::update(&place.support, |l| {
        l.pending = vec![oni_library::paths::canonical(&root)];
        l.left.values_mut().for_each(|left| left.said = true);
    })
    .unwrap();
    assert_eq!(swept(&place), 0, "unchanged, not taken again");
    assert!(ledger::read(&place.support).unwrap().left.values().all(|l| l.said));
    // Twin: a file that changed since is taken again, and leaves the list once brought.
    let changed = files.iter().find(|f| f.starts_with(&locked)).unwrap();
    std::fs::File::options().write(true).open(changed).unwrap().set_modified(SystemTime::now()).unwrap();
    ledger::update(&place.support, |l| l.pending = vec![oni_library::paths::canonical(&root)]).unwrap();
    assert_eq!(swept(&place), 1);
    assert_eq!(ledger::read(&place.support).unwrap().left.len(), 1);
}

#[test]
fn an_opened_folder_lands_its_rows_first_and_brings_a_new_rows_sidecar_as_it_reads_it() {
    let scratch = Scratch::new("gate");
    let files = library(&scratch.0.join("Never"), 1, 4);
    let folder = files[0].parent().unwrap().to_path_buf();
    let old = std::fs::read(&files[0]).unwrap();
    // Twin: an earlier sidecar reaching the live decoder is refused, which the gate prevents.
    assert!(oni_settings::document::decode(&old).is_err());
    let index = oni_library::index::LibraryIndex::open(Path::new(":memory:")).unwrap();
    let place = crate::library::Place { computer: &Computer::none(), kept: None };
    let folder = oni_library::paths::canonical(&folder);
    let mut landings: Vec<(usize, usize)> = Vec::new();
    crate::library::read_rows(&folder, &index, &place, &|| true, &mut |landed, _, _| landings.push((landed.len(), current(&files))));
    assert_eq!(landings.first(), Some(&(files.len(), 0)), "every row shown, waiting, before any sidecar is brought");
    assert_eq!(landings.last().map(|l| l.0), Some(files.len()));
    assert_eq!(current(&files), files.len(), "each new row's sidecar brought as it was read");
    // Rows standing, unchanged: a second opening reads no sidecar, so one of an earlier format laid
    // under an unchanged stamp stays as it is for the sweep.
    let stamp = std::fs::metadata(&files[0]).unwrap().modified().unwrap();
    std::fs::write(&files[0], &old).unwrap();
    std::fs::File::options().write(true).open(&files[0]).unwrap().set_modified(stamp).unwrap();
    crate::library::read_rows(&folder, &index, &place, &|| true, &mut |_, _, _| {});
    assert_eq!(std::fs::read(&files[0]).unwrap(), old, "no sidecar of a standing row read at an opening");
    // Twin: the same file under a stamp that moved is read again, and brought.
    std::fs::File::options().write(true).open(&files[0]).unwrap().set_modified(SystemTime::now()).unwrap();
    crate::library::read_rows(&folder, &index, &place, &|| true, &mut |_, _, _| {});
    assert_eq!(current(&files), files.len());
}

#[test]
fn a_quit_never_waits_on_a_sweep_held_by_a_disk_that_stopped_answering() {
    let ended = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let quick = ended.clone();
    drop(sweep::around(move || quick.store(true, std::sync::atomic::Ordering::Relaxed)));
    assert!(ended.load(std::sync::atomic::Ordering::Relaxed), "a sweep that ends is joined");
    let started = Instant::now();
    drop(sweep::around(|| std::thread::sleep(Duration::from_secs(30))));
    assert!(started.elapsed() < Duration::from_secs(5), "the quit waits a bounded time: {:?}", started.elapsed());
    // Planning stops before spelling a path once the quit asked.
    let scratch = Scratch::new("planning");
    let place = scratch.place();
    rooted(&place.support, &[&scratch.0.join("A"), &scratch.0.join("B")]);
    assert_eq!(super::walk::planned(&place.support, &place.caches, &[], &|| false), super::walk::Plan::default());
    // Twin: alive, the same plan spells both.
    assert_eq!(super::walk::planned(&place.support, &place.caches, &[], &|| true).trees.len(), 2);
}

/// Whether the check's cloud keeps its originals away, for the paths it marks.
static CLOUD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

fn in_check_cloud(path: &Path) -> bool {
    CLOUD.load(std::sync::atomic::Ordering::Relaxed) && path.to_string_lossy().contains("check-cloud")
}

#[test]
fn an_original_in_the_cloud_is_said_once_owes_no_sweep_and_is_brought_once_opened() {
    let scratch = Scratch::new("check-cloud");
    let folder = scratch.0.join("Roll");
    std::fs::create_dir_all(&folder).unwrap();
    // A format 14 sidecar asks how its file decodes, which reads the original.
    let early = Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-settings/tests/corpus/early/105-P1072740.RW2.json");
    let photo = folder.join("P1072740.tif");
    std::fs::write(&photo, b"").unwrap();
    std::fs::write(sidecar::url_for(&photo), std::fs::read(early).unwrap()).unwrap();
    migrate::install(Some(std::sync::Arc::new(super::facts::lookup(super::facts::Known::default(), in_check_cloud))));
    let place = scratch.place();
    rooted(&place.support, &[&folder]);
    swept(&place);
    let ledger = ledger::read(&place.support).unwrap();
    let owed = sweep::start(place.clone()).is_some();
    let left: Vec<_> = ledger.left.values().map(|l| l.why).collect();
    // Downloaded, then opened: the gate brings it.
    CLOUD.store(false, std::sync::atomic::Ordering::Relaxed);
    sweep::gated(&photo.to_string_lossy());
    migrate::install(None);
    assert!(ledger.pending.is_empty(), "no folder pending for an original in the cloud: {:?}", ledger.pending);
    assert_eq!(left, [ledger::Why::Away], "said once, with its reason");
    assert!(!owed, "no sweep owed at each launch");
    assert_eq!(migrate::standing(&std::fs::read(sidecar::url_for(&photo)).unwrap()), Standing::Current, "twin: once downloaded, the gate brings it");
}
