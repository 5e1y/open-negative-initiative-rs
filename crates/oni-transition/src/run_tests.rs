use super::*;
use crate::test_support::Scratch;

#[test]
fn the_frames_tried_spread_over_the_roll() {
    assert_eq!(spread(38), vec![9, 22, 34, 3, 15, 28]);
    assert_eq!(spread(2), vec![0, 1]);
    assert!(spread(0).is_empty());
    // Twin: the roll's first frames are not the first tried.
    assert_ne!(spread(38)[..3], [0, 1, 2]);
}

#[test]
fn a_file_libraw_cannot_name_is_no_camera_and_an_unplugged_root_waits() {
    let (support, roll) = (Scratch::new("support"), Scratch::new("roll"));
    std::fs::write(roll.path().join("scan.NEF"), b"not a RAW").unwrap();
    std::fs::write(roll.path().join("scan.tif"), b"not a RAW either").unwrap();
    let roots = vec![roll.path().to_string_lossy().into_owned(), "/Volumes/oni-unplugged-disk".into()];
    let found = survey::survey(support.path(), &roots);
    assert!(found.cameras.is_empty());
    assert_eq!(found.unreachable, ["/Volumes/oni-unplugged-disk"]);
    assert_eq!(pending(support.path(), &roots), 0);
    // Twin: an unreadable table measures nothing rather than starting an empty one.
    std::fs::create_dir_all(table::folder(support.path())).unwrap();
    std::fs::write(Table::path(support.path()), b"{ torn").unwrap();
    assert!(run(support.path(), &roots, &crate::Simulated { matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] }, &mut |_| {}).is_err());
}

const ZF: M3 = [[1.1252, -0.1698, -0.0064], [0.0259, 0.9537, -0.0640], [0.0092, -0.0109, 0.8969]];

#[test]
fn a_camera_is_matched_once_and_no_setting_moves() {
    let Some(dir) = oni_fixtures::raw_dir("a_camera_is_matched_once_and_no_setting_moves") else { return };
    let mut nefs: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "NEF")).collect();
    nefs.sort();
    let (support, roll) = (Scratch::new("support"), Scratch::new("roll"));
    for nef in nefs.iter().take(2) {
        std::fs::hard_link(nef, roll.path().join(nef.file_name().unwrap())).unwrap();
    }
    let sidecar = roll.path().join(format!("{}.json", nefs[0].file_name().unwrap().to_string_lossy()));
    std::fs::write(&sidecar, b"{\"version\":19}").unwrap();
    let roots = vec![roll.path().to_string_lossy().into_owned()];
    assert_eq!(pending(support.path(), &roots), 1);
    let mut events = Vec::new();
    let summary = run(support.path(), &roots, &crate::Simulated { matrix: ZF }, &mut |e| events.push(e)).unwrap();
    assert_eq!(summary.matched.len(), 2, "{summary:?}");
    assert!(matches!(events.last(), Some(Event::Measured { residual: Ok(_), .. })));
    let table = Table::load(support.path()).unwrap();
    let entry = table.get("Nikon", "Z f").expect("the Z f");
    for r in 0..3 {
        for c in 0..3 {
            assert!((entry.matrix[r][c] - ZF[r][c]).abs() < 2e-3, "{:?}", entry.matrix);
        }
    }
    assert_eq!(entry.frames.len(), 2);
    assert!(oni_decode::rendition::matrix_for("Nikon", "Z f").is_some(), "installed for every later decode");
    assert_eq!(std::fs::read(&sidecar).unwrap(), b"{\"version\":19}", "no sidecar is written");
    // Twin: a camera in the table is never measured again.
    assert_eq!(pending(support.path(), &roots), 0);
    let again = run(support.path(), &roots, &crate::Simulated { matrix: ZF }, &mut |_| {}).unwrap();
    assert!(again.cameras.is_empty() && again.matched.is_empty());
    oni_decode::rendition::install([]);
}

/// Apple's decode refusing every frame; `unplug` first removes the frame, as a disk pulled mid-run.
struct Refusing {
    unplug: bool,
}

impl crate::AppleDecode for Refusing {
    fn blocks(&self, raw: &Path) -> Result<crate::measure::Grid, String> {
        if self.unplug {
            let _ = std::fs::remove_file(raw);
        }
        Err("Apple cannot decode it".into())
    }
}

#[test]
fn a_camera_that_will_not_match_is_not_tried_at_every_launch() {
    let Some(dir) = oni_fixtures::raw_dir("a_camera_that_will_not_match_is_not_tried_at_every_launch") else { return };
    let nef = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).find(|p| p.extension().is_some_and(|e| e == "NEF")).unwrap();
    let (support, roll) = (Scratch::new("support"), Scratch::new("roll"));
    std::fs::hard_link(&nef, roll.path().join(nef.file_name().unwrap())).unwrap();
    let roots = vec![roll.path().to_string_lossy().into_owned()];
    let summary = run(support.path(), &roots, &Refusing { unplug: false }, &mut |_| {}).unwrap();
    assert!(summary.cameras[0].residual.is_err() && summary.matched.is_empty());
    assert_eq!(pending(support.path(), &roots), 0, "the refusal is remembered");
    assert!(Table::load(support.path()).unwrap().get("Nikon", "Z f").is_none(), "and decodes as LibRaw");
    // Twin: a frame unplugged during the run leaves the camera pending for its return.
    let (support, roll) = (Scratch::new("support"), Scratch::new("roll"));
    std::fs::hard_link(&nef, roll.path().join(nef.file_name().unwrap())).unwrap();
    let roots = vec![roll.path().to_string_lossy().into_owned()];
    run(support.path(), &roots, &Refusing { unplug: true }, &mut |_| {}).unwrap();
    std::fs::hard_link(&nef, roll.path().join(nef.file_name().unwrap())).unwrap();
    assert_eq!(pending(support.path(), &roots), 1);
}
