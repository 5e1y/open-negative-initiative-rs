//! The gifts' takes, shot in the real app on a held clock: the photo flying into the editor and the export, the Kit
//! taking a body from The Lens DB (simulated), the year on film played through. Each take is the whole window at
//! twice its pixels, a JPEG a frame, with the points the hand goes to; `crates/oni-app/welcome-motion` composes the
//! films from them. `ONI_WELCOME_TAKES` names where they go, `ONI_WELCOME_RENDERS` the photos the gallery shows.

use crate::common::{self, studio::Studio};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell, TldbSite};
use oni_app::{Act, Flight, Kit, KitLink, Library, Store, Wrapped};
use oni_core::{Archive, EditorStore};
use slint::{ComponentHandle, Model};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The window filmed, in points, drawn at three pixels a point so a film twice as fine may still come close.
const WINDOW: (u32, u32) = (1280, 800);
const PIXELS: u32 = 3;
/// A take's frames a second, the films' own.
const FPS: u64 = 25;
const WAIT: Duration = Duration::from_secs(60);
/// The gallery's second tile, which the hand opens, in the window's points.
const TILE: (f32, f32) = (519.0, 223.0);

/// The takes being shot: each one's frames, written as they come, and the points named in the window.
struct Takes<'a> {
    rig: &'a Studio,
    out: PathBuf,
    takes: BTreeMap<String, usize>,
    points: BTreeMap<String, (f32, f32)>,
}

impl Takes<'_> {
    /// The window as drawn now, written as the next frame of `take`.
    fn frame(&mut self, take: &str) {
        let (w, h, rgba) = self.rig.grab();
        let bytes: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let count = self.takes.entry(take.to_string()).or_default();
        let dir = self.out.join(take);
        std::fs::create_dir_all(&dir).unwrap();
        // Fine, and its colour kept whole at every pixel: a tab's or a slider's edge is one pixel thin.
        let mut encoder = jpeg_encoder::Encoder::new_file(dir.join(format!("{:04}.jpg", *count)), 94).unwrap();
        encoder.set_sampling_factor(jpeg_encoder::SamplingFactor::R_4_4_4);
        encoder.encode(&bytes, w as u16, h as u16, jpeg_encoder::ColorType::Rgb).unwrap();
        *count += 1;
    }

    /// The window at rest, one frame.
    fn still(&mut self, take: &str) {
        self.rig.settle();
        self.frame(take);
    }

    /// A move begun on a held clock, `seconds` of it at the films' pace; the clock then runs again.
    fn journey(&mut self, take: &str, seconds: f32) {
        self.frame(take);
        for _ in 0..(seconds * FPS as f32).round() as u64 {
            self.rig.advance(Duration::from_millis(1000 / FPS));
            self.frame(take);
        }
        self.rig.thaw();
    }

    fn point(&mut self, name: &str, at: (f32, f32)) {
        self.points.insert(name.to_string(), at);
    }

    fn write(&self) {
        let list = |m: Vec<(String, String)>| m.into_iter().map(|(k, v)| format!("    \"{k}\": {v}")).collect::<Vec<_>>().join(",\n");
        let takes = list(self.takes.iter().map(|(k, n)| (k.clone(), n.to_string())).collect());
        let points = list(self.points.iter().map(|(k, (x, y))| (k.clone(), format!("[{x:.1}, {y:.1}]"))).collect());
        let json = format!("{{\n  \"window\": [{}, {}],\n  \"pixels\": {PIXELS},\n  \"fps\": {FPS},\n  \"takes\": {{\n{takes}\n  }},\n  \"points\": {{\n{points}\n  }}\n}}\n", WINDOW.0, WINDOW.1);
        std::fs::write(self.out.join("takes.json"), json).unwrap();
    }
}

/// The photos the gallery shows: the splash's own and the welcome's renders, which may be shown, each a digital
/// photo on the neutral film; the splash's again where no render is named.
fn photos(scans: &Path) {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/ui/splash");
    let renders = std::env::var_os("ONI_WELCOME_RENDERS").map(PathBuf::from);
    let render = |name: &str, splash: &str| renders.as_ref().map(|r| r.join(name)).filter(|p| p.exists()).unwrap_or_else(|| ui.join(splash));
    let files = [
        ui.join("1.0.0.jpg"),
        render("film-photo.jpg", "1.3.0.jpg"),
        ui.join("1.3.0.jpg"),
        render("film-print-2.jpg", "1.5.0.jpg"),
        ui.join("1.5.0.jpg"),
        render("film-print-3.jpg", "1.0.0.jpg"),
    ];
    std::fs::create_dir_all(scans).unwrap();
    for (i, file) in files.iter().enumerate() {
        let photo = scans.join(format!("{}.jpg", (b'A' + i as u8) as char));
        std::fs::copy(file, &photo).unwrap();
        let rest = oni_settings::PipelineSettings::rest(oni_settings::ConversionMode::Digital, false);
        oni_settings::sidecar::write(&rest, &photo, oni_settings::sidecar::WriteOptions::default()).unwrap();
    }
}

pub(crate) fn the_welcome_films_are_shot_in_the_real_app() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the welcome's films are not shot"),
    };
    let fake = oni_tldb::fake::start();
    let root = std::env::temp_dir().join(format!("oni-app-welcome-films-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    photos(&scans);
    std::fs::create_dir_all(&support).unwrap();
    crate::kit::signed_in(&fake)(&support, &caches);
    let rig = Studio::open(&gpu);
    oni_app::language::speak(Some(oni_i18n::Language::English));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let tldb = TldbSite::Simulated(fake.site.clone());
    let options = Options { support, caches, picker: Box::new(|| None), browse: Box::new(|_| {}), tldb, ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window().dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: PIXELS as f32 });
    rig.window().set_size(slint::PhysicalSize::new(WINDOW.0 * PIXELS, WINDOW.1 * PIXELS));
    shell.window.show().unwrap();
    shell.open_folder(&scans);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let lib = w.global::<Library>();
    rig.until("the thumbnails", WAIT, || {
        rig.pump();
        ctl.state.borrow().read_done() && lib.get_tiles().row_count() == 6 && lib.get_tiles().iter().all(|t| t.picture.size().width > 0)
    });
    let out = std::env::var_os("ONI_WELCOME_TAKES").map(PathBuf::from).unwrap_or_else(|| root.join("takes"));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let mut takes = Takes { rig: &rig, out: out.clone(), takes: BTreeMap::new(), points: BTreeMap::new() };

    // The interface: a photo of the gallery flies into the editor, the editor's photo onto the export's preview,
    // Cancel brings it back, Back returns to the gallery.
    let flight = w.global::<Flight>();
    let key = |name: &str| common::the(w, Role::Button, name).middle();
    takes.still("gallery");
    takes.point("tile", TILE);
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    rig.freeze();
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(1, false, false);
    assert!(flight.get_flying(), "the double-clicked tile lifts off the grid");
    takes.journey("open", 0.9);
    rig.until("the photo rendered", WAIT, || ctl.state.borrow().renders() >= 1);
    takes.still("editor");
    // The next photo by its arrow key, then back to the first: each slides into the editor in turn.
    for (key, take) in [(slint::platform::Key::RightArrow, "next"), (slint::platform::Key::LeftArrow, "previous")] {
        let before = ctl.state.borrow().renders();
        rig.freeze();
        rig.key(&slint::SharedString::from(key));
        takes.journey(take, 0.9);
        rig.until("the photo rendered", WAIT, || ctl.state.borrow().renders() > before);
        takes.still(&format!("{take}-rendered"));
    }
    takes.point("export", key("EXPORT"));
    rig.freeze();
    ctl.act(Act::Export);
    takes.journey("export", 0.9);
    rig.until("the export's preview", WAIT, || !flight.get_flying());
    takes.still("exporting");
    takes.point("cancel", key("Cancel"));
    rig.freeze();
    ctl.leave_export();
    takes.journey("cancel", 0.9);
    takes.still("edited");
    takes.point("back", key("BACK"));
    rig.freeze();
    ctl.act(Act::ToggleMode);
    takes.journey("back", 0.9);
    takes.still("gallery-again");

    // The Collection: Search Gear, a body searched for letter by letter, Add, Done, and the body in the Kit.
    let kit = w.global::<Kit>();
    let (x, y) = (4.0, 4000.0);
    lib.invoke_pointer_down(x, y, false, false);
    lib.invoke_pointer_up(x, y, false, false);
    rig.until("signed in", WAIT, || kit.get_link() == KitLink::In);
    // The Kit is read as the search first opens: once, before the takes, so they start on the person's own Kit.
    rig.settle();
    rig.click(w, key("Search Gear"));
    rig.until("the Kit read", WAIT, || kit.get_bodies().row_count() > 0);
    rig.click(w, key("Done"));
    rig.until("the search closed", WAIT, || !kit.get_picking());
    takes.still("kit");
    let search = key("Search Gear");
    takes.point("search", search);
    rig.freeze();
    rig.click(w, search);
    takes.journey("search", 0.9);
    let field = common::the(w, Role::TextInput, "Search The Lens DB");
    takes.point("field", field.middle());
    rig.click(w, field.middle());
    takes.still("typed");
    for letter in "nikon fm2".chars() {
        rig.key(&letter.to_string());
        rig.pump();
        takes.frame("typed");
    }
    rig.until("the search answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
    takes.still("found");
    takes.point("add", key("Add"));
    rig.freeze();
    rig.click(w, key("Add"));
    takes.journey("added", 0.6);
    let done = key("Done");
    takes.point("done", done);
    rig.freeze();
    rig.click(w, done);
    rig.until("Done sent", WAIT, || !kit.get_picking());
    takes.journey("closed", 0.9);
    takes.still("kit-after");
    assert!(kit.get_bodies().iter().any(|p| p.name == "Nikon FM2"), "the body added is in the Kit");

    // The year on film: an edit and a medal noted, then the Wrapped played through, each scene coming in.
    let g = w.global::<Store>();
    let exposure = oni_app::fields::Field::Exposure.index();
    lib.invoke_tile_clicked(3, false, false);
    lib.invoke_tile_clicked(3, false, false);
    rig.until("D on the canvas", WAIT, || ctl.state.borrow().renders() >= 2);
    g.invoke_press(exposure, false);
    for step in 1..=6 {
        g.invoke_drag(exposure, step as f32 * 10.0, 300.0, false);
        rig.pump();
    }
    g.invoke_release(exposure);
    ctl.act(Act::MedalGold);
    rig.settle();
    let wrapped = w.global::<Wrapped>();
    ctl.show_wrapped(oni_library::wrapped::today().0);
    rig.until("the year read", WAIT, || wrapped.get_shown() && wrapped.get_scenes().row_count() > 4);
    wrapped.set_playing(false);
    let scenes = wrapped.get_scenes().row_count();
    takes.still("wrapped-0");
    for scene in 1..scenes {
        rig.freeze();
        rig.key(&slint::SharedString::from(slint::platform::Key::Return));
        takes.journey(&format!("wrapped-{scene}"), 1.2);
    }
    takes.write();
    shell.window.hide().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
