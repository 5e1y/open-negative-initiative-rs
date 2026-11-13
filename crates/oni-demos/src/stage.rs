//! The stage a scene plays on: a screenless window at two pixels a point drawn by the app's FemtoVG
//! renderer, a clock moving only when a frame is shot, the pointer's real events, as in the app.

use std::cell::Cell;
use std::path::Path;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use i_slint_core::window::WindowAdapterInternal;
use oni_lab::raster::Raster;
use slint::platform::femtovg_renderer::FemtoVGWGPURenderer;
use slint::platform::{EventLoopProxy, Platform, PointerEventButton, Renderer, WindowAdapter, WindowEvent};
use slint::{LogicalPosition, LogicalSize, PhysicalSize, PlatformError, Window, WindowSize};
use slint_interpreter::{ComponentHandle, ComponentInstance, Value};

use crate::photo::Rgba;

/// Pixels a point, as a Retina screen shows the card.
pub const SCALE: f32 = 2.0;

/// A window drawn offscreen: its size is what the scene asks, its pixels a snapshot of the renderer.
pub(crate) struct Offscreen {
    pub(crate) window: Window,
    renderer: FemtoVGWGPURenderer,
    size: Cell<PhysicalSize>,
}

impl WindowAdapter for Offscreen {
    fn window(&self) -> &Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn set_size(&self, size: WindowSize) {
        let scale = self.window.scale_factor();
        self.size.set(size.to_physical(scale));
        self.window.dispatch_event(WindowEvent::Resized { size: size.to_logical(scale) });
    }

    fn renderer(&self) -> &dyn Renderer {
        &self.renderer
    }

    fn internal(&self, _: i_slint_core::InternalToken) -> Option<&dyn WindowAdapterInternal> {
        Some(self)
    }
}

/// The menu bar is the system's, above every window, as on macOS: the window draws none of its own.
impl WindowAdapterInternal for Offscreen {
    fn supports_native_menu_bar(&self) -> bool {
        true
    }
}

/// What the app's threads post to the interface's, run when the stage pumps.
pub(crate) type Queue = Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>>;

struct Proxy(Queue);

impl EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        Ok(())
    }

    fn invoke_from_event_loop(&self, event: Box<dyn FnOnce() + Send>) -> Result<(), slint::EventLoopError> {
        self.0.lock().map_err(|_| slint::EventLoopError::EventLoopTerminated)?.push(event);
        Ok(())
    }
}

/// One offscreen window for the process, as Slint takes one platform; time is the clock's alone, so
/// a film's animations land on the same frame at every run.
struct Headless {
    window: Rc<Offscreen>,
    clock: Rc<Cell<Duration>>,
    queue: Queue,
}

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        self.clock.get()
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Proxy(self.queue.clone())))
    }
}

/// The process's offscreen window, its clock and the queue the app's threads post to.
#[derive(Clone)]
pub(crate) struct Platformed {
    pub(crate) window: Rc<Offscreen>,
    pub(crate) clock: Rc<Cell<Duration>>,
    pub(crate) queue: Queue,
}

impl Platformed {
    /// Runs what the app's threads posted, then the timers the clock reached; whether anything was posted.
    pub(crate) fn pump(&self) -> bool {
        let jobs = self.queue.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
        let posted = !jobs.is_empty();
        for job in jobs {
            job();
        }
        slint::platform::update_timers_and_animations();
        posted
    }
}

thread_local! {
    static PLATFORM: Result<Platformed, String> = {
        let gpu = oni_app::render::Device::open()?;
        let renderer = FemtoVGWGPURenderer::new(gpu.instance, gpu.device, gpu.queue).map_err(|e| e.to_string())?;
        let window = Rc::new_cyclic(|me: &Weak<Offscreen>| Offscreen {
            window: Window::new(me.clone() as Weak<dyn WindowAdapter>),
            renderer,
            size: Cell::new(PhysicalSize::new(1, 1)),
        });
        let clock = Rc::new(Cell::new(Duration::ZERO));
        let queue = Queue::default();
        let headless = Headless { window: window.clone(), clock: clock.clone(), queue: queue.clone() };
        slint::platform::set_platform(Box::new(headless)).map_err(|e| e.to_string())?;
        window.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: SCALE });
        Ok(Platformed { window, clock, queue })
    };
}

/// The process's stage platform, installed at the first call.
pub(crate) fn platform() -> Result<Platformed, String> {
    PLATFORM.with(|p| p.clone())
}

pub struct Stage {
    instance: ComponentInstance,
    window: Rc<Offscreen>,
    clock: Rc<Cell<Duration>>,
    /// Where the pointer was last sent and whether it was down.
    pointer: Option<((f32, f32), bool)>,
    /// The clock when the scene was made, every shot timed from it.
    origin: Duration,
}

impl Stage {
    /// The component `name` of the scene file `file`, laid at `size` points.
    pub fn new(file: &Path, name: &str, size: (f32, f32)) -> Result<Stage, String> {
        let Platformed { window, clock, .. } = platform()?;
        let result = crate::words::compile(file, oni_i18n::current());
        if result.has_errors() {
            return Err(format!("{} does not compile", file.display()));
        }
        let definition = oni_lab::pick(&result, Some(name)).ok_or_else(|| format!("no component {name} in {}", file.display()))?;
        let instance = definition.create().map_err(|e| e.to_string())?;
        oni_lab::serve(&instance, &Rc::new(Raster::default()));
        window.window.set_size(LogicalSize::new(size.0, size.1));
        instance.show().map_err(|e| e.to_string())?;
        // Each scene starts on a clock of its own, a second past the last one's, so no animation carries.
        let origin = clock.get() + Duration::from_secs(1);
        clock.set(origin);
        slint::platform::update_timers_and_animations();
        Ok(Stage { instance, window, clock, pointer: None, origin })
    }

    /// The scene itself, for its globals, its keyboard and its elements where the layout put them.
    pub fn instance(&self) -> &ComponentInstance {
        &self.instance
    }

    pub fn set(&self, name: &str, value: Value) {
        if let Err(e) = self.instance.set_property(name, value) {
            panic!("the scene's {name}: {e}");
        }
    }

    pub fn set_image(&self, name: &str, picture: &Rgba) {
        let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&picture.pixels, picture.width, picture.height);
        self.set(name, Value::Image(slint::Image::from_rgba8(buffer)));
    }

    pub fn get(&self, name: &str) -> Value {
        self.instance.get_property(name).unwrap_or_else(|e| panic!("the scene's {name}: {e}"))
    }

    /// A number or a length, in points.
    pub fn number(&self, name: &str) -> f32 {
        match self.get(name) {
            Value::Number(n) => n as f32,
            other => panic!("the scene's {name} is no number: {other:?}"),
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        matches!(self.get(name), Value::Bool(true))
    }

    /// The pointer at `at`, in points, and down or up: what moved is sent as the window's events.
    pub fn pointer(&mut self, at: (f32, f32), down: bool) {
        let position = LogicalPosition::new(at.0, at.1);
        let (was, was_down) = self.pointer.unwrap_or(((f32::NAN, f32::NAN), false));
        if was != at {
            self.window.window.dispatch_event(WindowEvent::PointerMoved { position });
        }
        if down != was_down {
            let button = PointerEventButton::Left;
            self.window.window.dispatch_event(if down {
                WindowEvent::PointerPressed { position, button }
            } else {
                WindowEvent::PointerReleased { position, button }
            });
        }
        self.pointer = Some((at, down));
    }

    /// A second passes unfilmed, so what the scene was first given has landed before the first frame.
    pub fn settle(&mut self) {
        self.origin += Duration::from_secs(1);
        self.clock.set(self.origin);
        slint::platform::update_timers_and_animations();
    }

    /// The scene at `t` into the clip, its animations and timers run up to it.
    pub fn shoot(&self, t: Duration) -> Result<Rgba, String> {
        self.clock.set(self.origin + t);
        slint::platform::update_timers_and_animations();
        let buffer = self.instance.window().take_snapshot().map_err(|e| e.to_string())?;
        Ok(Rgba { width: buffer.width(), height: buffer.height(), pixels: buffer.as_bytes().to_vec() })
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = self.instance.hide();
    }
}
