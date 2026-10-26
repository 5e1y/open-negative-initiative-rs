//! The tree's cost on a synthetic library: 5000 photos in 200 folders and a flat folder of 2000 beside
//! 20000 rows of other folders, each stage and each event timed and printed. `cargo test -p oni-app --lib tree_bench -- --ignored --nocapture`.

use super::*;
use oni_library::navigator::Computer;
use oni_library::{folders, roots};
use oni_settings::Date;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// The library's shape: folders of `per` photos, then one flat folder.
const FOLDERS: usize = 200;
const PER: usize = 25;
const FLAT: usize = 2000;
/// Rows the index holds for folders elsewhere, as a whole library's index does.
const ELSEWHERE: usize = 20_000;

struct Library {
    root: PathBuf,
    folder: String,
    index: LibraryIndex,
    medalled: String,
}

/// The files written once, their dates set back past `SETTLE`, every row and stamp in the index.
fn build(root: &Path) -> Library {
    let _ = std::fs::remove_dir_all(root);
    let opened = root.join("Library");
    let past = SystemTime::now() - Duration::from_secs(3600);
    let mut files = Vec::new();
    for f in 0..FOLDERS {
        let year = 2000 + f / 20;
        for p in 0..PER {
            files.push((opened.join(format!("{year}/Roll {f}/DSC_{p:04}.NEF")), f * PER + p));
        }
    }
    for p in 0..FLAT {
        files.push((opened.join(format!("Flat/IMG_{p:05}.NEF")), FOLDERS * PER + p));
    }
    for (path, i) in &files {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("photo {i}")).unwrap();
        let sidecar = oni_settings::sidecar::url_for(path);
        std::fs::write(&sidecar, b"{}").unwrap();
        for p in [path, &sidecar] {
            std::fs::File::options().write(true).open(p).unwrap().set_modified(past).unwrap();
        }
    }
    for f in (0..FOLDERS).step_by(4) {
        let year = 2000 + f / 20;
        oni_settings::roll_folder::set_roll(Some(&oni_settings::Film::new(&format!("Roll {f}"))), &opened.join(format!("{year}/Roll {f}"))).unwrap();
    }
    let folder = oni_library::paths::canonical(&opened);
    let index = LibraryIndex::open(&root.join("Caches/index.sqlite")).unwrap();
    let entries: Vec<Entry> = files
        .iter()
        .map(|(path, i)| {
            let path = oni_library::paths::canonical(path);
            let size = format!("photo {i}").len();
            let mut e = Entry::new(&path, Date::since_reference_date(7e8), &format!("{size}-{i:08x}"));
            e.captured_at = Some(Date::since_reference_date(7e8 + (*i as f64 * 7919.0) % 3e7));
            e.label = [Medal::None, Medal::None, Medal::Bronze, Medal::Gold][i % 4];
            e
        })
        .collect();
    index.upsert_all(&entries).unwrap();
    let elsewhere: Vec<Entry> = (0..ELSEWHERE).map(|i| Entry::new(&format!("/Volumes/Archive/{}/F{i}.NEF", i / 100), Date::since_reference_date(7e8), &format!("9-{i}"))).collect();
    index.upsert_all(&elsewhere).unwrap();
    let medalled = entries[PER + 3].path.clone();
    oni_settings::sidecar::write(&oni_settings::PipelineSettings::default(), Path::new(&medalled), Default::default()).unwrap();
    // The first read stamps every row, so the reads timed below find the index steady.
    let place = Place { computer: &Computer::none(), kept: None };
    read_rows(&folder, &index, &place, &|| true, &mut |_, _, _| {});
    Library { root: root.to_path_buf(), folder, index, medalled }
}

/// The best and the median of `runs` runs of `body`, in milliseconds.
fn timed<T>(name: &str, runs: usize, mut body: impl FnMut() -> T) -> T {
    let mut times = Vec::new();
    let mut out = None;
    for _ in 0..runs {
        let start = Instant::now();
        out = Some(body());
        times.push(start.elapsed().as_secs_f64() * 1e3);
    }
    times.sort_by(f64::total_cmp);
    println!("{name:<44} best {:>9.2} ms   median {:>9.2} ms", times[0], times[times.len() / 2]);
    out.unwrap()
}

/// The read as the app's library thread runs it: the rows landed, the survey they were cut on.
fn landed(lib: &Library) -> (Vec<Entry>, Arc<Survey>) {
    let place = Place { computer: &Computer::none(), kept: None };
    let mut last = None;
    read_rows(&lib.folder, &lib.index, &place, &|| true, &mut |rows, _, survey| last = Some((rows, survey.clone())));
    last.expect("a landing")
}

#[test]
#[ignore = "a bench, run on demand"]
fn tree_bench() {
    let root = std::env::temp_dir().join(format!("oni-tree-bench-{}", std::process::id()));
    let start = Instant::now();
    let lib = build(&root);
    println!("synthetic library of {} photos built in {:.1} s", FOLDERS * PER + FLAT, start.elapsed().as_secs_f64());
    let runs = 5;
    let folder = lib.folder.clone();

    println!("-- stages");
    let survey = timed("survey (roots::surveyed)", runs, || roots::surveyed(std::slice::from_ref(&folder), &|| true, &Computer::none()));
    fn floor(dir: &Path) -> usize {
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let m = std::fs::symlink_metadata(e.path()).unwrap();
            n += if m.is_dir() { floor(&e.path()) } else { 1 };
        }
        n
    }
    timed("survey floor (read_dir, lstat)", runs, || floor(Path::new(&folder)));
    timed("rolls (folders::rolls)", runs, || folders::rolls(&survey.images));
    timed("index read whole (index.all)", runs, || lib.index.all());
    timed("index read under the folder (index.under)", runs, || lib.index.under(&folder));
    timed("index stamps (stamps_under)", runs, || lib.index.stamps_under(&folder));
    timed("rows shown (shown_under)", runs, || shown_under(&lib.index, &folder));
    timed("rows unread (library::unresolved)", runs, || oni_library::library::unresolved(&survey.images, &lib.index));
    let (rows, survey) = timed("read whole (read_rows, survey included)", runs, || landed(&lib));
    timed("pairs (folders::paired)", runs, || folders::paired(&rows, &survey.images, &survey.sidecars));
    let root_of = oni_library::paths::parent(&folder);
    timed("tree (folders::tree)", runs, || folders::tree(&root_of, &rows, &survey.rolls, &survey.folders, &survey.made));
    timed("groups (folders::groups)", runs, || folders::groups(&root_of, &rows, &survey.rolls, &survey.made));
    timed("presence per row (Survey::presence)", runs, || rows.iter().map(|e| survey.presence(&e.path)).filter(|p| p.is_present()).count());
    let mut archive = timed("archive (Archive::from_rows)", runs, || Archive::from_rows(&folder, &rows, &survey, None));
    timed("archive, one medal filtered", runs, || Archive::from_rows(&folder, &rows, &survey, Some(Medal::Gold)));
    let all = timed("row list (Archive::rows), all unfolded", runs, || archive.rows());
    timed("folded (Archive::folded)", runs, || archive.folded());
    let mut names: Vec<String> = rows.iter().map(|e| oni_library::paths::name(&e.path)).collect();
    timed("sort 7000 names (standard_order)", runs, || {
        names.reverse();
        names.sort_by(|a, b| oni_library::paths::standard_order(a, b));
    });
    let accented: Vec<String> = rows.iter().map(|e| format!("/Volumes/\u{c9}t\u{e9}/{}", oni_library::paths::name(&e.path))).collect();
    timed("sort 7000 accented paths, weighed per comparison", runs, || {
        let mut v = accented.clone();
        v.sort_by(|a, b| oni_library::paths::standard_order(a, b));
    });
    timed("sort 7000 accented paths, weighed once", runs, || {
        let mut v: Vec<(oni_library::paths::NameKey, &String)> = accented.iter().map(|p| (oni_library::paths::NameKey::new(p), p)).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
    });
    println!("{} rows laid, {} folders", all.len(), survey.folders.len());

    println!("-- events");
    let open = |archive: &mut Archive, path: &str| {
        archive.unfold_to(path);
        archive.rows().len()
    };
    timed("launch or open a folder: read, archive, rows", runs, || {
        let (rows, survey) = landed(&lib);
        let a = Archive::from_rows(&folder, &rows, &survey, None);
        a.rows().len()
    });
    timed("open a photo: unfold, rows", runs, || open(&mut archive, &lib.medalled));
    timed("thumbnail landing: tree work", runs, || 0);
    let first = NodeId::Folder(oni_library::paths::parent(&lib.medalled));
    timed("fold a folder: toggle, rows", runs, || {
        archive.toggle(&first);
        archive.rows().len()
    });
    let medal = |label: Medal| {
        oni_settings::sidecar::set_label(label, Path::new(&lib.medalled), false).unwrap();
        let start = Instant::now();
        refresh(&lib.index, std::slice::from_ref(&lib.medalled));
        let rows = shown_under(&lib.index, &folder);
        (rows, start)
    };
    let mut medals = Vec::new();
    for i in 0..runs {
        let (rows, start) = medal([Medal::Gold, Medal::None][i % 2]);
        Archive::from_rows(&folder, &rows, &survey, None).rows();
        medals.push(start.elapsed().as_secs_f64() * 1e3);
    }
    medals.sort_by(f64::total_cmp);
    println!("{:<44} best {:>9.2} ms   median {:>9.2} ms", "medal: reread, rows shown, archive, rows", medals[0], medals[runs / 2]);

    println!("-- events, the interface thread's share as `lay_archive` lays them");
    let mut landings = Vec::new();
    for _ in 0..runs {
        let staged = Archive::from_rows(&folder, &rows, &survey, None);
        let start = Instant::now();
        let (a, how) = Archive::relaid(None, Some(staged), &folder, &rows, &survey, None);
        a.rows();
        landings.push(start.elapsed().as_secs_f64() * 1e3);
        assert_eq!(how, Relaid::Cut);
    }
    landings.sort_by(f64::total_cmp);
    println!("{:<44} best {:>9.2} ms   median {:>9.2} ms", "a read landing, its tree cut off the thread", landings[0], landings[runs / 2]);
    let mut held = Some(Archive::from_rows(&folder, &rows, &survey, None));
    let mut relay = |name: &str, rows: &[Entry], survey: &Arc<Survey>, staged: Option<Archive>, then: &dyn Fn(&mut Archive)| {
        let mut staged = Some(staged);
        timed(name, runs, || {
            let (mut a, how) = Archive::relaid(held.take(), staged.take().flatten(), &folder, rows, survey, None);
            then(&mut a);
            let n = a.rows().len();
            held = Some(a);
            (n, how)
        })
    };
    let cuts = cuts();
    println!("  {:?}", relay("a read again on focus, nothing moved", &rows, &Arc::new((*survey).clone()), None, &|_| {}));
    let photo = lib.medalled.clone();
    println!("  {:?}", relay("open a photo", &rows, &survey, None, &|a| {
        a.unfold_to(&photo);
    }));
    println!("  {:?}", relay("thumbnail landing", &rows, &survey, None, &|_| {}));
    println!("  {:?}", relay("fold a folder", &rows, &survey, None, &|a| {
        a.toggle(&first);
    }));
    let mut medals = Vec::new();
    for i in 0..runs {
        let (rows, start) = medal([Medal::Silver, Medal::None][i % 2]);
        let (a, how) = Archive::relaid(held.take(), None, &folder, &rows, &survey, None);
        a.rows();
        held = Some(a);
        medals.push(start.elapsed().as_secs_f64() * 1e3);
        // The first reread also reads the mode the synthetic rows lack, which cuts the tree.
        assert!(i == 0 || how == Relaid::Medals, "{how:?}");
    }
    medals.sort_by(f64::total_cmp);
    println!("{:<44} best {:>9.2} ms   median {:>9.2} ms", "medal: reread, rows shown, relaid, rows", medals[0], medals[runs / 2]);
    let after = super::cuts();
    println!("trees cut {}, row lists laid {} over these events", after.0 - cuts.0, after.1 - cuts.1);
    let _ = std::fs::remove_dir_all(&lib.root);
}
