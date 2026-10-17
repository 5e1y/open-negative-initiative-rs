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

    pub fn shot(&self, name: &str) -> PathBuf {
        self.window.request_redraw();
        let size = self.window.size();
        let (w, h) = (size.width as usize, size.height as usize);
        let mut buffer = vec![Rgb565Pixel(0); w * h];
        self.window.draw_if_needed(|r| {
            r.render(&mut buffer, w);
        });
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
    let window = MinimalSoftwareWindow::new(slint::platform::software_renderer::RepaintBufferType::NewBuffer);
    let queue: Arc<Mutex<Vec<Queued>>> = Arc::default();
    slint::platform::set_platform(Box::new(Headless { window: window.clone(), queue: queue.clone(), start: Instant::now() })).unwrap();
    Rig { window, queue, shots }
}
