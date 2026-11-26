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
const FRAME: (usize, usize) = (800, 500);
/// The window filmed, in points, drawn at twice its pixels so the frames come out smooth.
const WINDOW: (u32, u32) = (1280, 800);
/// A journey's frames, in ms from its start, past its spring's whole pace.
const JOURNEY: [u64; 10] = [0, 50, 100, 150, 200, 260, 330, 420, 540, 700];
const WAIT: Duration = Duration::from_secs(60);
/// The gallery's second tile, which the hand opens, in the window's points.
const TILE: (f32, f32) = (519.0, 223.0);

/// A part of the window, in its points: left, top, width; its height follows the frame's shape.
type Crop = [f32; 3];

/// The whole window, as a crop.
const WHOLE: Crop = [0.0, 0.0, WINDOW.0 as f32];

/// A crop `width` points wide about `at`, kept inside the window.
fn about((x, y): (f32, f32), width: f32) -> Crop {
    let height = width * FRAME.1 as f32 / FRAME.0 as f32;
    [(x - width / 2.0).clamp(0.0, WINDOW.0 as f32 - width), (y - height / 2.0).clamp(0.0, WINDOW.1 as f32 - height), width]
}

/// The system's arrow, as the welcome's `Arrow` draws it, in points from its tip.
const ARROW: [(f32, f32); 7] = [(0.0, 0.0), (0.0, 16.0), (4.0, 12.5), (6.8, 18.6), (9.0, 17.6), (6.3, 11.7), (11.5, 11.7)];

fn inside(p: (f32, f32), poly: &[(f32, f32)]) -> bool {
    let mut odd = false;
    for (i, a) in poly.iter().enumerate() {
        let b = poly[(i + 1) % poly.len()];
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) {
            odd = !odd;
        }
    }
    odd
}

/// A film being shot: where it looks, the hand on it, its frames and how long each holds, in seconds.
struct Film<'a> {
    rig: &'a Rig,
    crop: Crop,
    hand: Option<(f32, f32)>,
    frames: Vec<(Vec<u8>, f32)>,
}

impl<'a> Film<'a> {
    fn new(rig: &'a Rig) -> Film<'a> {
        Film { rig, crop: WHOLE, hand: None, frames: Vec::new() }
    }

    /// The window as drawn now, its width, 8 bits a channel.
    fn window(&self) -> (usize, Vec<Rgb8Pixel>) {
        self.rig.window.request_redraw();
        let w = self.rig.window.size().width as usize;
        let mut buffer = vec![Rgb8Pixel::default(); w * self.rig.window.size().height as usize];
        self.rig.window.draw_if_needed(|r| {
            r.render(&mut buffer, w);
        });
        (w, buffer)
    }

    /// The crop of `window` as a frame, the hand drawn over it, pressed or not.
    fn compose(&self, (w, buffer): &(usize, Vec<Rgb8Pixel>), pressed: bool) -> Vec<u8> {
        let scale = *w as f32 / WINDOW.0 as f32;
        let [left, top, width] = self.crop;
        let step = width * scale / FRAME.0 as f32;
        let (x0, y0) = (left * scale, top * scale);
        let mut out = vec![0u8; FRAME.0 * FRAME.1 * 3];
        for y in 0..FRAME.1 {
            for x in 0..FRAME.0 {
                let (sx, sy) = ((x0 + x as f32 * step) as usize, (y0 + y as f32 * step) as usize);
                let (ex, ey) = ((sx + step.ceil() as usize).min(*w), (sy + step.ceil() as usize).min(buffer.len() / w));
                let mut sum = [0u32; 3];
                for p in (sy..ey).flat_map(|row| buffer[row * w + sx..row * w + ex].iter()) {
                    sum = [sum[0] + u32::from(p.r), sum[1] + u32::from(p.g), sum[2] + u32::from(p.b)];
                }
                let n = ((ex - sx) * (ey - sy)).max(1) as u32;
                out[(y * FRAME.0 + x) * 3..][..3].copy_from_slice(&sum.map(|s| (s / n) as u8));
            }
        }
        if let Some((hx, hy)) = self.hand {
            // The arrow at the window's own size, a ring about its tip while it presses.
            let points = FRAME.0 as f32 / width;
            let size = if pressed { 0.9 } else { 1.0 };
            let (tx, ty) = ((hx - left) * points, (hy - top) * points);
            for y in 0..FRAME.1 {
                for x in 0..FRAME.0 {
                    let p = ((x as f32 - tx) / points / size, (y as f32 - ty) / points / size);
                    let grown: Vec<(f32, f32)> = ARROW.iter().map(|(ax, ay)| ((ax - 3.5) * 1.18 + 3.5, (ay - 6.0) * 1.18 + 6.0 - 0.6)).collect();
                    let ring = pressed && ((p.0 * p.0 + p.1 * p.1).sqrt() - 13.0).abs() < 1.2;
                    let colour = if inside(p, &ARROW) { Some(20) } else if inside(p, &grown) || ring { Some(245) } else { None };
                    if let Some(c) = colour {
                        out[(y * FRAME.0 + x) * 3..][..3].fill(c);
                    }
                }
            }
        }
        out
    }

    /// The window as it stands, held `seconds`.
    fn hold(&mut self, seconds: f32) {
        self.rig.settle();
        let frame = self.compose(&self.window(), false);
        self.frames.push((frame, seconds));
    }

    /// The hand moved to `to` over `seconds`, weighted, then pressing there.
    fn reach(&mut self, to: (f32, f32)) {
        self.rig.settle();
        let window = self.window();
        let from = self.hand.unwrap_or((to.0 + 90.0, to.1 + 70.0));
        for k in 1..=6 {
            let p = k as f32 / 6.0;
            let eased = 1.0 - (1.0 - p).powi(3);
            self.hand = Some((from.0 + (to.0 - from.0) * eased, from.1 + (to.1 - from.1) * eased));
            let frame = self.compose(&window, false);
            self.frames.push((frame, 0.09));
        }
        let frame = self.compose(&window, true);
        self.frames.push((frame, 0.16));
    }

    /// A journey begun on a held clock, a frame at each of its steps.
    fn journey(&mut self) {
        let mut at = 0;
        for step in JOURNEY {
            self.rig.advance(Duration::from_millis(step - at));
            at = step;
            let next = JOURNEY.iter().find(|s| **s > step).map_or(0.06, |s| (s - step) as f32 / 1000.0);
            let frame = self.compose(&self.window(), false);
            self.frames.push((frame, next));
        }
        self.rig.thaw();
    }

    /// The frames one under the other in one JPEG, and how long each holds, one a line.
    fn write(&self, out: &Path, name: &str) {
        let strip: Vec<u8> = self.frames.iter().flat_map(|(f, _)| f.iter().copied()).collect();
        let height = (FRAME.1 * self.frames.len()) as u16;
        let encoder = jpeg_encoder::Encoder::new_file(out.join(format!("{name}.jpg")), 74).unwrap();
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

    // The interface, close on what moves: the hand opens a photo of the gallery, which flies into the editor; it
    // presses Export, the photo moves onto the export's preview; Cancel, and back to the gallery.
    let flight = w.global::<Flight>();
    let key = |name: &str| common::the(w, Role::Button, name).middle();
    let mut film = Film::new(&rig);
    film.crop = [160.0, 60.0, 880.0];
    film.hold(0.9);
    film.reach(TILE);
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    rig.freeze();
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(1, false, false);
    assert!(flight.get_flying(), "the double-clicked tile lifts off the grid");
    film.hand = None;
    film.journey();
    rig.until("the photo rendered", WAIT, || ctl.state.borrow().renders() >= 1);
    film.hold(1.2);
    let export = key("EXPORT");
    film.crop = about(export, 560.0);
    film.hold(0.4);
    film.reach(export);
    rig.freeze();
    ctl.act(Act::Export);
    film.hand = None;
    film.crop = [40.0, 60.0, 960.0];
    film.journey();
    rig.until("the export's preview", WAIT, || !flight.get_flying());
    film.hold(1.4);
    let cancel = key("Cancel");
    film.crop = about(cancel, 560.0);
    film.reach(cancel);
    rig.freeze();
    ctl.leave_export();
    film.hand = None;
    film.crop = [160.0, 60.0, 880.0];
    film.journey();
    film.hold(0.6);
    let back = key("BACK");
    film.crop = about(back, 560.0);
    film.reach(back);
    rig.freeze();
    ctl.act(Act::ToggleMode);
    film.hand = None;
    film.crop = [160.0, 60.0, 880.0];
    film.journey();
    film.hold(1.0);
    assert!(film.frames.len() > 60, "every journey and gesture filmed");
    film.write(&out, "film-interface");

    // The Collection, close on the Kit: Search Gear, a body searched for, Add, Done, and the body in the Kit.
    let kit = w.global::<Kit>();
    let (x, y) = (4.0, 4000.0);
    lib.invoke_pointer_down(x, y, false, false);
    lib.invoke_pointer_up(x, y, false, false);
    rig.until("signed in", WAIT, || kit.get_link() == KitLink::In);
    // The Kit is read as the search first opens: once, before the film, so it starts on the person's own Kit.
    rig.settle();
    common::click(&rig, w, key("Search Gear"));
    rig.until("the Kit read", WAIT, || kit.get_bodies().row_count() > 0);
    common::click(&rig, w, key("Done"));
    rig.until("the search closed", WAIT, || !kit.get_picking());
    rig.settle();
    let mut film = Film::new(&rig);
    let search = key("Search Gear");
    film.crop = about((search.0 - 120.0, search.1 + 140.0), 640.0);
    film.hold(1.2);
    film.reach(search);
    rig.freeze();
    common::click(&rig, w, search);
    film.hand = None;
    film.journey();
    let field = common::the(w, Role::TextInput, "Search The Lens DB");
    let done = key("Done");
    film.crop = about((field.x + 300.0, field.y + 150.0), 640.0);
    film.reach(field.middle());
    common::click(&rig, w, field.middle());
    rig.key("nikon fm2");
    rig.until("the search answered", WAIT, || !kit.get_searching() && kit.get_found().row_count() > 0);
    film.hold(1.0);
    film.reach(key("Add"));
    common::click(&rig, w, key("Add"));
    film.hold(1.0);
    film.crop = about((done.0 - 200.0, done.1 + 150.0), 640.0);
    film.reach(done);
    rig.freeze();
    common::click(&rig, w, done);
    rig.until("Done sent and closed", WAIT, || !kit.get_picking());
    film.hand = None;
    film.journey();
    film.crop = about((search.0 - 120.0, search.1 + 140.0), 640.0);
    film.hold(2.0);
    assert!(kit.get_bodies().iter().any(|p| p.name == "Nikon FM2"), "the body added is in the Kit");
    film.write(&out, "film-collection");
    shell.window.hide().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
