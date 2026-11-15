//! The Kit against a simulated The Lens DB: a click on the gallery's empty room offers the service; Connect
//! opens its sign-in in the browser and the Kit shows once it returns; a search adds pieces that leave for the
//! service only at Validate; a removal asks first; the export's Camera offers the Kit before the names typed in
//! ONI; Disconnect revokes the token and drops the copy.

use oni_app::InfoKind;
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::{Confirm, Gear, GearKind, Kit, KitLink, Library};
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
    assert_eq!(names(gear.get_bodies()), ["Canon AE-1 Program", "Pentax MX"], "the Kit first in the export's list, then the typed names");
    println!("shot {}", rig.shot("kit-signed-in").display());

    // A search's answer joins the pieces to validate; nothing leaves for the service before Validate.
    kit.invoke_search("nikon fm2".into());
    rig.until("the search answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
    let fm2 = kit.get_found().iter().position(|f| f.name == "Nikon FM2").expect("the FM2 found");
    kit.invoke_add(fm2 as i32);
    rig.settle();
    assert_eq!(piece_names(kit.get_adding()), ["Nikon FM2"]);
    assert!(kit.get_found().row_data(fm2).is_some_and(|f| f.waiting && !f.held), "an answer added waits for Validate");
    assert_eq!(fake.kit_keys().len(), 2, "twin: nothing reaches the service before Validate");
    println!("shot {}", rig.shot("kit-to-validate").display());
    kit.invoke_validate();
    rig.until("the change sent", WAIT, || kit.get_adding().row_count() == 0 && !kit.get_sending());
    assert_eq!(piece_names(kit.get_bodies()), ["Canon AE-1 Program", "Nikon FM2"]);
    assert_eq!(fake.kit_keys().iter().filter(|k| k.kind == Kind::Camera).count(), 2, "the service holds it now: {:?}", fake.kit_keys());

    // A removal asks first, as it erases what the person noted of the piece on the service.
    kit.invoke_remove(GearKind::Body, 0);
    rig.settle();
    assert!(kit.get_bodies().row_data(0).unwrap().removing && kit.get_removals() == 1);
    kit.invoke_validate();
    rig.settle();
    let confirm = w.global::<Confirm>();
    assert!(confirm.get_shown() && confirm.get_destructive(), "Validate asks before removing");
    confirm.invoke_answered(false);
    rig.settle();
    assert_eq!(fake.kit_keys().len(), 3, "twin: Cancel removes nothing");
    kit.invoke_validate();
    rig.settle();
    confirm.invoke_answered(true);
    rig.until("the removal sent", WAIT, || kit.get_removals() == 0 && !kit.get_sending());
    assert_eq!(piece_names(kit.get_bodies()), ["Nikon FM2"]);
    assert_eq!(fake.kit_keys().len(), 2);

    // Disconnect revokes the token and drops the copy; the typed names stay offered.
    kit.invoke_disconnect();
    rig.settle();
    assert_eq!(kit.get_link(), KitLink::Out);
    assert!(!Kept::file_path(&support).exists(), "the copy dropped");
    assert_eq!(names(gear.get_bodies()), ["Pentax MX"]);
    assert!(opened.borrow().len() == 1, "twin: no page opened but the sign-in");
    let _ = std::fs::remove_dir_all(&root);
}
