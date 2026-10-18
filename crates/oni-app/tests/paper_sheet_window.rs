//! The paper profiles sheet on the headless window: the makers in two groups, a page read and
//! filtered, a profile installed, chosen and uninstalled, all through hands standing in for the web.

use oni_app::app::export_card::{self, paper_sheet};
use oni_app::app::{Options, Shell};
use oni_app::display::Screen;
use oni_app::{Exporting, MainWindow};
use oni_colour::PaperSource;
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model, PhysicalSize};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod common;
#[path = "../../oni-colour/tests/common/mod.rs"]
mod stand_in;

const LONG: Duration = Duration::from_secs(60);

const PICTO: &str = r#"<a href="/files/Baryta_Gloss.icc">Baryta</a> <a href="/files/notice.pdf">pdf</a>
    <a href="https://www.pictoonline.com/files/Matte_Fine.icc">Matte</a> <img src="https://evil.example/x.icc">"#;

fn rows(e: &Exporting) -> Vec<(String, bool, bool)> {
    let m = e.get_paper_rows();
    (0..m.row_count()).filter_map(|i| m.row_data(i)).map(|r| (r.title.to_string(), r.installed, r.installing)).collect()
}

#[test]
fn a_maker_s_profile_is_listed_installed_chosen_and_uninstalled() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the paper sheet is not exercised"),
    };
    let shots = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots");
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let root = std::env::temp_dir().join(format!("oni-app-paper-sheet-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let support = root.join("support");
    let profiles = root.join("Profiles");
    let served = root.join("served");
    std::fs::create_dir_all(&served).unwrap();
    stand_in::write_stand_in(&served.join("Baryta_Gloss.icc"), "Baryta", 0.02, Some(stand_in::bluish()), false);

    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { screen: Screen::DisplayP3, canvas: None, support: support.clone(), caches: root.join("caches"), picker: Box::new(|| None) };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(1440, 900));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w: &MainWindow = &shell.window;
    let e = w.global::<Exporting>();
    export_card::profiles_in(&ctl, profiles.clone());

    // The web as the check serves it: Picto's page, its Baryta file, no other.
    let fetched: Arc<Mutex<Vec<String>>> = Arc::default();
    let opened: Rc<RefCell<Vec<String>>> = Rc::default();
    let discarded: Rc<RefCell<Vec<PathBuf>>> = Rc::default();
    {
        let (fetched, opened, discarded, served) = (fetched.clone(), opened.clone(), discarded.clone(), served.clone());
        paper_sheet::web_with(&ctl, paper_sheet::Web {
            fetch: Arc::new(move |page: &str| {
                fetched.lock().unwrap().push(page.to_owned());
                Ok(if page.contains("pictoonline") { PICTO.to_owned() } else { String::new() })
            }),
            download: Arc::new(move |url: &str, folder: &Path| {
                let name = url.rsplit('/').next().unwrap().to_owned();
                let file = served.join(&name);
                if !file.exists() {
                    return Err("offline".into());
                }
                std::fs::create_dir_all(folder).unwrap();
                std::fs::copy(&file, folder.join(&name)).unwrap();
                Ok(folder.join(name))
            }),
            open: Rc::new(move |page: &str| opened.borrow_mut().push(page.to_owned())),
            discard: Rc::new(move |file: &Path| {
                discarded.borrow_mut().push(file.to_path_buf());
                std::fs::remove_file(file)
            }),
        });
    }
    let all = PaperSource::all();
    let settled = |what: &str| rig.until(what, LONG, || !e.get_paper_reading() && !e.get_paper_busy());

    // Get Paper Profiles… opens on the first maker; the two groups split as the sources reach.
    e.invoke_get_papers();
    assert!(e.get_papers_shown());
    assert_eq!(e.get_paper_direct().row_count(), 4);
    assert_eq!(e.get_paper_browsed().row_count(), all.len() - 4);
    assert_eq!(e.get_paper_name(), "Awagami");
    settled("Awagami's page");
    assert_eq!(fetched.lock().unwrap().last().map(String::as_str), Some(all[0].page));
    assert!(!e.get_paper_any(), "an empty page lists nothing");

    // Picto's page: its two profiles, the pdf and the other host left out.
    e.invoke_paper_picked(1);
    settled("Picto's page");
    assert_eq!(rows(&e), [("Baryta Gloss".into(), false, false), ("Matte Fine".into(), false, false)]);
    e.invoke_paper_filter_typed("matte fine".into());
    assert_eq!(rows(&e).len(), 1);
    // Twin: a word no title holds lists nothing.
    e.invoke_paper_filter_typed("lustre".into());
    assert!(rows(&e).is_empty());
    e.invoke_paper_filter_typed("".into());

    // Install: downloaded, adopted into the profiles, chosen as the space, recorded under its link.
    e.invoke_paper_install(0);
    settled("the install");
    assert_eq!(e.get_paper_message(), "Installed 1 profile — the export now uses “Baryta”.");
    assert!(profiles.join("Baryta_Gloss.icc").exists());
    assert!(e.get_chip_title().contains("Baryta") && e.get_on_paper(), "{}", e.get_chip_title());
    assert_eq!(rows(&e)[0], ("Baryta Gloss".into(), true, false));
    let prefs: serde_json::Value = serde_json::from_slice(&std::fs::read(support.join("preferences.json")).unwrap()).unwrap();
    assert!(prefs["paperInstalls"]["/files/Baryta_Gloss.icc"].is_array(), "{}", prefs["paperInstalls"]);
    assert!(!support.join("Scratch").exists() || std::fs::read_dir(support.join("Scratch")).unwrap().all(|d| !d.unwrap().file_name().to_string_lossy().starts_with("paper-download")));
    // Twin: a download that fails installs nothing and says so.
    e.invoke_paper_install(1);
    settled("the failed install");
    assert_eq!(e.get_paper_message(), "The download failed: offline");
    assert!(!rows(&e)[1].1);

    e.invoke_paper_open_page();
    assert_eq!(opened.borrow().as_slice(), [all[1].page]);

    // Uninstall: the file discarded, the link forgotten, the space back off the profile gone.
    e.invoke_paper_uninstall(0);
    assert_eq!(e.get_paper_message(), "Moved Baryta Gloss to the Trash.");
    assert_eq!(discarded.borrow().len(), 1);
    assert!(!profiles.join("Baryta_Gloss.icc").exists());
    assert!(!rows(&e)[0].1);
    assert_eq!(e.get_chip_title(), oni_export::Settings::default().space().title());
    assert!(!e.get_on_paper());
    // Twin: uninstalling again finds nothing left.
    e.invoke_paper_uninstall(0);
    assert_eq!(e.get_paper_message(), "Nothing was left to remove.");

    // A maker behind a form: its page opens in the browser and is never read here.
    let reads = fetched.lock().unwrap().len();
    e.invoke_paper_picked(4);
    settled("Hahnemühle");
    assert!(e.get_paper_browser());
    assert_eq!(e.get_paper_name(), all[4].name);
    assert_eq!(fetched.lock().unwrap().len(), reads, "a browser source is not fetched");
    rig.settle();
    println!("shot {}", rig.shot("paper-sheet").display());

    e.invoke_papers_done();
    assert!(!e.get_papers_shown() && !paper_sheet::sheet(&ctl).shown);
    let _ = std::fs::remove_dir_all(&root);
}
