//! The Kit against a simulated The Lens DB, driven by clicks and keys as a hand would: a click on the
//! gallery's empty room offers the service; Connect opens its sign-in in the browser and the Kit shows once it
//! returns; Search Gear searches in a sheet whose Add puts a piece in the Kit until Save; a card's menu opens its
//! page, makes it the default or removes it; a removal asks at Save;
//! leaving the Kit with changes not saved asks about them; a default body and lens fill an export where nothing
//! names one, its roll keeping the body; Disconnect revokes the token and drops the copy.

use oni_app::InfoKind;
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::export_card;
use oni_app::app::{Options, Shell};
use oni_app::{Confirm, Exporting, Gear, Kit, KitLink, KitMark, Library};
use oni_core::{Archive, EditorStore};
use oni_library::index::{Entry, LibraryIndex};
use oni_settings::{Date, Film};
use oni_tldb::{Kept, Kind};
use slint::{ComponentHandle, Model, SharedString};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use crate::common;

const WAIT: Duration = Duration::from_secs(20);

fn names(model: slint::ModelRc<SharedString>) -> Vec<String> {
    model.iter().map(|s| s.to_string()).collect()
}

fn piece_names(model: slint::ModelRc<oni_app::KitPiece>) -> Vec<String> {
    model.iter().map(|p| p.name.to_string()).collect()
}

/// What the browser does with the service's redirect: a GET on ONI's loopback port.
fn follow(url: &str) {
    let rest = url.strip_prefix("http://").expect("a loopback address");
    let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let mut stream = std::net::TcpStream::connect(host).expect("ONI's port listens");
    write!(stream, "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").unwrap();
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    assert!(answer.starts_with("HTTP/1.1 200"), "the page ONI answers: {answer}");
}

pub(crate) fn the_kit_signs_in_searches_and_validates() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Kit is not exercised"),
    };
    let fake = oni_tldb::fake::start();
    // SAFETY: this check runs alone in its process, before any thread reads the environment.
    unsafe { std::env::set_var("ONI_TLDB", &fake.site) };
    let root = std::env::temp_dir().join(format!("oni-app-kit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support) = (root.join("Scans"), root.join("Support"));
    for name in ["A.tif", "B.tif"] {
        common::linear_tiff(&scans.join(name), 48, 32, 0.5);
    }
    oni_settings::film::roll_folder::set_roll(Some(&Film::new("Scans")), &scans).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    // A roll read before, its body typed by hand: the names typed in ONI take it in as the app starts.
    let caches = root.join("Caches");
    {
        let index = LibraryIndex::open(&LibraryIndex::default_path(&caches)).expect("the library index");
        let earlier = root.join("Earlier").join("C.tif").to_string_lossy().into_owned();
        index.upsert(&Entry::new(&earlier, Date::since_reference_date(800_000_000.0), "c-1")).unwrap();
        let mut roll = Film::new("Earlier");
        roll.camera_body = Some("Pentax MX".into());
        index.set_film(Some(&roll), &earlier).unwrap();
    }
    let opened: Rc<RefCell<Vec<String>>> = Rc::default();
    let browser = opened.clone();
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), browse: Box::new(move |url| browser.borrow_mut().push(url.to_string())), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    shell.open_folder(&scans);
    rig.until("the photos read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().order().len() == 2);
    rig.settle();
    let (lib, gear, kit) = (w.global::<Library>(), w.global::<Gear>(), w.global::<Kit>());
    assert_eq!(names(gear.get_bodies()), ["Pentax MX"], "signed out, the export offers the names typed in ONI");

    // Nothing picked: the column offers The Lens DB; twin: on a roll, it does not.
    assert!(common::placed(w, Role::Button, "Connect The Lens DB").is_empty(), "twin: no offer on a roll");
    common::click(&rig, w, (500.0, 700.0));
    rig.settle();
    assert_eq!(lib.get_info_kind(), InfoKind::Collection);
    assert_eq!(kit.get_link(), KitLink::Out);
    assert!(common::placed(w, Role::TextInput, "Search The Lens DB").is_empty(), "twin: no search before the sign-in");
    println!("shot {}", rig.shot("kit-offer").display());

    // Connect opens the service's sign-in; the Kit shows once the browser comes back to ONI's port.
    common::click(&rig, w, common::the(w, Role::Button, "Connect The Lens DB").middle());
    rig.settle();
    assert_eq!(kit.get_link(), KitLink::Waiting);
    let authorize = opened.borrow().last().cloned().expect("the browser opened");
    assert!(authorize.starts_with(&format!("{}/oauth/authorize?", fake.site)) && authorize.contains("code_challenge_method=S256"), "{authorize}");
    let back = fake.approve(&authorize, false);
    std::thread::spawn(move || follow(&back));
    rig.until("signed in, the Kit read", WAIT, || kit.get_link() == KitLink::In && kit.get_bodies().row_count() > 0);
    assert_eq!(kit.get_account(), "Camille");
    assert_eq!(piece_names(kit.get_bodies()), ["Canon AE-1 Program"]);
    assert_eq!(piece_names(kit.get_lenses()), ["Canon FD 50mm f/1.4"]);
    assert!(Kept::load(&support).items.len() == 2, "the copy kept in the support folder");
    let cards = |m: slint::ModelRc<oni_app::GearCard>| m.iter().map(|c| c.name.to_string()).collect::<Vec<_>>();
    assert_eq!(cards(gear.get_body_cards()), ["Canon AE-1 Program"], "the gear fields offer the Kit as cards");
    assert_eq!(names(gear.get_bodies()), ["Pentax MX"], "then the names typed that the Kit does not hold");
    println!("shot {}", rig.shot("kit-signed-in").display());

    // Search Gear opens the search over the window; a brand's quick filter asks for its pieces alone, again
    // for none; its Add puts the FM2 in the Kit, nothing sent before Save.
    common::click(&rig, w, common::the(w, Role::Button, "Search Gear").middle());
    rig.settle();
    assert!(kit.get_picking(), "the search's sheet is shown");
    common::click(&rig, w, common::the(w, Role::Button, "Leica").middle());
    rig.until("the brand answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
    assert!(kit.get_found().iter().all(|p| p.name.starts_with("Leica")), "{:?}", piece_names(kit.get_found()));
    common::click(&rig, w, common::the(w, Role::Button, "Leica").middle());
    rig.until("the filter let go", WAIT, || !kit.get_searching() && kit.get_found().row_count() == 0);
    assert_eq!(kit.get_brand_filter(), "");
    common::click(&rig, w, common::the(w, Role::TextInput, "Search The Lens DB").middle());
    rig.key("nikon fm2");
    rig.until("the search answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
    rig.settle();
    println!("shot {}", rig.shot("kit-search").display());
    common::click(&rig, w, common::the(w, Role::Button, "Add").middle());
    rig.settle();
    assert!(kit.get_found().iter().any(|f| f.name == "Nikon FM2" && f.mark == KitMark::Added), "the FM2 added");
    assert!(!common::placed(w, Role::Button, "Added").is_empty(), "its key says so");
    assert_eq!(fake.kit_keys().len(), 2, "twin: nothing reaches the service before Save");
    common::click(&rig, w, common::the(w, Role::Button, "Done").middle());
    rig.settle();
    assert!(!kit.get_picking());
    assert_eq!(piece_names(kit.get_bodies()), ["Canon AE-1 Program", "Nikon FM2"], "the Kit shows it, not saved yet");
    assert_eq!(kit.get_unsaved(), 1);
    println!("shot {}", rig.shot("kit-unsaved").display());
    common::click(&rig, w, common::the(w, Role::Button, "Save").middle());
    rig.until("the change saved", WAIT, || kit.get_unsaved() == 0 && !kit.get_sending());
    assert_eq!(fake.kit_keys().iter().filter(|k| k.kind == Kind::Camera).count(), 2, "the service holds it now: {:?}", fake.kit_keys());
    assert!(kit.get_bodies().iter().all(|p| p.mark == KitMark::Kept));

    // A removal from a card's menu waits for Save, which asks first, as it erases what the person noted of the
    // piece on the service.
    let menu = |piece: &str, line: &str| {
        common::click(&rig, w, common::the(w, Role::Button, &format!("More for {piece}")).middle());
        rig.settle();
        common::click(&rig, w, common::in_popup(w, Role::Button, line).middle());
        rig.settle();
    };
    menu("Canon AE-1 Program", "Remove from My Kit");
    assert!(kit.get_bodies().row_data(0).unwrap().mark == KitMark::Leaving && kit.get_unsaved() == 1);
    assert!(opened.borrow().len() == 1, "twin: the menu opens no page unasked");
    common::click(&rig, w, common::the(w, Role::Button, "Save").middle());
    rig.settle();
    let confirm = w.global::<Confirm>();
    assert!(confirm.get_shown() && confirm.get_destructive(), "Save asks before removing");
    confirm.invoke_answered(false);
    rig.settle();
    assert_eq!(fake.kit_keys().len(), 3, "twin: Cancel removes nothing");

    // Leaving the Kit with the removal not saved asks; Later keeps it, Save sends it.
    lib.invoke_tile_clicked(0, false, false);
    rig.settle();
    assert!(kit.get_asking(), "leaving the Kit asks about its changes");
    println!("shot {}", rig.shot("kit-ask").display());
    common::click(&rig, w, common::the(w, Role::Button, "Later").middle());
    rig.settle();
    assert!(!kit.get_asking() && kit.get_unsaved() == 1, "Later keeps the change");
    common::click(&rig, w, (500.0, 700.0));
    rig.settle();
    assert_eq!(lib.get_info_kind(), InfoKind::Collection);
    assert!(!kit.get_asking(), "twin: coming back to the Kit asks nothing");
    common::click(&rig, w, common::the(w, Role::Button, "Save").middle());
    rig.settle();
    confirm.invoke_answered(true);
    rig.until("the removal saved", WAIT, || kit.get_unsaved() == 0 && !kit.get_sending());
    assert_eq!(piece_names(kit.get_bodies()), ["Nikon FM2"]);
    assert_eq!(fake.kit_keys().len(), 2);

    // The FM2 and the FD 50 made the defaults: an export of the roll, which names no body, shows them.
    menu("Nikon FM2", "Set as Default");
    menu("Canon FD 50mm f/1.4", "Set as Default");
    assert!(kit.get_bodies().row_data(0).unwrap().by_default && kit.get_lenses().row_data(0).unwrap().by_default);
    println!("shot {}", rig.shot("kit-defaults").display());
    let e = w.global::<Exporting>();
    lib.invoke_tile_clicked(0, false, false);
    ctl.act(oni_app::Act::Export);
    rig.until("the line read", Duration::from_secs(60), || export_card::lines(w).iter().all(|l| l.ready));
    let line = export_card::lines(w)[0].clone();
    assert_eq!((line.camera.as_str(), line.lens.as_str()), ("Nikon FM2", "Canon FD 50mm f/1.4"), "the defaults fill the line");
    let out = root.join("Out").join("A.tiff");
    export_card::answer_with(&ctl, Rc::new(move |_| vec![out.clone()]));
    e.invoke_export();
    rig.until("the frame written", Duration::from_secs(60), || !export_card::card(&ctl).exporting && !e.get_shown());
    let roll = oni_settings::film::roll_folder::roll(&scans).expect("the roll");
    assert_eq!(roll.camera_body.as_deref(), Some("Nikon FM2"), "the roll keeps the body its frame was written with");
    let kept = |name: &str| oni_settings::sidecar::read(&scans.join(name), false).map(|l| l.exif).unwrap_or_default();
    assert_eq!(kept("A.tif").lens.as_deref(), Some("Canon FD 50mm f/1.4"), "the frame written keeps its lens");
    assert_eq!(kept("A.tif").camera, None, "the body is the roll's, which a later one replaces");
    let photos = |m: slint::ModelRc<oni_app::KitPiece>| m.row_data(0).unwrap().photos;
    assert_eq!((photos(kit.get_bodies()), photos(kit.get_lenses())), (2, 1), "the roll's two photos name the body, the frame written the lens");
    // Twin: the defaults let go, the roll's other frame still names the body and no longer the lens.
    common::click(&rig, w, (500.0, 700.0));
    rig.settle();
    menu("Nikon FM2", "Remove Default");
    menu("Canon FD 50mm f/1.4", "Remove Default");
    assert!(kit.get_bodies().iter().chain(kit.get_lenses().iter()).all(|p| !p.by_default));
    assert_eq!(kept("B.tif").lens, None);
    lib.invoke_tile_clicked(1, false, false);
    ctl.act(oni_app::Act::Export);
    rig.until("the other line read", Duration::from_secs(60), || export_card::lines(w).iter().all(|l| l.ready));
    let other = export_card::lines(w)[0].clone();
    assert_eq!((other.camera.as_str(), other.lens.as_str()), ("Nikon FM2", ""));
    e.invoke_cancel();
    rig.settle();

    // The roll's Camera Body offers the Kit as its cards: one picked names the roll's body.
    common::click(&rig, w, common::the(w, Role::Button, "Pick From Your Kit").middle());
    rig.settle();
    println!("shot {}", rig.shot("kit-field").display());
    common::click(&rig, w, common::in_popup(w, Role::Button, "Nikon FM2").middle());
    rig.settle();
    assert_eq!(lib.get_camera_body(), "Nikon FM2");

    // A card's menu opens its page on the service.
    common::click(&rig, w, (800.0, 400.0));
    rig.settle();
    menu("Nikon FM2", "See on The Lens DB");
    assert_eq!(opened.borrow().len(), 2, "the card's page opened");

    // Settings' Connected Apps disconnects: the token revoked, the copy dropped, the typed names still offered.
    ctl.act(oni_app::Act::Settings);
    w.global::<oni_app::AppSettings>().set_section(5);
    rig.settle();
    common::click(&rig, w, common::the(w, Role::Button, "Disconnect").middle());
    rig.settle();
    ctl.act(oni_app::Act::CloseSettings);
    rig.settle();
    assert_eq!(kit.get_link(), KitLink::Out);
    assert!(!Kept::file_path(&support).exists(), "the copy dropped");
    assert_eq!(names(gear.get_bodies()), ["Nikon FM2", "Pentax MX"], "the body a hand picked for the roll stays among the names typed");
    assert!(opened.borrow().len() == 2, "twin: no page opened but the sign-in and the card's");
    let _ = std::fs::remove_dir_all(&root);
}
