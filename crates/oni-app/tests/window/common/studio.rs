//! A headless window drawn by the app's own renderer, FemtoVG on wgpu, into a texture read back: the welcome's films
//! show the interface as the app draws it, where the checks' software renderer draws some edges and layers otherwise.

use super::{Proxy, Queued};
use slint::platform::femtovg_renderer::FemtoVGWGPURenderer;
use slint::platform::{EventLoopProxy, Platform, Renderer, WindowAdapter, WindowEvent};
use slint::{PhysicalSize, Window, WindowSize};
use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The window: its size in pixels, drawn on demand.
pub struct Lens {
    window: Window,
    renderer: FemtoVGWGPURenderer,
    size: Cell<PhysicalSize>,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl WindowAdapter for Lens {
    fn window(&self) -> &Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn set_size(&self, size: WindowSize) {
        let scale = self.window.scale_factor();
        let physical = size.to_physical(scale);
        self.size.set(physical);
        self.window.dispatch_event(WindowEvent::Resized { size: physical.to_logical(scale) });
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn request_redraw(&self) {}
}

/// The platform that hands out the one window and keeps Slint's clock, held or ahead, as the checks' does.
struct Stage {
    lens: Rc<Lens>,
    queue: Arc<Mutex<Vec<Queued>>>,
    start: Instant,
    ahead: Rc<Cell<Duration>>,
    frozen: Rc<Cell<Option<Duration>>>,
}

impl Platform for Stage {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.lens.clone())
    }

    fn duration_since_start(&self) -> Duration {
        self.frozen.get().unwrap_or_else(|| self.start.elapsed() + self.ahead.get())
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Proxy(self.queue.clone())))
    }
}

/// The studio the films are shot in: the window, the background threads' posts, the clock.
pub struct Studio {
    pub lens: Rc<Lens>,
    queue: Arc<Mutex<Vec<Queued>>>,
    start: Instant,
    ahead: Rc<Cell<Duration>>,
    frozen: Rc<Cell<Option<Duration>>>,
}

impl Studio {
    /// The studio installed as this process's platform, drawing on `gpu`.
    pub fn open(gpu: &oni_app::render::Device) -> Studio {
        // SAFETY: set before the shell or any worker thread of this process starts: the films speak the interface's
        // own language, which the live interface reads in English, and keep their caches to themselves.
        unsafe { std::env::set_var("ONI_LANGUAGE", "en") };
        if std::env::var_os("ONI_CACHES").is_none() {
            // SAFETY: as above.
            unsafe { std::env::set_var("ONI_CACHES", std::env::temp_dir().join(format!("oni-studio-caches-{}", std::process::id()))) };
        }
        let renderer = FemtoVGWGPURenderer::new(gpu.instance.clone(), gpu.device.clone(), gpu.queue.clone()).expect("FemtoVG on wgpu");
        let lens = Rc::new_cyclic(|me: &Weak<Lens>| Lens {
            window: Window::new(me.clone() as Weak<dyn WindowAdapter>),
            renderer,
            size: Cell::new(PhysicalSize::new(0, 0)),
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
        });
        let (queue, ahead, frozen): (Arc<Mutex<Vec<Queued>>>, Rc<Cell<Duration>>, Rc<Cell<Option<Duration>>>) = Default::default();
        let start = Instant::now();
        let stage = Stage { lens: lens.clone(), queue: queue.clone(), start, ahead: ahead.clone(), frozen: frozen.clone() };
        slint::platform::set_platform(Box::new(stage)).unwrap();
        Studio { lens, queue, start, ahead, frozen }
    }

    pub fn window(&self) -> &Window {
        &self.lens.window
    }

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
    }

    /// Lets springs and delays finish, the window drawn first as a real one is.
    pub fn settle(&self) {
        self.grab();
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(900) {
            self.pump();
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    /// Slint's clock moved `by` while the real one stands, then the timers it reached run.
    pub fn advance(&self, by: Duration) {
        match self.frozen.get() {
            Some(at) => self.frozen.set(Some(at + by)),
            None => self.ahead.set(self.ahead.get() + by),
        }
        slint::platform::update_timers_and_animations();
    }

    /// Slint's clock held where it stands, `advance` alone moving it, until `thaw`.
    pub fn freeze(&self) {
        if self.frozen.get().is_none() {
            self.frozen.set(Some(self.start.elapsed() + self.ahead.get()));
        }
        slint::platform::update_timers_and_animations();
    }

    pub fn thaw(&self) {
        if let Some(at) = self.frozen.take() {
            self.ahead.set(at.saturating_sub(self.start.elapsed()).max(self.ahead.get()));
        }
        slint::platform::update_timers_and_animations();
    }

    pub fn key(&self, text: &str) {
        let t: slint::SharedString = text.into();
        self.window().dispatch_event(WindowEvent::KeyPressed { text: t.clone() });
        self.window().dispatch_event(WindowEvent::KeyReleased { text: t });
        self.pump();
    }

    /// A press and release at `(x, y)`, in the window's points.
    pub fn click(&self, window: &oni_app::MainWindow, (x, y): (f32, f32)) {
        let position = slint::LogicalPosition::new(x, y);
        self.window().dispatch_event(WindowEvent::PointerMoved { position });
        oni_app::outside::pressed(window);
        self.window().dispatch_event(WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left });
        self.window().dispatch_event(WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left });
        self.pump();
    }

    /// The window as the app's renderer draws it now: its width, height and rows of RGBA bytes.
    pub fn grab(&self) -> (u32, u32, Vec<u8>) {
        let PhysicalSize { width, height } = self.lens.size.get();
        let texture = self.lens.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("studio"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.lens.renderer.render_to_texture(&texture).expect("the window drawn");
        let bytes = oni_app::display::read_back(&self.lens.device, &self.lens.queue, &texture, width, height).expect("the window read back");
        (width, height, bytes)
    }
}
