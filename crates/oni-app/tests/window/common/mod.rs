//! The headless platform the window checks run on: Slint's software renderer, no screen, the
//! background threads' posts and the timers run by `pump`.

#![allow(dead_code)]

use slint::platform::software_renderer::{MinimalSoftwareWindow, Rgb565Pixel};
use slint::platform::{EventLoopProxy, Platform, WindowAdapter, WindowEvent};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub type Queued = Box<dyn FnOnce() + Send>;

pub struct Headless {
    pub window: Rc<MinimalSoftwareWindow>,
    pub queue: Arc<Mutex<Vec<Queued>>>,
    pub start: Instant,
}

struct Proxy(Arc<Mutex<Vec<Queued>>>);

impl EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        Ok(())
    }

    fn invoke_from_event_loop(&self, event: Queued) -> Result<(), slint::EventLoopError> {
        self.0.lock().unwrap().push(event);
        Ok(())
    }
}

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        self.start.elapsed()
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Proxy(self.queue.clone())))
    }
}

pub struct Rig {
    pub window: Rc<MinimalSoftwareWindow>,
    pub queue: Arc<Mutex<Vec<Queued>>>,
    pub shots: PathBuf,
}

impl Rig {
    /// Runs what background threads posted, then the timers and animations.
    pub fn pump(&self) {
        let jobs: Vec<Queued> = std::mem::take(&mut *self.queue.lock().unwrap());
        for job in jobs {
            job();
        }
        slint::platform::update_timers_and_animations();
    }

    pub fn until(&self, what: &str, limit: Duration, done: impl Fn() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < limit, "timed out waiting for {what}");
            self.pump();
            std::thread::sleep(Duration::from_millis(20));
        }
        self.pump();
        println!("{what}: {:.1} s", start.elapsed().as_secs_f64());
    }

    /// Lets springs and the tooltip delay finish, as a person waits before looking.
    pub fn settle(&self) {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(900) {
            self.pump();
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    /// The window drawn now, as its width and rows of RGB565 pixels.
    pub fn drawn(&self) -> (usize, Vec<Rgb565Pixel>) {
        self.window.request_redraw();
        let size = self.window.size();
        let (w, h) = (size.width as usize, size.height as usize);
        let mut buffer = vec![Rgb565Pixel(0); w * h];
        self.window.draw_if_needed(|r| {
            r.render(&mut buffer, w);
        });
        (w, buffer)
    }

    pub fn shot(&self, name: &str) -> PathBuf {
        let (w, buffer) = self.drawn();
        let h = buffer.len() / w;
        let path = self.shots.join(format!("{name}.png"));
        let file = std::fs::File::create(&path).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let bytes: Vec<u8> = buffer
            .iter()
            .flat_map(|p| {
                let v = p.0;
                let (r, g, b) = ((v >> 11) & 0x1f, (v >> 5) & 0x3f, v & 0x1f);
                [(r * 255 / 31) as u8, (g * 255 / 63) as u8, (b * 255 / 31) as u8]
            })
            .collect();
        writer.write_image_data(&bytes).unwrap();
        path
    }

    pub fn key(&self, text: &str) {
        let t: slint::SharedString = text.into();
        self.window.dispatch_event(WindowEvent::KeyPressed { text: t.clone() });
        self.window.dispatch_event(WindowEvent::KeyReleased { text: t });
        self.pump();
    }

    /// Several modifiers held around one key, pressed in order and released in reverse.
    pub fn chord_with(&self, modifiers: &[slint::platform::Key], text: &str) {
        let held: Vec<slint::SharedString> = modifiers.iter().map(|m| (*m).into()).collect();
        for m in &held {
            self.window.dispatch_event(WindowEvent::KeyPressed { text: m.clone() });
        }
        self.key(text);
        for m in held.iter().rev() {
            self.window.dispatch_event(WindowEvent::KeyReleased { text: m.clone() });
        }
        self.pump();
    }

    pub fn chord(&self, modifier: slint::platform::Key, text: &str) {
        let m: slint::SharedString = modifier.into();
        self.window.dispatch_event(WindowEvent::KeyPressed { text: m.clone() });
        self.key(text);
        self.window.dispatch_event(WindowEvent::KeyReleased { text: m });
        self.pump();
    }
}

/// The headless platform installed for this process, and the rig that drives it.
pub fn rig(shots: PathBuf) -> Rig {
    // `AppFolders`: a check writes its dust patches in a folder of its own, never the machine's caches.
    if std::env::var_os("ONI_CACHES").is_none() {
        // SAFETY: set before the shell or any worker thread of this process starts.
        unsafe { std::env::set_var("ONI_CACHES", std::env::temp_dir().join(format!("oni-window-caches-{}", std::process::id()))) };
    }
    let window = MinimalSoftwareWindow::new(slint::platform::software_renderer::RepaintBufferType::NewBuffer);
    let queue: Arc<Mutex<Vec<Queued>>> = Arc::default();
    slint::platform::set_platform(Box::new(Headless { window: window.clone(), queue: queue.clone(), start: Instant::now() })).unwrap();
    Rig { window, queue, shots }
}

/// A little-endian LinearRaw TIFF, 16-bit RGB in one strip, a smooth ramp scaled by `tone`: the
/// linear branch decodes it, so a check renders a frame without a camera file.
/// A 16-bit RGB TIFF naming no colour profile: a scanner's plain output, whose encoding the hand says.
pub fn plain_tiff(path: &std::path::Path, w: usize, h: usize, tone: f32) {
    tiff(path, w, h, tone, false);
}

pub fn linear_tiff(path: &std::path::Path, w: usize, h: usize, tone: f32) {
    tiff(path, w, h, tone, true);
}

/// A LinearRaw TIFF whose third channel ramps too, so every channel has two ends to measure.
pub fn graded_tiff(path: &std::path::Path, w: usize, h: usize, tone: f32) {
    write_tiff(path, w, h, tone, true, |x, y| [x, y, (x + y) / 2.0]);
}

/// A LinearRaw TIFF whose every pixel the caller gives, each channel from 0 (black) to 1 (white).
pub fn pixels_tiff(path: &std::path::Path, w: usize, h: usize, value: impl Fn(usize, usize) -> [f32; 3]) {
    write_tiff(path, w, h, 1.0, true, |x, y| value((x * w as f32).round() as usize, (y * h as f32).round() as usize).map(|v| (v - 0.1) / 0.8));
}

fn tiff(path: &std::path::Path, w: usize, h: usize, tone: f32, linear_raw: bool) {
    write_tiff(path, w, h, tone, linear_raw, |x, y| [x, y, 0.5]);
}

fn write_tiff(path: &std::path::Path, w: usize, h: usize, tone: f32, linear_raw: bool, channels: impl Fn(f32, f32) -> [f32; 3]) {
    let samples: Vec<u16> = (0..w * h)
        .flat_map(|i| {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            channels(x, y).map(|v| (100.0 + 900.0 * (0.1 + 0.8 * v * tone)) as u16)
        })
        .collect();
    let data: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    let ifd_at = 8 + data.len();
    let mut entries: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, w as u32),
        (257, 4, 1, h as u32),
        (258, 3, 1, 16),
        (259, 3, 1, 1),
        (262, 3, 1, if linear_raw { 34892 } else { 2 }),
        (273, 4, 1, 8),
        (277, 3, 1, 3),
        (278, 4, 1, h as u32),
        (279, 4, 1, data.len() as u32),
    ];
    if linear_raw {
        entries.extend([(50714, 4, 1, 100), (50717, 4, 1, 1000)]);
    }
    let mut out = b"II".to_vec();
    out.extend(42u16.to_le_bytes());
    out.extend((ifd_at as u32).to_le_bytes());
    out.extend(data);
    out.extend((entries.len() as u16).to_le_bytes());
    for (tag, t, n, v) in entries {
        out.extend(tag.to_le_bytes());
        out.extend(t.to_le_bytes());
        out.extend(n.to_le_bytes());
        if t == 3 {
            out.extend((v as u16).to_le_bytes());
            out.extend([0, 0]);
        } else {
            out.extend(v.to_le_bytes());
        }
    }
    out.extend(0u32.to_le_bytes());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, out).unwrap();
}
