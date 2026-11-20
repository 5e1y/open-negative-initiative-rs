//! The Kit against a simulated The Lens DB, driven by clicks and keys as a hand would: a click on the
//! gallery's empty room offers the service; Connect opens its sign-in in the browser and the Kit shows once it
//! returns; Search Gear searches in a sheet, filtered by kind, whose ↗ opens a
//! piece's page and whose Add picks it, Done or Escape sending the picks in one call; a card's menu opens its
//! page, makes it the default or removes it once confirmed; a default body and lens fill an export where
//! nothing names one, its roll keeping the body; Disconnect revokes the token and drops the copy. Every call
//! names ONI and carries no Origin.

use crate::common;
use crate::kit::{self, EMPTY, WAIT, calls, names, piece_names};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::InfoKind;
use oni_app::app::{TldbSite, export_card};
use oni_app::{Confirm, Exporting, Gear, Kit, KitLink, KitMark, Library};
use oni_library::index::{Entry, LibraryIndex};
use oni_settings::{Date, Film};
use oni_tldb::{Kept, Kind};
use slint::{ComponentHandle, Model};
use std::rc::Rc;
use std::time::Duration;

/// A roll read before, its body typed by hand: the names typed in ONI take it in as the app starts.
fn earlier_roll(root: &std::path::Path, caches: &std::path::Path) {
    let index = LibraryIndex::open(&LibraryIndex::default_path(caches)).expect("the library index");
    let earlier = root.join("Earlier").join("C.tif").to_string_lossy().into_owned();
    index.upsert(&Entry::new(&earlier, Date::since_reference_date(800_000_000.0), "c-1")).unwrap();
    let mut roll = Film::new("Earlier");
    roll.camera_body = Some("Pentax MX".into());
    index.set_film(Some(&roll), &earlier).unwrap();
}

pub(crate) fn the_kit_signs_in_searches_and_validates() {
    let fake = oni_tldb::fake::start();
    let Some(stage) = kit::stage("kit", TldbSite::Simulated(fake.site.clone()), |support, caches| earlier_roll(support.parent().unwrap(), caches)) else { return };
    let kit::Stage { rig, shell, root, scans, support, opened } = &stage;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (lib, gear, kit) = (w.global::<Library>(), w.global::<Gear>(), w.global::<Kit>());
    assert_eq!(names(gear.get_bodies()), ["Pentax MX"], "signed out, the export offers the names typed in ONI");

    // Nothing picked: the column offers The Lens DB, credited; twin: on a roll, it does not.
    assert!(common::placed(w, Role::Button, "Connect The Lens DB").is_empty(), "twin: no offer on a roll");
    common::click(rig, w, EMPTY);
    rig.settle();
    assert_eq!(lib.get_info_kind(), InfoKind::Collection);
    assert_eq!(kit.get_link(), KitLink::Out);
    assert!(common::placed(w, Role::Button, "Catalogue provided by The Lens DB").is_empty(), "the offer names the two projects, no credit line");
    assert_eq!(common::placed(w, Role::Button, "The Lens DB").len(), 1, "ONI × The Lens DB");
    assert_eq!(common::placed(w, Role::Image, "ONI").len(), 1, "the app's logo beside the mark");
    println!("shot {}", rig.shot("kit-offer").display());

    // Connect opens the service's sign-in; the Kit shows once the browser comes back to ONI's port.
    common::click(rig, w, common::the(w, Role::Button, "Connect The Lens DB").middle());
    rig.settle();
    assert_eq!(kit.get_link(), KitLink::Waiting);
    let authorize = opened.borrow().last().cloned().expect("the browser opened");
    assert!(authorize.starts_with(&format!("{}/oauth/authorize?", fake.site)) && authorize.contains("code_challenge_method=S256"), "{authorize}");
    let back = fake.approve(&authorize, false);
    std::thread::spawn(move || kit::follow(&back));
    rig.until("signed in, the Kit read", WAIT, || kit.get_link() == KitLink::In && kit.get_bodies().row_count() > 0);
    assert_eq!(kit.get_account(), "Camille");
    assert_eq!(piece_names(kit.get_bodies()), ["Canon AE-1 Program"]);
    assert_eq!(piece_names(kit.get_lenses()), ["Canon FD 50mm f/1.4"]);
    assert!(Kept::load(support).items.len() == 2, "the copy kept in the support folder");
    let cards = |m: slint::ModelRc<oni_app::GearCard>| m.iter().map(|c| c.name.to_string()).collect::<Vec<_>>();
    assert_eq!(cards(gear.get_body_cards()), ["Canon AE-1 Program"], "the gear fields offer the Kit as cards");
    assert_eq!(names(gear.get_bodies()), ["Pentax MX"], "then the names typed that the Kit does not hold");
    assert_eq!(common::placed(w, Role::Button, "Catalogue provided by The Lens DB").len(), 1, "the column credits the catalogue");
    println!("shot {}", rig.shot("kit-signed-in").display());

    // Search Gear; ↗ opens the FM2's page, Add picks it, nothing sent until Done, which sends one call and closes.
    common::click(rig, w, common::the(w, Role::Button, "Search Gear").middle());
    rig.settle();
    assert!(kit.get_picking(), "the search's sheet is shown");
    assert_eq!(common::placed(w, Role::Button, "Catalogue provided by The Lens DB").len(), 2, "the sheet credits it too");
    common::click(rig, w, common::the(w, Role::TextInput, "Search The Lens DB").middle());
    rig.key("nikon fm2");
    rig.until("the search answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
    rig.settle();
    let pages = opened.borrow().len();
    assert!(common::placed(w, Role::Button, "Add").len() >= 1 && common::placed(w, Role::Button, "More for Nikon FM2").is_empty(), "the search's card ends with its Add");
    common::click(rig, w, common::the(w, Role::Button, "See Nikon FM2 on The Lens DB").middle());
    let page = opened.borrow().last().cloned().unwrap();
    assert!(page.starts_with(&format!("{}/cameras/nikon-fm2", fake.site)) && page.ends_with("utm_source=oni"), "{page}");
    common::click(rig, w, common::the(w, Role::Button, "Add").middle());
    rig.settle();
    assert_eq!(opened.borrow().len(), pages + 1, "twin: Add opens no page");
    assert!(kit.get_found().iter().any(|f| f.name == "Nikon FM2" && f.mark == KitMark::Added), "the FM2 picked");
    assert_eq!(kit.get_picked(), 1);
    assert!(calls(&fake, "/kit/changes").is_empty(), "twin: nothing reaches the service before Done");
    common::click(rig, w, common::the(w, Role::Button, "Done").middle());
    rig.until("Done sent and closed", WAIT, || !kit.get_picking());
    assert_eq!(calls(&fake, "/kit/changes").len(), 1, "one call for the picks");
    assert_eq!(fake.kit_keys().iter().filter(|k| k.kind == Kind::Camera).count(), 2, "the service holds it: {:?}", fake.kit_keys());
    assert_eq!(piece_names(kit.get_bodies()), ["Canon AE-1 Program", "Nikon FM2"]);

    // Opened again, the sheet starts bare, the kind a select beside the search; Lenses asks again with
    // it. Escape sends its pick as Done does.
    common::click(rig, w, common::the(w, Role::Button, "Search Gear").middle());
    rig.settle();
    assert_eq!((kit.get_kind_filter(), kit.get_picked(), kit.get_found().row_count()), (0, 0, 0));
    assert!(common::placed(w, Role::Tab, "Bodies").is_empty(), "the kind is a select, no tabs");
    let (field, kind) = (common::the(w, Role::TextInput, "Search The Lens DB"), common::the(w, Role::Button, "Bodies & Lenses"));
    assert!(kind.x > field.x + field.width && (kind.y + kind.height / 2.0 - field.y - field.height / 2.0).abs() < 2.0, "the select right of the search: {field:?} {kind:?}");
    let (title, done) = (common::the(w, Role::Text, "Add to Your Kit"), common::the(w, Role::Button, "Done"));
    let credit = common::placed(w, Role::Button, "Catalogue provided by The Lens DB").into_iter().max_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
    assert!((credit.x - title.x).abs() < 1.0 && (kind.x + kind.width - done.x - done.width).abs() < 1.0, "the content on the header's margins: {title:?} {credit:?} {kind:?} {done:?}");
    common::click(rig, w, field.middle());
    rig.key("50");
    rig.until("the search answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
    rig.settle();
    // A press beside the field in the sheet lets it go: the sheet takes the keyboard back.
    assert_eq!(common::focused_label(w), "Search The Lens DB");
    common::click(rig, w, common::the(w, Role::Text, "Add to Your Kit").middle());
    rig.settle();
    assert_ne!(common::focused_label(w), "Search The Lens DB", "the field let go");
    assert!(kit.get_picking(), "twin: the sheet still open");
    println!("shot {}", rig.shot("kit-search").display());
    assert!(kit.get_found().iter().any(|p| p.detail.starts_with("Body")), "twin: both kinds before Lenses");
    common::click(rig, w, kind.middle());
    rig.settle();
    common::click(rig, w, common::in_popup(w, Role::Button, "Lenses").middle());
    rig.until("the lenses answered", WAIT, || !kit.get_searching() && kit.get_kind_filter() == 2);
    assert_eq!(calls(&fake, "/search").last().and_then(|s| s.param("type").map(str::to_string)).as_deref(), Some("lens"));
    let nikkor = kit.get_found().iter().position(|p| p.name == "Nikon Nikkor 50mm f/1.8 AI-S").expect("the Nikkor among the lenses");
    kit.invoke_take(nikkor as i32);
    rig.settle();
    rig.key(&slint::SharedString::from(slint::platform::Key::Escape));
    rig.until("Escape sent and closed", WAIT, || !kit.get_picking());
    assert_eq!(calls(&fake, "/kit/changes").len(), 2);
    assert_eq!(piece_names(kit.get_lenses()), ["Canon FD 50mm f/1.4", "Nikon Nikkor 50mm f/1.8 AI-S"]);

    // A removal from a card's menu asks first, as it erases what the person noted of the piece on the
    // service; Cancel sends nothing, Remove one call.
    let menu = |piece: &str, line: &str| {
        common::click(rig, w, common::the(w, Role::Button, &format!("More for {piece}")).middle());
        rig.settle();
        common::click(rig, w, common::in_popup(w, Role::Button, line).middle());
        rig.settle();
    };
    menu("Canon AE-1 Program", "Remove from My Kit");
    let confirm = w.global::<Confirm>();
    assert!(confirm.get_shown() && confirm.get_destructive(), "a removal asks first");
    confirm.invoke_answered(false);
    rig.settle();
    assert_eq!(calls(&fake, "/kit/changes").len(), 2, "twin: Cancel sends nothing");
    menu("Canon AE-1 Program", "Remove from My Kit");
    confirm.invoke_answered(true);
    rig.until("the removal sent", WAIT, || !kit.get_sending() && kit.get_bodies().row_count() == 1);
    assert_eq!(calls(&fake, "/kit/changes").len(), 3);
    assert_eq!(piece_names(kit.get_bodies()), ["Nikon FM2"]);
    let (x, y) = common::the(w, Role::Button, "More for Nikon FM2").middle();
    rig.window.dispatch_event(slint::platform::WindowEvent::PointerMoved { position: slint::LogicalPosition::new(x, y) });
    rig.settle();
    println!("shot {}", rig.shot("kit-hover").display());
    let (x, y) = common::the(w, Role::Button, "See Nikon FM2 on The Lens DB").middle();
    rig.window.dispatch_event(slint::platform::WindowEvent::PointerMoved { position: slint::LogicalPosition::new(x, y) });
    rig.settle();
    println!("shot {}", rig.shot("kit-link-hover").display());

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
    let roll = oni_settings::film::roll_folder::roll(scans).expect("the roll");
    assert_eq!(roll.camera_body.as_deref(), Some("Nikon FM2"), "the roll keeps the body its frame was written with");
    let kept = |name: &str| oni_settings::sidecar::read(&scans.join(name), false).map(|l| l.exif).unwrap_or_default();
    assert_eq!(kept("A.tif").lens.as_deref(), Some("Canon FD 50mm f/1.4"), "the frame written keeps its lens");
    assert_eq!(kept("A.tif").camera, None, "the body is the roll's, which a later one replaces");
    let photos = |m: slint::ModelRc<oni_app::KitPiece>| m.row_data(0).unwrap().photos;
    assert_eq!((photos(kit.get_bodies()), photos(kit.get_lenses())), (2, 1), "the roll's two photos name the body, the frame written the lens");
    // Twin: the defaults let go, the roll's other frame still names the body and no longer the lens.
    common::click(rig, w, EMPTY);
    rig.settle();
    assert_eq!(lib.get_info_kind(), InfoKind::Collection, "a click on the gallery's empty room shows the Kit");
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

    // The roll's Camera Body is a combobox: clicked into, it drops the Kit as its cards under it while its
    // words are still typed; one picked names the roll's body, a name typed and Return names it too.
    assert!(common::placed(w, Role::Button, "Pick From Your Kit").is_empty(), "no key beside the field");
    common::click(rig, w, common::the(w, Role::TextInput, "Camera Body").middle());
    rig.settle();
    println!("shot {}", rig.shot("kit-field").display());
    common::click(rig, w, common::in_popup(w, Role::Button, "Nikon FM2").middle());
    rig.settle();
    assert_eq!(lib.get_camera_body(), "Nikon FM2");
    common::click(rig, w, common::the(w, Role::TextInput, "Camera Body").middle());
    rig.settle();
    assert_eq!(i_slint_core::window::WindowInner::from_pub(w.window()).active_popups().len(), 1, "the list dropped");
    rig.key("Rollei 35");
    assert_eq!(lib.get_camera_body(), "Rollei 35", "typed in the field itself, the list open under it");
    println!("shot {}", rig.shot("kit-field-typed").display());
    rig.key(&slint::SharedString::from(slint::platform::Key::Return));
    rig.settle();
    assert_eq!(names(gear.get_bodies()).iter().filter(|n| *n == "Rollei 35").count(), 1, "Return gives the name typed");
    common::click(rig, w, common::the(w, Role::TextInput, "Camera Body").middle());
    rig.settle();
    common::click(rig, w, common::in_popup(w, Role::Button, "Nikon FM2").middle());
    rig.settle();
    assert_eq!(lib.get_camera_body(), "Nikon FM2");

    // The Kit's card is the search's without its Add: its name opens its page on the service, the menu no more.
    common::click(rig, w, EMPTY);
    rig.settle();
    let pages = opened.borrow().len();
    common::click(rig, w, common::the(w, Role::Button, "See Nikon FM2 on The Lens DB").middle());
    assert_eq!(opened.borrow().len(), pages + 1, "the card's page opened");
    assert!(opened.borrow().last().unwrap().contains("/cameras/nikon-fm2") && opened.borrow().last().unwrap().ends_with("utm_source=oni"));
    common::click(rig, w, common::the(w, Role::Button, "More for Nikon FM2").middle());
    rig.settle();
    assert!(!common::placed(w, Role::Button, "Remove from My Kit").is_empty(), "the menu open");
    assert!(common::placed(w, Role::Button, "See on The Lens DB").is_empty(), "the menu no longer opens the page");
    rig.key(&slint::SharedString::from(slint::platform::Key::Escape));
    rig.settle();

    // Settings' Connected Apps disconnects: the token revoked, the copy dropped, the typed names still offered.
    ctl.act(oni_app::Act::Settings);
    w.global::<oni_app::AppSettings>().set_section(5);
    rig.settle();
    common::click(rig, w, common::the(w, Role::Button, "Disconnect").middle());
    rig.settle();
    ctl.act(oni_app::Act::CloseSettings);
    rig.settle();
    assert_eq!(kit.get_link(), KitLink::Out);
    assert!(!Kept::file_path(support).exists(), "the copy dropped");
    assert_eq!(names(gear.get_bodies()), ["Nikon FM2", "Rollei 35", "Pentax MX"], "the bodies a hand gave the roll stay among the names typed");
    rig.until("the token revoked", WAIT, || fake.journal().iter().any(|s| s.path == "/api/oauth/revoke"));

    // Every call named ONI and its system, bore its token but the exchange, and carried no Origin.
    let journal = fake.journal();
    assert!(journal.len() >= 8, "{journal:?}");
    for seen in &journal {
        assert!(seen.agent.as_deref().is_some_and(|a| a.starts_with("ONI/") && a.ends_with(')')), "{seen:?}");
        assert_eq!(seen.origin, None, "{seen:?}");
        assert_eq!(seen.bearer.is_some(), seen.path != "/api/oauth/token", "{seen:?}");
    }
    let _ = std::fs::remove_dir_all(root);
}
