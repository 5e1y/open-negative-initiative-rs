//! The small gifts filmed in the real app on a held clock: the photo flying into the editor and the export, then the
//! Kit taking a body from The Lens DB (simulated). `ONI_WELCOME_FILMS` names where `scripts/welcome-assets` writes them.

use crate::common::{self, Rig};
use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell, TldbSite};
use oni_app::{Act, Flight, Kit, KitLink, Library};
use oni_core::{Archive, EditorStore};
use slint::{ComponentHandle, Model, Rgb8Pixel};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A film's frame, in pixels: the welcome's demo frame on a large screen.
const FRAME: (usize, usize) = (896, 560);
/// The window filmed, in points, drawn at twice its pixels so the frames come out smooth.
const WINDOW: (u32, u32) = (1280, 800);
/// A journey's frames, in ms from its start, past its spring's whole pace.
const JOURNEY: [u64; 10] = [0, 50, 100, 150, 200, 260, 330, 420, 540, 700];
const WAIT: Duration = Duration::from_secs(60);

/// A film being shot: its frames and how long each holds, in seconds.
struct Film<'a> {
    rig: &'a Rig,
    frames: Vec<(Vec<u8>, f32)>,
}

impl Film<'_> {
    /// The window as drawn now, reduced to a frame, 8 bits a channel.
    fn frame(&self) -> Vec<u8> {
        self.rig.window.request_redraw();
        let size = self.rig.window.size();
        let (w, h) = (size.width as usize, size.height as usize);
        let mut buffer = vec![Rgb8Pixel::default(); w * h];
        self.rig.window.draw_if_needed(|r| {
            r.render(&mut buffer, w);
        });
        let mut out = vec![0u8; FRAME.0 * FRAME.1 * 3];
        for y in 0..FRAME.1 {
            for x in 0..FRAME.0 {
                let (x0, x1, y0, y1) = (x * w / FRAME.0, (x + 1) * w / FRAME.0, y * h / FRAME.1, (y + 1) * h / FRAME.1);
                let mut sum = [0u32; 3];
                for p in (y0..y1).flat_map(|sy| buffer[sy * w + x0..sy * w + x1].iter()) {
                    sum = [sum[0] + u32::from(p.r), sum[1] + u32::from(p.g), sum[2] + u32::from(p.b)];
                }
                let n = ((x1 - x0) * (y1 - y0)).max(1) as u32;
                out[(y * FRAME.0 + x) * 3..][..3].copy_from_slice(&sum.map(|s| (s / n) as u8));
            }
        }
        out
    }

    /// The window as it stands, held `seconds`.
    fn hold(&mut self, seconds: f32) {
        self.rig.settle();
        let frame = self.frame();
        self.frames.push((frame, seconds));
    }

    /// A journey begun on a held clock, a frame at each of its steps.
    fn journey(&mut self) {
        let mut at = 0;
        for step in JOURNEY {
            self.rig.advance(Duration::from_millis(step - at));
            at = step;
            let next = JOURNEY.iter().find(|s| **s > step).map_or(0.06, |s| (s - step) as f32 / 1000.0);
            let frame = self.frame();
            self.frames.push((frame, next));
        }
        self.rig.thaw();
    }

    /// The frames one under the other in one JPEG, and how long each holds, one a line.
    fn write(&self, out: &Path, name: &str) {
        let strip: Vec<u8> = self.frames.iter().flat_map(|(f, _)| f.iter().copied()).collect();
        let height = (FRAME.1 * self.frames.len()) as u16;
        let encoder = jpeg_encoder::Encoder::new_file(out.join(format!("{name}.jpg")), 78).unwrap();
        encoder.encode(&strip, FRAME.0 as u16, height, jpeg_encoder::ColorType::Rgb).unwrap();
        let holds: Vec<String> = self.frames.iter().map(|(_, s)| format!("{s:.3}")).collect();
        std::fs::write(out.join(format!("{name}.txt")), format!("{} {}\n{}\n", FRAME.0, FRAME.1, holds.join("\n"))).unwrap();
    }
}

/// The photos the films show: the welcome's own, which may be shown, each a digital photo on the neutral film.
fn photos(scans: &Path) {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/ui");
    let files = ["splash/1.0.0.jpg", "welcome/film-photo.jpg", "splash/1.3.0.jpg", "welcome/film-print-2.jpg", "splash/1.5.0.jpg", "welcome/film-print-3.jpg"];
    std::fs::create_dir_all(scans).unwrap();
    for (i, file) in files.iter().enumerate() {
        let photo = scans.join(format!("{}.jpg", (b'A' + i as u8) as char));
        std::fs::copy(ui.join(file), &photo).unwrap();
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
    let rig = common::rig();
    oni_app::language::speak(Some(oni_i18n::Language::English));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let tldb = TldbSite::Simulated(fake.site.clone());
    let options = Options { support, caches, picker: Box::new(|| None), browse: Box::new(|_| {}), tldb, ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(WINDOW.0 * 2, WINDOW.1 * 2));
    shell.window.show().unwrap();
    shell.open_folder(&scans);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let lib = w.global::<Library>();
    rig.until("the thumbnails", WAIT, || {
        rig.pump();
        ctl.state.borrow().read_done() && lib.get_tiles().row_count() == 6 && lib.get_tiles().iter().all(|t| t.picture.size().width > 0)
    });
    let out = std::env::var_os("ONI_WELCOME_FILMS").map(PathBuf::from).unwrap_or_else(|| root.join("films"));
    std::fs::create_dir_all(&out).unwrap();

    // The interface: the photo flies into the editor, the editor turns into the export, and back to the gallery.
    let flight = w.global::<Flight>();
    let mut film = Film { rig: &rig, frames: Vec::new() };
    film.hold(1.4);
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    rig.freeze();
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(1, false, false);
    assert!(flight.get_flying(), "the double-clicked tile lifts off the grid");
    film.journey();
    rig.until("the photo rendered", WAIT, || ctl.state.borrow().renders() >= 1);
    film.hold(1.6);
    rig.freeze();
    ctl.act(Act::Export);
    film.journey();
    rig.until("the export's preview", WAIT, || !flight.get_flying());
    film.hold(1.6);
    rig.freeze();
    ctl.leave_export();
    film.journey();
    film.hold(0.8);
    rig.freeze();
    ctl.act(Act::ToggleMode);
    film.journey();
    film.hold(1.2);
    assert!(film.frames.len() > 40, "every journey filmed");
    film.write(&out, "film-interface");

    // The Collection: the Kit from The Lens DB beside the gallery, a body searched for, added, and in the Kit.
    // Nothing picked: the column shows the Kit, read from the service as it first shows.
    let kit = w.global::<Kit>();
    let (x, y) = (4.0, 4000.0);
    lib.invoke_pointer_down(x, y, false, false);
    lib.invoke_pointer_up(x, y, false, false);
    rig.until("the Kit read", WAIT, || kit.get_link() == KitLink::In);
    let mut film = Film { rig: &rig, frames: Vec::new() };
    film.hold(1.6);
    rig.freeze();
    common::click(&rig, w, common::the(w, Role::Button, "Search Gear").middle());
    film.journey();
    common::click(&rig, w, common::the(w, Role::TextInput, "Search The Lens DB").middle());
    rig.key("nikon fm2");
    rig.until("the search answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
    film.hold(1.4);
    common::click(&rig, w, common::the(w, Role::Button, "Add").middle());
    film.hold(1.2);
    rig.freeze();
    common::click(&rig, w, common::the(w, Role::Button, "Done").middle());
    rig.until("Done sent and closed", WAIT, || !kit.get_picking());
    film.journey();
    film.hold(2.0);
    assert!(kit.get_bodies().iter().any(|p| p.name == "Nikon FM2"), "the body added is in the Kit");
    film.write(&out, "film-collection");
    shell.window.hide().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
