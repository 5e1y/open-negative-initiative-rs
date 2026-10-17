//! The window rendered without a screen by Slint's software renderer, on roll #8's DSC_6440: one frame
//! written to `frame.png`, then the slider's gestures driven through real pointer events.

use std::rc::Rc;
use std::time::{Duration, Instant};

use oni_ui_poc::frame::{self, Frame, display_rgba8};
use oni_ui_poc::{EXPOSURE_RANGE, Poc, track};
use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType};
use slint::platform::{Key, Platform, PointerEventButton, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};

struct Headless(Rc<MinimalSoftwareWindow>);

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

const SCALE: f32 = 2.0;
const SIZE: (u32, u32) = (2200, 1360);

struct Screen {
    window: Rc<MinimalSoftwareWindow>,
}

impl Screen {
    fn draw(&self) -> Vec<[u8; 3]> {
        slint::platform::update_timers_and_animations();
        let (w, h) = (SIZE.0 as usize, SIZE.1 as usize);
        let mut buffer = vec![PremultipliedRgbaColor::default(); w * h];
        self.window.request_redraw();
        self.window.draw_if_needed(|renderer| {
            renderer.render(&mut buffer, w);
        });
        buffer.iter().map(|p| [p.red, p.green, p.blue]).collect()
    }

    fn event(&self, e: WindowEvent) {
        self.window.dispatch_event(e);
    }

    fn press(&self, x: f32, y: f32) {
        let position = LogicalPosition::new(x, y);
        self.event(WindowEvent::PointerMoved { position });
        self.event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    }

    fn move_to(&self, x: f32, y: f32) {
        self.event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
    }

    fn release(&self, x: f32, y: f32) {
        self.event(WindowEvent::PointerReleased { position: LogicalPosition::new(x, y), button: PointerEventButton::Left });
    }

    /// Runs the timers until `done` holds or two seconds pass, as an event loop would.
    fn settle(&self, done: impl Fn() -> bool) -> bool {
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            slint::platform::update_timers_and_animations();
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        false
    }
}

/// The mean of a block of device pixels, per channel.
fn mean(pixels: &[[u8; 3]], stride: usize, x0: usize, y0: usize, n: usize) -> [f64; 3] {
    let mut acc = [0.0; 3];
    for y in y0..y0 + n {
        for x in x0..x0 + n {
            for c in 0..3 {
                acc[c] += pixels[y * stride + x][c] as f64;
            }
        }
    }
    acc.map(|v| v / (n * n) as f64)
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f64::max)
}

fn write_png(path: &std::path::Path, pixels: &[[u8; 3]]) {
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), SIZE.0, SIZE.1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&pixels.concat()).unwrap();
}

#[test]
fn the_window_renders_the_roll_and_the_slider_drives_the_kernel() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
    let screen = Screen { window: window.clone() };

    let Some(roll) = oni_fixtures::raw_dir("the_window_renders_the_roll_and_the_slider_drives_the_kernel") else { return };
    let path = roll.join(frame::FRAME);
    let started = Instant::now();
    let frame = Frame::open(&path).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
    println!("decoded and reduced {} to {} x {} in {:.2} s", frame::FRAME, frame.canvas.width, frame.canvas.height, started.elapsed().as_secs_f64());
    let set = frame.set;
    let poc = Poc::new(frame, SCALE as f64).unwrap();
    screen.event(WindowEvent::ScaleFactorChanged { scale_factor: SCALE });
    window.set_size(PhysicalSize::new(SIZE.0, SIZE.1));
    poc.window.show().unwrap();
    let w = &poc.window;
    let stride = SIZE.0 as usize;

    // 1. The frame as set on the roll: the canvas shows the kernel's output, not the negative.
    let started = Instant::now();
    let first = screen.draw();
    println!("software frame in {:.0} ms", started.elapsed().as_secs_f64() * 1e3);
    write_png(&std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("frame.png"), &first);
    assert_eq!(w.get_exposure_readout(), "+1.50 EV");
    let (cx, cy, cw, ch) = (w.get_canvas_x(), w.get_canvas_y(), w.get_canvas_width(), w.get_canvas_height());
    assert_eq!(poc.state.borrow().renders(), 1, "one render at opening");
    let raw = {
        let f = Frame::open(&path).unwrap();
        let started = Instant::now();
        let converted = display_rgba8(&f.converted(&set.stage47));
        println!("stages 4 to 7 on the CPU and display encoding in {:.0} ms", started.elapsed().as_secs_f64() * 1e3);
        (display_rgba8(&f.canvas), f.canvas.width, f.canvas.height, converted)
    };
    let converted = &raw.3;
    let (iw, ih) = (raw.1 as f32, raw.2 as f32);
    let (ax, ay, aw, ah) = (cx + 40.0, cy + 40.0, cw - 80.0, ch - 80.0);
    let fit = (aw / iw).min(ah / ih);
    let (ox, oy) = (ax + (aw - iw * fit) / 2.0, ay + (ah - ih * fit) / 2.0);
    let source_mean = |bytes: &[u8], u0: f32, v0: f32, side: f32| -> [f64; 3] {
        let mut acc = [0.0; 3];
        let mut n = 0.0;
        let (u1, v1) = ((u0 + side) as usize, (v0 + side) as usize);
        for v in v0 as usize..v1 {
            for u in u0 as usize..u1 {
                let i = (v * raw.1 + u) * 4;
                for c in 0..3 {
                    acc[c] += bytes[i + c] as f64;
                }
                n += 1.0;
            }
        }
        acc.map(|a| a / n)
    };
    let mut worst = 0.0f64;
    let mut worst_raw = f64::MAX;
    for (fx, fy) in [(0.5, 0.5), (0.3, 0.4), (0.7, 0.6), (0.25, 0.75)] {
        let (lx, ly) = (ox + iw * fit * fx, oy + ih * fit * fy);
        let n = 24usize;
        let (dx, dy) = ((lx * SCALE) as usize, (ly * SCALE) as usize);
        let shown = mean(&first, stride, dx, dy, n);
        let side = n as f32 / (fit * SCALE);
        let (u0, v0) = ((dx as f32 / SCALE - ox) / fit, (dy as f32 / SCALE - oy) / fit);
        let expected = source_mean(converted, u0, v0, side);
        let negative = source_mean(&raw.0, u0, v0, side);
        worst = worst.max(distance(shown, expected));
        worst_raw = worst_raw.min(distance(shown, negative));
        println!("block at ({fx}, {fy}): shown {shown:.1?}, converted {expected:.1?}, negative {negative:.1?}");
    }
    assert!(worst < 6.0, "the canvas departs from the kernel's output by {worst:.1} levels");
    assert!(worst_raw > 20.0, "twin: the canvas would match the unconverted negative ({worst_raw:.1})");

    // The knob is the keycap raster: a white grip line on its face, the rail beside it is not.
    let (kx, ky) = (w.get_knob_centre_x(), w.get_knob_centre_y());
    let span = w.get_track_width();
    let expected_x = {
        let f = track::position(1.5, &EXPOSURE_RANGE, None);
        kx - span * f
    };
    println!("knob at ({kx}, {ky}), track {span} pt from x = {expected_x}");
    let px = |x: f32, y: f32| first[(y * SCALE) as usize * stride + (x * SCALE) as usize];
    let grip = px(kx + 2.5, ky);
    let rail = px(kx - 40.0, ky);
    println!("grip {grip:?}, rail {rail:?}, face {:?}", px(kx, ky));
    assert!(grip.iter().all(|c| *c > 200), "grip line {grip:?}");
    // Mid-dish the face is the albedo itself, #313131, as `midtoneMatch` places it.
    let face = px(kx, ky);
    assert!(face.iter().all(|c| (*c as i32 - 0x31).abs() <= 2), "face {face:?}");
    assert!(rail.iter().all(|c| *c < 200), "twin: the rail away from the knob is not white: {rail:?}");

    // 2. A drag of a quarter of the travel moves Exposure by 2 EV and brightens the canvas.
    let before = mean(&first, stride, ((ox + iw * fit * 0.5) * SCALE) as usize, ((oy + ih * fit * 0.5) * SCALE) as usize, 48);
    screen.press(kx, ky);
    screen.move_to(kx + span * 0.25, ky);
    screen.release(kx + span * 0.25, ky);
    assert!((poc.state.borrow().value() - 3.5).abs() < 1e-4, "dragged to {}", poc.state.borrow().value());
    assert_eq!(w.get_exposure_readout(), "+3.50 EV");
    let dragged = screen.draw();
    let after = mean(&dragged, stride, ((ox + iw * fit * 0.5) * SCALE) as usize, ((oy + ih * fit * 0.5) * SCALE) as usize, 48);
    println!("centre mean before {before:.1?}, after +2 EV {after:.1?}");
    assert!(after.iter().zip(before).all(|(a, b)| *a > b + 5.0), "brighter after +2 EV");
    write_png(&std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("frame-dragged.png"), &dragged);

    // Twin: a press and release with no travel writes nothing.
    let renders = poc.state.borrow().renders();
    let (kx, ky) = (w.get_knob_centre_x(), w.get_knob_centre_y());
    std::thread::sleep(Duration::from_millis(600));
    screen.press(kx, ky);
    screen.release(kx, ky);
    assert_eq!((poc.state.borrow().value(), poc.state.borrow().renders()), (3.5, renders));

    // 3. Option slows the drag fivefold: the same travel moves 0.4 EV.
    std::thread::sleep(Duration::from_millis(600));
    screen.event(WindowEvent::KeyPressed { text: Key::Alt.into() });
    screen.press(kx, ky);
    screen.move_to(kx - span * 0.25, ky);
    screen.release(kx - span * 0.25, ky);
    screen.event(WindowEvent::KeyReleased { text: Key::Alt.into() });
    assert!((poc.state.borrow().value() - 3.1).abs() < 1e-4, "fine drag landed at {}", poc.state.borrow().value());

    // 4. The readout follows every move and the release hands the last value; the 30 Hz rate itself is
    // judged in `track`, since each synchronous render here outlasts the interval.
    std::thread::sleep(Duration::from_millis(600));
    let (kx, ky) = (w.get_knob_centre_x(), w.get_knob_centre_y());
    let renders = poc.state.borrow().renders();
    screen.press(kx, ky);
    let burst = Instant::now();
    for k in 1..=10 {
        screen.move_to(kx - k as f32, ky);
    }
    let elapsed = burst.elapsed();
    let during = poc.state.borrow().renders() - renders;
    let readout = w.get_exposure_readout();
    let live = track::value(track::position(3.1, &EXPOSURE_RANGE, None) - 10.0 / span, &EXPOSURE_RANGE, None);
    println!("ten moves in {:.1} ms: {during} renders, readout {readout}", elapsed.as_secs_f64() * 1e3);
    assert_eq!(readout, oni_ui_poc::exposure_readout(live), "the readout follows the hand at once");
    screen.release(kx - 10.0, ky);
    assert!((poc.state.borrow().value() - live).abs() < 1e-4, "the release hands the last value");
    assert!(poc.state.borrow().renders() > renders, "twin: the release does render");

    // 5. A double click puts Exposure back to rest at once and springs the knob there.
    std::thread::sleep(Duration::from_millis(600));
    let (kx, ky) = (w.get_knob_centre_x(), w.get_knob_centre_y());
    screen.press(kx, ky);
    screen.release(kx, ky);
    screen.press(kx, ky);
    assert_eq!(poc.state.borrow().value(), 0.0);
    assert_eq!(w.get_exposure_readout(), "+0.00 EV");
    let midway = w.get_exposure_fraction();
    assert!(midway > 0.5 + 1e-3, "the knob has not jumped: {midway}");
    screen.move_to(kx + 50.0, ky);
    screen.release(kx + 50.0, ky);
    assert_eq!(poc.state.borrow().value(), 0.0, "the rest of a double click's press drags nothing");
    // AppKit's click count keeps counting: a third quick press resets again and drags nothing.
    screen.press(kx, ky);
    screen.move_to(kx + 50.0, ky);
    screen.release(kx + 50.0, ky);
    assert_eq!(poc.state.borrow().value(), 0.0, "a third click is a reset, not a drag");
    assert!(screen.settle(|| (w.get_exposure_fraction() - 0.5).abs() < 1e-6), "the knob springs to rest");

    // Twin: two presses farther apart than the interval are two single clicks.
    std::thread::sleep(Duration::from_millis(600));
    let (kx, ky) = (w.get_knob_centre_x(), w.get_knob_centre_y());
    screen.press(kx, ky);
    screen.move_to(kx + span * 0.125, ky);
    screen.release(kx + span * 0.125, ky);
    std::thread::sleep(Duration::from_millis(600));
    screen.press(kx, ky);
    screen.release(kx, ky);
    assert!((poc.state.borrow().value() - 1.0).abs() < 1e-4, "no reset: {}", poc.state.borrow().value());

    // 6. The accessibility increment steps by a hundredth of the range.
    w.invoke_step(1);
    assert!((poc.state.borrow().value() - 1.08).abs() < 1e-4);

    // 7. A press with no travel while a value springs in lets the spring land: the knob is never left
    // short of its value.
    std::thread::sleep(Duration::from_millis(600));
    let (kx, ky) = (w.get_knob_centre_x(), w.get_knob_centre_y());
    screen.press(kx, ky);
    screen.release(kx, ky);
    screen.press(kx, ky);
    screen.release(kx, ky);
    assert_eq!(poc.state.borrow().value(), 0.0);
    std::thread::sleep(Duration::from_millis(600));
    screen.press(kx, ky);
    screen.release(kx, ky);
    assert!(screen.settle(|| (w.get_exposure_fraction() - 0.5).abs() < 1e-6), "the knob stopped short at {}", w.get_exposure_fraction());

    // 8. A scroll along the track moves the value a quarter of the glide; one across it is the panel's.
    let (kx, ky) = (w.get_knob_centre_x(), w.get_knob_centre_y());
    let renders = poc.state.borrow().renders();
    let scroll = |dx: f32, dy: f32| screen.event(WindowEvent::PointerScrolled { position: LogicalPosition::new(kx, ky), delta_x: dx, delta_y: dy });
    scroll(0.0, 40.0);
    scroll(10.0, 40.0);
    assert_eq!((poc.state.borrow().value(), poc.state.borrow().renders()), (0.0, renders), "twin: a scroll across the track moves nothing");
    scroll(40.0, 0.0);
    let expected = track::value(0.5 + 40.0 * oni_ui_poc::TRACKPAD_TRAVEL / span, &EXPOSURE_RANGE, None);
    println!("scrolled to {} for {expected}", poc.state.borrow().value());
    assert!((poc.state.borrow().value() - expected).abs() < 1e-4, "scrolled to {}", poc.state.borrow().value());
    assert_eq!(poc.state.borrow().renders(), renders + 1);
}
