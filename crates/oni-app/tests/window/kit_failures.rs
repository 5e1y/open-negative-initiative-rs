//! The Kit when The Lens DB says no or nothing: a run that names no service asks none; a 429 holds every
//! search until its Retry-After, then asks the latest once; Done unreached keeps its picks and sends them again;
//! Done signed out keeps the copy and says why on the offer; a firewall's page at the first use leaves the copy shown.

use crate::common;
use crate::kit::{self, EMPTY, WAIT, calls, piece_names};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::TldbSite;
use oni_app::{Kit, KitLink};
use oni_tldb::fake::Fake;
use oni_tldb::{Kept, Read};
use slint::{ComponentHandle, Model};
use std::time::{Duration, Instant};

/// The note shown in the window as a text, so the person sees why.
fn note_shown(w: &oni_app::MainWindow, kit: &Kit) -> bool {
    let note = kit.get_note();
    !note.is_empty() && !common::placed(w, Role::Text, &note).is_empty()
}

/// The Kit's column, the search sheet opened, the Kit read once its token is, and `words` typed, its answers landed.
fn searched(stage: &kit::Stage, words: &str) {
    let (rig, w) = (&stage.rig, &stage.shell.window);
    let kit = w.global::<Kit>();
    opened_search(stage);
    common::click(rig, w, common::the(w, Role::TextInput, "Search The Lens DB").middle());
    rig.key(words);
    rig.until("the search answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
}

/// The Kit's column, signed in by the copy's mark, then its search sheet, which reads the token and the Kit.
fn opened_search(stage: &kit::Stage) {
    let (rig, w) = (&stage.rig, &stage.shell.window);
    let kit = w.global::<Kit>();
    common::click(rig, w, EMPTY);
    rig.settle();
    assert_eq!(kit.get_link(), KitLink::In, "signed in by the copy's mark");
    common::click(rig, w, common::the(w, Role::Button, "Search Gear").middle());
    rig.until("the Kit read", WAIT, || kit.get_bodies().row_count() > 0);
    rig.settle();
}

fn picked_fm2(fake: &Fake, name: &str) -> Option<kit::Stage> {
    let stage = kit::stage(name, TldbSite::Simulated(fake.site.clone()), kit::signed_in(fake))?;
    searched(&stage, "nikon fm2");
    let w = &stage.shell.window;
    common::click(&stage.rig, w, common::the(w, Role::Button, "Add").middle());
    stage.rig.settle();
    assert_eq!(w.global::<Kit>().get_picked(), 1);
    Some(stage)
}

pub(crate) fn a_run_naming_no_service_asks_none_and_connect_opens_nothing() {
    let fake = oni_tldb::fake::start();
    // A token left in the folder is never read: no service is named.
    let Some(stage) = kit::stage("kit-none", TldbSite::None, kit::signed_in(&fake)) else { return };
    let (rig, w) = (&stage.rig, &stage.shell.window);
    let kit = w.global::<Kit>();
    common::click(rig, w, EMPTY);
    rig.settle();
    assert_eq!(kit.get_link(), KitLink::Out, "no token read");
    common::click(rig, w, common::the(w, Role::Button, "Connect The Lens DB").middle());
    rig.settle();
    assert_eq!(kit.get_link(), KitLink::Out);
    assert!(stage.opened.borrow().is_empty(), "no sign-in page opened");
    assert!(fake.journal().is_empty(), "nothing asked: {:?}", fake.journal());
    // Twin: the firewall's check below names the service, and its first use reads the Kit once.
    let _ = std::fs::remove_dir_all(&stage.root);
}

pub(crate) fn a_429_holds_every_search_until_its_end_then_asks_the_latest_once() {
    let fake = oni_tldb::fake::start();
    let Some(stage) = kit::stage("kit-wait", TldbSite::Simulated(fake.site.clone()), kit::signed_in(&fake)) else { return };
    let (rig, w) = (&stage.rig, &stage.shell.window);
    let kit = w.global::<Kit>();
    opened_search(&stage);
    common::click(rig, w, common::the(w, Role::TextInput, "Search The Lens DB").middle());
    // One call in ten seconds: the read of the Kit as the sheet opened took it.
    fake.set_rate(1, Duration::from_secs(10));
    rig.key("leica");
    rig.until("the service asks to wait", WAIT, || kit.get_note().contains("wait"));
    assert!(note_shown(w, &kit), "the wait said in the sheet");
    let held = calls(&fake, "/search");
    assert_eq!((held.len(), held[0].status), (1, 429));
    rig.key(" m6");
    let typed = Instant::now();
    // Twin: a search inside the window is refused, so the one the hand asked for was held, not lost.
    assert!(matches!(fake.client().search("leica m6", None, None, 5), Err(oni_tldb::Failure::Wait(_))));
    rig.until("the latest asked once the wait is over", WAIT, || kit.get_searched() == "leica m6");
    let searches: Vec<_> = calls(&fake, "/search").into_iter().filter(|s| s.bearer == held[0].bearer).collect();
    assert_eq!(searches.len(), 2, "no search inside the wait, then one: {searches:?}");
    assert_eq!((searches[1].param("q"), searches[1].status), (Some("leica m6"), 200));
    assert!(searches[1].at > typed + Duration::from_millis(500), "asked after the wait, not as typed");
    let _ = std::fs::remove_dir_all(&stage.root);
}

pub(crate) fn done_unreached_keeps_its_picks_and_sends_them_again() {
    let fake = oni_tldb::fake::start();
    let Some(stage) = picked_fm2(&fake, "kit-down") else { return };
    let (rig, w) = (&stage.rig, &stage.shell.window);
    let kit = w.global::<Kit>();
    let token = std::fs::read_to_string(stage.support.join("tldb-fake-token")).unwrap();
    fake.set_down(true);
    common::click(rig, w, common::the(w, Role::Button, "Done").middle());
    rig.until("the change failed", WAIT, || !kit.get_sending() && !kit.get_note().is_empty());
    rig.settle();
    assert!(kit.get_picking() && kit.get_picked() == 1, "the sheet stays with its pick");
    assert!(note_shown(w, &kit), "the note in the sheet");
    assert_eq!(std::fs::read_to_string(stage.support.join("tldb-fake-token")).unwrap(), token, "still signed in");
    println!("shot {}", rig.shot("kit-search-failed").display());
    fake.set_down(false);
    common::click(rig, w, common::the(w, Role::Button, "Done").middle());
    rig.until("sent and closed", WAIT, || !kit.get_picking());
    let posts = calls(&fake, "/kit/changes");
    assert_eq!(posts.iter().map(|s| s.status).collect::<Vec<_>>(), [503, 200], "one more call, applied");
    assert_eq!(piece_names(kit.get_bodies()), ["Canon AE-1 Program", "Nikon FM2"]);
    let _ = std::fs::remove_dir_all(&stage.root);
}

pub(crate) fn done_signed_out_keeps_the_copy_and_says_why_on_the_offer() {
    let fake = oni_tldb::fake::start();
    let Some(stage) = picked_fm2(&fake, "kit-out") else { return };
    let (rig, w) = (&stage.rig, &stage.shell.window);
    let kit = w.global::<Kit>();
    fake.sign_out_everyone();
    common::click(rig, w, common::the(w, Role::Button, "Done").middle());
    rig.until("signed out", WAIT, || kit.get_link() == KitLink::Out);
    rig.settle();
    assert!(!kit.get_picking(), "the sheet closed, its picks dropped");
    assert!(note_shown(w, &kit), "the offer says why");
    assert!(Kept::file_path(&stage.support).exists(), "the copy kept");
    rig.until("the token forgotten", WAIT, || !stage.support.join("tldb-fake-token").exists());
    assert!(!Kept::load(&stage.support).signed_in(), "the copy marked signed out");
    assert_eq!(fake.kit_keys().len(), 2, "twin: nothing was added");
    let _ = std::fs::remove_dir_all(&stage.root);
}

pub(crate) fn a_firewall_at_the_first_use_leaves_the_copy_shown_and_says_why() {
    let fake = oni_tldb::fake::start();
    let Ok(Read::Fresh { kit: read, .. }) = fake.client().kit(None) else { panic!("the simulated Kit") };
    let before = calls(&fake, "/kit").len();
    let token = kit::signed_in(&fake);
    let copy = move |support: &std::path::Path, caches: &std::path::Path| {
        token(support, caches);
        // A copy written before the mark: its account stands for the token kept.
        Kept { account: read.account.display_name, items: read.items, etag: Some("W/\"older\"".into()), signed: None }.save(support).unwrap();
    };
    fake.set_firewall(Some(403));
    let Some(stage) = kit::stage("kit-firewall", TldbSite::Simulated(fake.site.clone()), copy) else { return };
    let (rig, w) = (&stage.rig, &stage.shell.window);
    let kit = w.global::<Kit>();
    common::click(rig, w, EMPTY);
    rig.settle();
    assert_eq!(calls(&fake, "/kit").len(), before, "the launch asks the service nothing");
    assert_eq!((kit.get_link(), piece_names(kit.get_bodies())), (KitLink::In, vec!["Canon AE-1 Program".to_string()]), "the copy shown, signed in by its account");
    common::click(rig, w, common::the(w, Role::Button, "Search Gear").middle());
    rig.until("the first use's read failed", WAIT, || !kit.get_note().is_empty());
    rig.key(&slint::SharedString::from(slint::platform::Key::Escape));
    rig.settle();
    assert_eq!(calls(&fake, "/kit").len(), before + 1, "the first use read the Kit once");
    assert_eq!(kit.get_link(), KitLink::In, "a firewall signs no one out");
    assert_eq!(piece_names(kit.get_bodies()), ["Canon AE-1 Program"], "the copy shown");
    assert!(note_shown(w, &kit), "the column says why");
    let _ = std::fs::remove_dir_all(&stage.root);
}
