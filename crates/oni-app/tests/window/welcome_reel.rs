//! The welcome on a held clock: each gift opened before the next, on by a click or Return, never past one still
//! wrapped. With `ONI_WELCOME_REEL` naming a folder, its moments are laid on one contact sheet there.

use oni_app::app::{Options, Shell};
use oni_app::Help;
use oni_app::stage::unwrap;
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Rgb8Pixel};
use std::path::{Path, PathBuf};

use crate::common;

/// A sheet's cell, and how many stand on a row.
const CELL: (usize, usize) = (480, 300);
const ROW: usize = 3;

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

    /// The clock moved on by `seconds` and the picture drawn.
    fn step(&mut self, seconds: f32) {
        self.clock += seconds;
        self.shell.ctl.welcome_hold(Some(self.clock));
        self.rig.pump();
    }

    /// This moment kept for the contact sheet, when a reel is asked.
    fn keep(&mut self) {
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

    /// The moments kept, laid on one sheet under `name`.
    fn cut(&mut self, name: &str) {
        let kept = std::mem::take(&mut self.cells);
        let Some(out) = &self.out else { return };
        if kept.is_empty() {
            return;
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
        println!("sheet {name}: {} moments", kept.len());
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

    /// The middle of the screen's gift in the window, from the place the stage tells the sheet.
    fn gift(&self) -> (f32, f32) {
        let spot = self.help().get_gift_spot();
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

pub(crate) fn the_welcome_is_unwrapped_screen_by_screen() {
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
    let help = shell.window.global::<Help>();
    // Before the stage's shader lands, the gift stands as its still: the welcome never waits on the GPU.
    reel.step(0.0);
    assert!(help.get_welcome_shown() && help.get_welcome_page() == 1, "the welcome opens on its first gift");
    rig.until("the stage", std::time::Duration::from_secs(60), || {
        shell.ctl.welcome_hold(Some(0.0));
        help.get_stage_live() && help.get_stage().size().width > 0
    });
    for page in 1..=4 {
        let gift = page as usize - 1;
        reel.play(0.6, 0.2);
        assert_eq!(help.get_welcome_page(), page, "gift {gift}'s screen");
        assert!(help.get_opened_time() < 0.0, "gift {gift} still wrapped");
        reel.keep();
        // Twin: neither Return-less keys nor the wheel go on past a gift still wrapped.
        rig.key(&slint::SharedString::from(slint::platform::Key::DownArrow));
        reel.step(0.1);
        assert_eq!(help.get_welcome_page(), page, "gift {gift}: no way on while it is wrapped");
        // A click on the gift opens it; the first gift by Return.
        if page == 1 {
            rig.key("\n");
        } else {
            reel.tap(reel.gift());
        }
        reel.step(0.05);
        assert!(help.get_opened_time() >= 0.0, "gift {gift} opened");
        let words = unwrap::title(gift);
        reel.play((unwrap::pop(gift) / 0.2).round() * 0.2, 0.2);
        reel.keep();
        reel.play(((words + 1.2 - unwrap::pop(gift)) / 0.2).round() * 0.2, 0.2);
        assert!(help.get_may_go_on(), "gift {gift}: its words come, the way on opens");
        if gift == 3 {
            // The person's own year read for it, its cards laid on the screen, the Wrapped itself not shown.
            let wrapped = shell.window.global::<oni_app::Wrapped>();
            rig.until("the year read", std::time::Duration::from_secs(30), || slint::Model::row_count(&wrapped.get_scenes()) > 0);
            assert!(!wrapped.get_shown(), "twin: the cards are on the welcome, not the Wrapped over it");
        }
        reel.play(2.4, 0.2);
        reel.keep();
        assert!(light(&reel, reel.gift()) > 3.0, "gift {gift} drawn where it stands");
        rig.key(&slint::SharedString::from(slint::platform::Key::DownArrow));
        reel.play(0.8, 0.2);
        assert_eq!(help.get_welcome_page(), page + 1, "on past gift {gift} once opened");
    }
    // The message, and only its key leaves; the gifts kept opened for a launch after this one.
    reel.play(1.0, 0.2);
    reel.keep();
    reel.cut("welcome");
    assert_eq!(help.get_welcome_unwrapped(), 0b1111, "every gift opened");
    rig.key(&slint::SharedString::from(slint::platform::Key::Escape));
    assert!(help.get_welcome_shown(), "Escape does not leave it");
    rig.key("\n");
    assert!(!help.get_welcome_shown(), "its last key enters the app");
    shell.window.hide().unwrap();
}
