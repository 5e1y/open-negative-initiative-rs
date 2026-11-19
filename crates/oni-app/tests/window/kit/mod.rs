//! What the Kit's window checks share: a shell over two scans and a simulated The Lens DB, the browser's
//! return to ONI's port, and the service's journal read by address.

use crate::common::{self, Rig};
use oni_app::app::{Options, Shell, TldbSite};
use oni_core::{Archive, EditorStore};
use oni_settings::Film;
use oni_tldb::fake::{Fake, Seen};
use slint::{ComponentHandle, Model, SharedString};
use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

pub const WAIT: Duration = Duration::from_secs(20);
/// The gallery's empty room, clear of the export's toast at its foot.
pub const EMPTY: (f32, f32) = (800.0, 400.0);

pub struct Stage {
    pub rig: Rig,
    pub shell: Shell,
    pub root: PathBuf,
    pub scans: PathBuf,
    pub support: PathBuf,
    /// Every page the app opened in the browser.
    pub opened: Rc<RefCell<Vec<String>>>,
}

/// Two scans of one roll read, the window shown; `before` lays files in the support and caches folders
/// first, as a token or an index. None without a GPU, said as a check skipped.
pub fn stage(name: &str, tldb: TldbSite, before: impl FnOnce(&std::path::Path, &std::path::Path)) -> Option<Stage> {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("{e}: the Kit is not exercised");
            return None;
        }
    };
    let root = std::env::temp_dir().join(format!("oni-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for file in ["A.tif", "B.tif"] {
        common::linear_tiff(&scans.join(file), 48, 32, 0.5);
    }
    oni_settings::film::roll_folder::set_roll(Some(&Film::new("Scans")), &scans).unwrap();
    std::fs::create_dir_all(&support).unwrap();
    before(&support, &caches);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let opened: Rc<RefCell<Vec<String>>> = Rc::default();
    let browser = opened.clone();
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let browse = Box::new(move |url: &str| browser.borrow_mut().push(url.to_string()));
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), browse, tldb, ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    shell.open_folder(&scans);
    let ctl = shell.ctl.clone();
    rig.until("the photos read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().order().len() == 2);
    rig.settle();
    Some(Stage { rig, shell, root, scans, support, opened })
}

/// The token a finished sign-in to the simulated service leaves in the support folder.
pub fn signed_in(fake: &Fake) -> impl FnOnce(&std::path::Path, &std::path::Path) {
    let token = fake.token();
    move |support, _| std::fs::write(support.join("tldb-fake-token"), token).unwrap()
}

/// What the browser does with the service's redirect: a GET on ONI's loopback port.
pub fn follow(url: &str) {
    let rest = url.strip_prefix("http://").expect("a loopback address");
    let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let mut stream = std::net::TcpStream::connect(host).expect("ONI's port listens");
    write!(stream, "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").unwrap();
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    assert!(answer.starts_with("HTTP/1.1 200"), "the page ONI answers: {answer}");
}

/// The calls the service received at an address of the partner API, `/kit/changes` say.
pub fn calls(fake: &Fake, path: &str) -> Vec<Seen> {
    fake.journal().into_iter().filter(|s| s.path == format!("/api/partner/v1{path}")).collect()
}

pub fn piece_names(model: slint::ModelRc<oni_app::KitPiece>) -> Vec<String> {
    model.iter().map(|p| p.name.to_string()).collect()
}

pub fn names(model: slint::ModelRc<SharedString>) -> Vec<String> {
    model.iter().map(|s| s.to_string()).collect()
}
