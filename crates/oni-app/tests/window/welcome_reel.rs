//! The welcome's film on a held clock: the overture, a gift untied by a click, the huge gift's steps, a scene hurried.
//! With `ONI_WELCOME_REEL` naming a folder, every scene's sheet and its stills are written there.

use oni_app::app::{Options, Shell};
use oni_app::stage::direction::OVERTURE;
use oni_app::stage::{film, unwrap};
use oni_app::{Help, StageSpot};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, Rgb8Pixel};
use std::path::{Path, PathBuf};

use crate::common;

/// A sheet's cell, and how many stand on a row.
const CELL: (usize, usize) = (480, 300);
const ROW: usize = 4;

struct Reel<'a> {
    rig: &'a common::Rig,
    shell: &'a Shell,
    out: Option<PathBuf>,
    clock: f32,
    /// The pictures of the scene being filmed, reduced to a sheet's cells.
    cells: Vec<Vec<u8>>,
}

impl Reel<'_> {
    /// The window as drawn now, 8 bits a channel.
    fn drawn(&self) -> (usize, usize, Vec<u8>) {
        self.rig.window.request_redraw();
        let size = self.rig.window.size();
        let (w, h) = (size.width as usize, size.height as usize);
        let mut buffer = vec![Rgb8Pixel::default(); w * h];
        self.rig.window.draw_if_needed(|r| {
            r.render(&mut buffer, w);
        });
        (w, h, buffer.iter().flat_map(|p| [p.r, p.g, p.b]).collect())
    }

    /// The clock moved on by `seconds` and the picture drawn; kept for the scene's sheet when a reel is asked.
    fn step(&mut self, seconds: f32) {
        self.clock += seconds;
        self.shell.ctl.welcome_hold(Some(self.clock));
        self.rig.pump();
        if self.out.is_some() {
            let (w, h, pixels) = self.drawn();
            self.cells.push(reduced(&pixels, w, h));
        }
    }

    /// `seconds` of film, a picture every `every`.
    fn play(&mut self, seconds: f32, every: f32) {
        for _ in 0..(seconds / every).round() as usize {
            self.step(every);
        }
    }

    /// The scene filmed since the last cut, laid on a sheet under `name`; then the next scene starts.
    fn cut(&mut self, name: &str) {
        let cells = std::mem::take(&mut self.cells);
        let Some(out) = &self.out else { return };
        if cells.is_empty() {
            return;
        }
        // At most three rows a sheet: a long scene shows every nth picture, its last always.
        let nth = cells.len().div_ceil(ROW * 3);
        let mut kept: Vec<&Vec<u8>> = cells.iter().step_by(nth).collect();
        if (cells.len() - 1) % nth != 0 {
            kept.push(&cells[cells.len() - 1]);
        }
        let rows = kept.len().div_ceil(ROW);
        let (w, h) = (ROW * CELL.0, rows * CELL.1);
        let mut sheet = vec![0u8; w * h * 3];
        for (i, cell) in kept.iter().enumerate() {
            let (cx, cy) = (i % ROW * CELL.0, i / ROW * CELL.1);
            for y in 0..CELL.1 {
                sheet[((cy + y) * w + cx) * 3..][..CELL.0 * 3].copy_from_slice(&cell[y * CELL.0 * 3..][..CELL.0 * 3]);
            }
        }
        write(&out.join(format!("{name}.png")), &sheet, w, h);
        println!("sheet {name}: {} pictures, every {nth}", cells.len());
    }

    /// One picture at full size under `name`.
    fn still(&self, name: &str) {
        if let Some(out) = &self.out {
            let (w, h, pixels) = self.drawn();
            write(&out.join(format!("{name}.png")), &pixels, w, h);
        }
    }

    fn tap(&self, (x, y): (f32, f32)) {
        let position = LogicalPosition::new(x, y);
        self.rig.window.dispatch_event(WindowEvent::PointerMoved { position });
        self.rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
        self.rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
        self.rig.pump();
    }

    fn help(&self) -> Help<'_> {
        self.shell.window.global::<Help>()
    }

    /// The middle of gift `i` in the window, from the place the stage tells the sheet.
    fn gift(&self, i: usize) -> (f32, f32) {
        let spot: StageSpot = self.help().get_spots().row_data(i).expect("the gift's place");
        let origin = self.shell.window.window().size().to_logical(1.0).height - self.sheet_height();
        (spot.x + spot.width / 2.0, origin + spot.y + spot.height / 2.0)
    }

    /// The sheet's height: the window's, less a menu bar drawn in it.
    fn sheet_height(&self) -> f32 {
        use i_slint_backend_testing::ElementHandle;
        ElementHandle::find_by_element_type_name(&self.shell.window, "WelcomeSheet").next().map_or(0.0, |e| e.size().height)
    }
}

/// A picture reduced to a sheet's cell, each cell pixel the mean of those it covers.
fn reduced(pixels: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut cell = vec![0u8; CELL.0 * CELL.1 * 3];
    for y in 0..CELL.1 {
        for x in 0..CELL.0 {
            let (x0, x1, y0, y1) = (x * w / CELL.0, ((x + 1) * w / CELL.0).max(x * w / CELL.0 + 1), y * h / CELL.1, ((y + 1) * h / CELL.1).max(y * h / CELL.1 + 1));
            let mut sum = [0u32; 3];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += u32::from(pixels[(sy * w + sx) * 3 + c]);
                    }
                }
            }
            let count = ((x1 - x0) * (y1 - y0)) as u32;
            for (c, s) in sum.iter().enumerate() {
                cell[(y * CELL.0 + x) * 3 + c] = (s / count) as u8;
            }
        }
    }
    cell
}

fn write(path: &Path, rgb: &[u8], w: usize, h: usize) {
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path).unwrap()), w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(rgb).unwrap();
}

/// How much light the window holds about a point: the mean of a 40-pixel square's channels, 0 to 255.
fn light(reel: &Reel, (x, y): (f32, f32)) -> f32 {
    let (w, h, pixels) = reel.drawn();
    let (x, y) = ((x as usize).clamp(20, w - 21), (y as usize).clamp(20, h - 21));
    let mut sum = 0u32;
    for sy in y - 20..y + 20 {
        for sx in x - 20..x + 20 {
            sum += pixels[(sy * w + sx) * 3..][..3].iter().map(|v| u32::from(*v)).sum::<u32>();
        }
    }
    sum as f32 / (1600.0 * 3.0)
}

pub(crate) fn the_welcome_plays_its_acts_on_a_held_clock() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the welcome's stage is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-welcome-reel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let rig = common::rig();
    oni_app::language::speak(Some(oni_i18n::Language::English));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    let out = std::env::var_os("ONI_WELCOME_REEL").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    let size: Vec<u32> = std::env::var("ONI_WELCOME_REEL_SIZE").unwrap_or_else(|_| "1440x900".into()).split('x').map(|v| v.parse().unwrap()).collect();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(size[0], size[1]));
    shell.window.show().unwrap();
    shell.ctl.act(oni_app::Act::Welcome);
    // Drawn once, so the sheet stands in the window and tells the stage its size.
    rig.settle();
    let mut reel = Reel { rig: &rig, shell: &shell, out, clock: 0.0, cells: Vec::new() };
    // The stage's shader compiles off the interface's thread: the film starts once it has landed.
    reel.shell.ctl.welcome_hold(Some(0.0));
    rig.until("the stage", std::time::Duration::from_secs(60), || {
        shell.ctl.welcome_hold(Some(0.0));
        shell.window.global::<Help>().get_stage().size().width > 0
    });
    let help = shell.window.global::<Help>();

    // The overture: black, the release's name alone, then each gift lit where it stands, the hall's words at its end.
    assert_eq!(help.get_act(), 0, "the welcome opens on its overture");
    reel.step(0.0);
    let dark = light(&reel, (size[0] as f32 / 2.0, size[1] as f32 * 0.7));
    reel.play(2.0, 0.2);
    reel.still("1-overture-name");
    reel.play(2.8, 0.2);
    reel.still("1-overture-huge-gift");
    reel.play(OVERTURE - 4.4, 0.2);
    assert_eq!(help.get_act(), 1, "the overture ends on the hall");
    reel.cut("1-overture");
    reel.still("1-hall-end");
    let lit = light(&reel, reel.gift(2));
    assert!(dark < 3.0 && lit > 30.0, "black at first, the huge gift lit in the hall: {dark} then {lit}");

    // The hand on the huge gift: it answers; a click unties it, its page only once its light is out.
    let gold = reel.gift(2);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(gold.0, gold.1) });
    reel.play(1.2, 0.15);
    reel.cut("2-hover");
    reel.still("2-hover-end");
    // Twin: a click on the dark above the gifts opens nothing.
    reel.tap((gold.0, size[1] as f32 * 0.3));
    reel.step(0.1);
    assert_eq!((help.get_welcome_page(), help.get_act()), (0, 1), "twin: a click beside the gifts opens nothing");
    reel.tap(gold);
    reel.step(0.1);
    assert_eq!((help.get_welcome_page(), help.get_act(), help.get_act_gift()), (3, 2, 2), "the huge gift untied before its page");
    reel.play(unwrap::pop(2) - 0.3, 0.2);
    reel.still("3-unwrap-huge-seam");
    reel.play(unwrap::length(2) - unwrap::pop(2) - 1.0, 0.2);
    assert_eq!(help.get_act(), 2, "still untying a breath before its end");
    reel.still("3-unwrap-huge-title");
    reel.play(1.4, 0.2);
    assert_eq!((help.get_act(), help.get_welcome_unwrapped()), (3, 4), "its page once untied, the gift counted unwrapped");
    reel.cut("3-unwrap-huge");

    // Its steps follow one another by themselves; the last holds: the print, the photo laid back over it, each
    // film once, then the three side by side.
    for step in 0..film::STEPS - 1 {
        reel.step(0.05);
        assert_eq!(help.get_film_step(), step as i32, "step {step} plays in its turn");
        let length = film::length(step);
        reel.play((length * 0.5 / 0.2).floor() * 0.2, 0.2);
        reel.still(&format!("4-film-step-{}-middle", step + 1));
        reel.play(length - 0.4 - (length * 0.5 / 0.2).floor() * 0.2, 0.2);
        reel.still(&format!("4-film-step-{}-end", step + 1));
        reel.play(0.4, 0.2);
        reel.cut(&format!("4-film-step-{}", step + 1));
    }
    reel.step(0.05);
    assert_eq!(help.get_film_step(), 4, "the last step holds");
    // Each moment of the last step, in seconds since it began, reached in turn.
    let mut at = 0.05f32;
    let mut until = |reel: &mut Reel, moment: f32| {
        reel.play(((moment - at) / 0.2).round() * 0.2, 0.2);
        at += ((moment - at) / 0.2).round() * 0.2;
    };
    until(&mut reel, 0.85);
    reel.still("4-film-step-5-middle");
    until(&mut reel, film::PRINTED + 0.85);
    assert!(help.get_film_printed() && help.get_plate_spot().width > 100.0, "the print come forward, its place told for the hand");
    reel.still("4-film-step-5-end");
    until(&mut reel, film::SECOND + 0.85);
    reel.still("4-film-stock-2");
    until(&mut reel, film::COMPARE.0 + 1.65);
    assert!(help.get_film_compare() > 0.3, "the photo laid back over the second stock's print");
    reel.still("4-film-compare");
    reel.cut("4-film-step-5");
    until(&mut reel, film::THIRD + 1.25);
    assert!(help.get_film_compare() < 0.01, "then the print alone");
    reel.still("4-film-stock-3");
    until(&mut reel, film::FINALE + 4.05);
    assert!(help.get_film_ended() > 3.0 && help.get_plate_spot().width == 0.0, "the three prints side by side, no edge left to drag");
    reel.still("4-film-finale-end");
    reel.cut("4-film-finale");

    // Back to the hall, then a small gift played whole.
    help.set_welcome_page(0);
    reel.play(2.2, 0.2);
    assert_eq!(help.get_act(), 1);
    reel.cut("5-back-to-hall");
    reel.still("5-hall-one-opened");
    reel.tap(reel.gift(0));
    reel.play(unwrap::length(0) + 0.6, 0.2);
    reel.cut("6-unwrap-small");
    reel.play(6.0, 0.25);
    reel.cut("6-page-interface");
    reel.still("6-page-interface-end");

    // The mystery, played whole from a page: the camera goes from one gift to the other.
    help.set_welcome_page(4);
    reel.play(unwrap::length(3) + 0.6, 0.2);
    assert_eq!((help.get_act(), help.get_act_gift()), (3, 3));
    reel.cut("7-unwrap-mystery");
    reel.play(5.0, 0.25);
    reel.cut("7-page-mystery");
    reel.still("7-page-mystery-end");

    // The Collection's: Return hurries its untying to its page.
    help.set_welcome_page(2);
    reel.play(1.2, 0.2);
    assert_eq!(help.get_act(), 2);
    rig.key("\n");
    reel.step(0.1);
    assert_eq!((help.get_act(), help.get_act_gift()), (3, 1), "Return hurries the untying to its page");
    reel.play(6.0, 0.25);
    reel.cut("8-page-collection");
    reel.still("8-page-collection-end");

    // The message, its lines one after the other.
    help.set_welcome_page(5);
    reel.play(3.0, 0.2);
    reel.still("9-message-lights");
    reel.play(5.6, 0.2);
    assert_eq!(help.get_act(), 4);
    reel.cut("9-message");
    reel.still("9-message-end");
    shell.window.hide().unwrap();
}
