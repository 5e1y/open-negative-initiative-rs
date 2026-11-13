//! A library of an earlier format met by this build: said once, every sidecar brought in the
//! background while the folder opens whole, then a photo double-clicked, edited and left.

use crate::projects_tab::Hand;
use oni_app::migration::Brought;
use oni_app::{App, Help, Told, Upgraded};
use oni_settings::migrate::{self, Facts, Standing, reshape};
use oni_settings::{ConversionMode, Document, Medal, PipelineSettings, sidecar};
use slint::ComponentHandle;
use slint::platform::Key;
use std::path::Path;
use std::time::Duration;

const TONES: [(&str, f32); 3] = [("Roll/A.tif", 0.3), ("Roll/B.tif", 0.5), ("Roll/C.tif", 0.7)];

/// Each photo's sidecar as format 19 kept it: an exposure of its own, B a gold medal.
fn earlier(root: &Path) {
    for (i, (file, _)) in TONES.iter().enumerate() {
        let mut s = PipelineSettings::rest(ConversionMode::Negative, false);
        s.print.exposure = 0.1 * (i + 1) as f32;
        let mut document = Document::of(s);
        document.label = if i == 1 { Medal::Gold } else { Medal::None };
        let bytes = serde_json::to_vec_pretty(&reshape::as_stored(&document, 19)).unwrap();
        std::fs::write(sidecar::url_for(&root.join("Scans").join(file)), bytes).unwrap();
    }
    let mut roots = oni_library::roots::Roots::default();
    roots.add(&oni_library::paths::canonical(&root.join("Scans")));
    std::fs::create_dir_all(root.join("Support")).unwrap();
    std::fs::write(oni_library::roots::Roots::file_path(&root.join("Support")), serde_json::to_vec(&roots).unwrap()).unwrap();
}

fn said(bytes: &[u8]) -> (PipelineSettings, Medal) {
    let (c, _) = migrate::reading(bytes, &Facts::bare(false), true).unwrap();
    (c.document.settings, c.document.label)
}

pub(crate) fn an_earlier_library_is_brought_while_the_person_works() {
    let hand = Hand::made_with("migration", &TONES, earlier, Brought { tell: true, ..Brought::default() });
    let paths: Vec<String> = TONES.iter().map(|(f, _)| hand.at(f)).collect();
    let before: Vec<Vec<u8>> = paths.iter().map(|p| std::fs::read(sidecar::url_for(Path::new(p))).unwrap()).collect();
    let w = &hand.shell.window;
    // The app's launch installs the facts an old sidecar asks before any read.
    oni_app::migration::facts::install(&hand.root.join("Support"));
    hand.shell.launch_sheets();
    hand.shell.migrate(&hand.root.join("Caches"));
    // One sheet at a time: the welcome first, the notice once its last key closes it, then holding the keys.
    let (help, upgraded) = (w.global::<Help>(), w.global::<Upgraded>());
    assert!(help.get_welcome_shown() && !upgraded.get_shown(), "the notice waits while the welcome shows");
    for _ in 0..7 {
        assert!(!upgraded.get_shown(), "never over the welcome, on any page");
        hand.rig.key("\n");
    }
    assert!(!help.get_welcome_shown() && upgraded.get_shown(), "said once, as the welcome closes");
    hand.rig.key("\n");
    assert!(!upgraded.get_shown(), "its GOT IT holds the keyboard");
    let current = |p: &String| migrate::standing(&std::fs::read(sidecar::url_for(Path::new(p))).unwrap()) == Standing::Current;
    hand.rig.until("every sidecar brought", Duration::from_secs(30), || paths.iter().all(current) && w.global::<Upgraded>().get_line().is_empty());
    assert!(!w.global::<Told>().get_shown(), "nothing left as it was, nothing to say");
    // Each reads as its earlier file did.
    for (p, was) in paths.iter().zip(&before) {
        assert_eq!(said(&std::fs::read(sidecar::url_for(Path::new(p))).unwrap()), said(was), "{p}");
    }
    // The gallery, a double click, a notch, back: the edited frame alone moves.
    hand.open("Roll");
    assert_eq!(hand.shown().len(), 3);
    let ctl = hand.shell.ctl.clone();
    hand.click(&paths[0], false);
    let i = ctl.state.borrow().order().iter().position(|p| *p == paths[0]).unwrap() as i32;
    hand.lib().invoke_tile_clicked(i, false, false);
    hand.rig.pump();
    assert!(!w.global::<App>().get_gallery(), "a double click opens the editor");
    w.global::<oni_app::Store>().invoke_step(oni_app::fields::Field::Exposure.index(), 1);
    hand.rig.chord(Key::Control, "g");
    assert!(w.global::<App>().get_gallery(), "back to the gallery");
    let edited = || said(&std::fs::read(sidecar::url_for(Path::new(&paths[0]))).unwrap()).0 != said(&before[0]).0;
    hand.rig.until("the edit written", Duration::from_secs(10), edited);
    for (p, was) in paths.iter().zip(&before).skip(1) {
        assert_eq!(said(&std::fs::read(sidecar::url_for(Path::new(p))).unwrap()), said(was), "{p} untouched in meaning");
    }
    assert!(paths.iter().all(current));
    // The sheet shown records the telling, so the next launch says nothing.
    let support = hand.root.join("Support");
    assert!(!oni_app::migration::brought(&support, None).tell);
    // Twin: a support folder whose sheet no window showed is told at its next launch.
    oni_app::migration::ledger::update(&support, |l| l.told = 0).unwrap();
    assert!(oni_app::migration::brought(&support, None).tell);
}

/// Twin: with no welcome due, the launch says the new format at once.
pub(crate) fn with_no_welcome_due_the_notice_shows_at_once() {
    let seen = |root: &Path| {
        earlier(root);
        let mut preferences = oni_app::preferences::Preferences::load(&root.join("Support"));
        preferences.set("welcomeSeen", "1.6".into()).unwrap();
    };
    let hand = Hand::made_with("migration-seen", &TONES, seen, Brought { tell: true, ..Brought::default() });
    let w = &hand.shell.window;
    hand.shell.launch_sheets();
    assert!(!w.global::<Help>().get_welcome_shown() && w.global::<Upgraded>().get_shown(), "the notice at once");
    let _ = std::fs::remove_dir_all(&hand.root);
}

/// Whether the check's cloud keeps its originals away.
static CLOUD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

fn in_cloud(_: &Path) -> bool {
    CLOUD.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn an_original_the_opening_downloads_opens_editable() {
    let hand = Hand::made_with("cloud", &TONES, earlier, Brought::default());
    oni_app::migration::facts::install_with(&hand.root.join("Support"), in_cloud);
    let w = &hand.shell.window;
    hand.open("Roll");
    let ctl = hand.shell.ctl.clone();
    let opened = |name: &str| {
        let path = hand.at(name);
        hand.click(&path, false);
        let i = ctl.state.borrow().order().iter().position(|p| *p == path).unwrap() as i32;
        hand.lib().invoke_tile_clicked(i, false, false);
        hand.rig.pump();
        assert!(!w.global::<App>().get_gallery(), "the editor open on {name}");
        hand.rig.until("the frame decoded", Duration::from_secs(30), || w.global::<oni_app::Canvas>().get_status().contains(" × "));
        hand.rig.pump();
    };
    // Twin: an original the cloud still keeps after its decode stays read-only, its sidecar as it was.
    let a = sidecar::url_for(Path::new(&hand.at("Roll/A.tif")));
    let before = std::fs::read(&a).unwrap();
    opened("Roll/A.tif");
    assert!(w.global::<oni_app::Canvas>().get_read_only());
    assert_eq!(std::fs::read(&a).unwrap(), before);
    hand.rig.chord(Key::Control, "g");
    // Downloaded by its decode: read again, brought, and editable.
    hand.rig.pump();
    let b = sidecar::url_for(Path::new(&hand.at("Roll/B.tif")));
    let path = hand.at("Roll/B.tif");
    hand.click(&path, false);
    // No preview kept from the gallery: the opening decodes the file, as one the cloud keeps.
    ctl.state.borrow().forget_previews();
    let i = ctl.state.borrow().order().iter().position(|p| *p == path).unwrap() as i32;
    hand.lib().invoke_tile_clicked(i, false, false);
    assert!(w.global::<oni_app::Canvas>().get_read_only(), "opened while its original was in the cloud");
    CLOUD.store(false, std::sync::atomic::Ordering::Relaxed);
    hand.rig.until("read again once decoded", Duration::from_secs(30), || !w.global::<oni_app::Canvas>().get_read_only());
    assert_eq!(migrate::standing(&std::fs::read(&b).unwrap()), Standing::Current);
}
