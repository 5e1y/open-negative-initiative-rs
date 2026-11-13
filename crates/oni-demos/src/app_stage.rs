//! The app on the stage: the real window and its shell on the stage's offscreen FemtoVG-wgpu window,
//! fed a world of its own, driven by real pointer and key events, shot at the film's clock.

use std::rc::Rc;
use std::time::{Duration, Instant};

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementRoot};
use oni_app::app::{Ctl, Options, Shell};
use oni_app::render::{Device, Engine};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, LogicalSize};

use crate::film::FPS;
use crate::photo::Rgba;
use crate::stage::{self, Platformed, SCALE};

/// The app's smallest window, where bar, canvas and panel lie closest together.
pub const MINIMUM: (f32, f32) = (1100.0, 680.0);
/// Pumps in a row with nothing posted and nothing busy before the app counts as idle.
const QUIET: u32 = 3;

/// Where the layout put an element, in window points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Placed {
    pub fn middle(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// The smallest rectangle holding both.
    pub fn union(&self, other: &Placed) -> Placed {
        let (x, y) = (self.x.min(other.x), self.y.min(other.y));
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Placed { x, y, width: right - x, height: bottom - y }
    }

    /// Grown by `by` on every side, then widened or heightened about its middle to `aspect` (width
    /// over height) and slid inside `within`, which it never outgrows.
    pub fn fitted(&self, by: f32, aspect: f32, within: Placed) -> Placed {
        let (mut width, mut height) = (self.width + 2.0 * by, self.height + 2.0 * by);
        if width / height < aspect {
            width = height * aspect;
        } else {
            height = width / aspect;
        }
        let scale = (within.width / width).min(within.height / height).min(1.0);
        let (width, height) = (width * scale, height * scale);
        let (mx, my) = self.middle();
        let x = (mx - width / 2.0).clamp(within.x, within.x + within.width - width);
        let y = (my - height / 2.0).clamp(within.y, within.y + within.height - height);
        Placed { x, y, width, height }
    }
}

/// The real window run by the real shell, on the stage's platform.
pub struct AppStage {
    pub shell: Shell,
    platform: Platformed,
    size: (f32, f32),
    /// Where the pointer was last sent and whether it was down.
    pointer: Option<((f32, f32), bool)>,
    /// The clock when the stage was made, and how far the film has run since.
    origin: Duration,
    at: Duration,
}

impl AppStage {
    /// The shell on `options` laid in a window of `size` points, reopening the folder last opened as a
    /// launch does.
    pub fn new(options: Options, size: (f32, f32)) -> Result<AppStage, String> {
        let platform = stage::platform()?;
        let gpu = Device::open()?;
        let engine = Engine::shared(&gpu);
        {
            let mut e = engine.lock()?;
            e.support = Some(options.support.clone());
            e.caches = Some(oni_app::patches::caches());
        }
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let shell = Shell::new(store, engine, options).map_err(|e| e.to_string())?;
        platform.window.window.set_size(LogicalSize::new(size.0, size.1));
        shell.window.show().map_err(|e| e.to_string())?;
        platform.window.window.dispatch_event(WindowEvent::WindowActiveChanged(true));
        // A film's world changes only by its gestures: no watch reads the folder again behind them.
        shell.ctl.set_watching(false);
        if !shell.open_remembered() {
            return Err("no folder kept as the last opened".into());
        }
        let origin = platform.clock.get() + Duration::from_secs(1);
        platform.clock.set(origin);
        let stage = AppStage { shell, platform, size, pointer: None, origin, at: Duration::ZERO };
        stage.platform.pump();
        Ok(stage)
    }

    pub fn ctl(&self) -> &Rc<Ctl> {
        &self.shell.ctl
    }

    pub fn window(&self) -> &oni_app::MainWindow {
        &self.shell.window
    }

    /// The whole window, in points.
    pub fn bounds(&self) -> Placed {
        Placed { x: 0.0, y: 0.0, width: self.size.0, height: self.size.1 }
    }

    /// Every element shown with this role and accessible label, where the layout put it.
    pub fn placed(&self, role: AccessibleRole, label: &str) -> Vec<Placed> {
        ElementHandle::find_by_accessible_label(&self.shell.window, label)
            .filter(|e| e.accessible_role() == Some(role) && e.size().width > 0.0)
            .map(|e| {
                let (at, size) = (e.absolute_position(), e.size());
                Placed { x: at.x, y: at.y, width: size.width, height: size.height }
            })
            .collect()
    }

    /// Every element shown of the component `kind` or inheriting it; the search reads the names the
    /// live interface carries, or a compiled one built with `SLINT_EMIT_DEBUG_INFO`.
    pub fn kind(&self, kind: &str) -> Vec<Placed> {
        let found = self.shell.window.root_element().query_descendants().match_descendants().match_inherits(kind).find_all();
        let mut placed: Vec<Placed> = found
            .into_iter()
            .filter(|e| e.size().width > 0.0)
            .map(|e| {
                let (at, size) = (e.absolute_position(), e.size());
                Placed { x: at.x, y: at.y, width: size.width, height: size.height }
            })
            .collect();
        // The search meets an element once per repeater it sits in.
        placed.dedup();
        placed
    }

    /// The one element so named, or why there is not one.
    pub fn the(&self, role: AccessibleRole, label: &str) -> Result<Placed, String> {
        match self.placed(role, label).as_slice() {
            [one] => Ok(*one),
            found => Err(format!("one {role:?} named {label:?} shown, found {}", found.len())),
        }
    }

    /// The pointer at `at`, in points, down or up; a press and a release reach the shell's hooks first,
    /// as the app's window hands them over.
    pub fn pointer(&mut self, at: (f32, f32), down: bool) {
        let position = LogicalPosition::new(at.0, at.1);
        let window = self.shell.window.window();
        let (was, was_down) = self.pointer.unwrap_or(((f32::NAN, f32::NAN), false));
        if was != at {
            window.dispatch_event(WindowEvent::PointerMoved { position });
        }
        if down && !was_down {
            oni_app::outside::pressed(&self.shell.window);
            oni_app::editors::pressed_anywhere(&self.shell.ctl, (f64::from(at.0), f64::from(at.1)));
            window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
        } else if !down && was_down {
            self.shell.ctl.lifted();
            window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
        }
        self.pointer = Some((at, down));
        self.platform.pump();
    }

    /// One key pressed and released; `text` is a character or one of Slint's `Key`s.
    pub fn key(&mut self, text: &str) {
        let w = &self.shell.window;
        oni_app::outside::keyed(w.window(), || w.invoke_refocus());
        let text: slint::SharedString = text.into();
        w.window().dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
        w.window().dispatch_event(WindowEvent::KeyReleased { text });
        self.platform.pump();
    }

    /// What keeps the app from rest: work off the interface's thread it will post back.
    pub fn busy(&self) -> Option<&'static str> {
        let s = self.shell.ctl.state.borrow();
        let rendering = s.open_path().is_some() && s.rendered() < s.store().revision();
        [
            (s.disk_busy(), "a gesture's writes"),
            (s.healing(), "dust patches"),
            (s.drawing(), "a thumbnail redraw"),
            (s.load_bar().is_some(), "what the load bar shows"),
            (rendering, "a render"),
        ]
        .into_iter()
        .find_map(|(on, what)| on.then_some(what))
    }

    /// Before the film: the app at the real pace, the film's clock following the real one, until
    /// `done`; past `most`, an error naming what was awaited.
    pub fn until(&mut self, what: &str, most: Duration, done: impl Fn(&AppStage) -> bool) -> Result<(), String> {
        let started = Instant::now();
        let mut last = started;
        while !done(self) {
            if started.elapsed() > most {
                return Err(format!("{what} not reached within {most:?}"));
            }
            std::thread::sleep(Duration::from_millis(5));
            self.at += last.elapsed();
            last = Instant::now();
            self.platform.clock.set(self.origin + self.at);
            self.platform.pump();
        }
        Ok(())
    }

    /// The app's posts and timers run, the film's clock standing, until it rests; past `most` of
    /// real time, an error naming what still runs.
    pub fn idle(&self, most: Duration) -> Result<(), String> {
        let started = Instant::now();
        let mut quiet = 0;
        while quiet < QUIET {
            let posted = self.platform.pump();
            quiet = if posted || self.busy().is_some() { 0 } else { quiet + 1 };
            if started.elapsed() > most {
                return Err(format!("the app still runs {} after {most:?}", self.busy().unwrap_or("posts")));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    /// The film's clock moved `by`, a frame's step at a time, the app left to rest at each: a timer
    /// set by work that landed meanwhile fires at its own time, as on a screen.
    pub fn advance(&mut self, by: Duration, most: Duration) -> Result<(), String> {
        let step = Duration::from_millis(1000 / u64::from(FPS));
        let end = self.at + by;
        while self.at < end {
            self.at = (self.at + step).min(end);
            self.platform.clock.set(self.origin + self.at);
            self.platform.pump();
            self.idle(most)?;
        }
        Ok(())
    }

    /// The next frame of the film: a twentieth of a second on, the app at rest, then shot.
    pub fn frame(&mut self, most: Duration) -> Result<Rgba, String> {
        self.advance(Duration::from_millis(1000 / u64::from(FPS)), most)?;
        self.shoot()
    }

    /// Frames at the real pace, the film's clock following the real one, from the first where `done`
    /// holds and `least` has passed; past `most`, an error rather than a film that lies.
    pub fn gate(&mut self, what: &str, least: Duration, most: Duration, done: impl Fn(&AppStage) -> bool) -> Result<Vec<Rgba>, String> {
        let (started, step) = (Instant::now(), Duration::from_millis(1000 / u64::from(FPS)));
        let mut frames = Vec::new();
        while started.elapsed() < least || !done(self) {
            if started.elapsed() > most {
                return Err(format!("{what} not shown within {most:?}"));
            }
            let due = started + step * (frames.len() as u32 + 1);
            while Instant::now() < due {
                self.platform.pump();
                std::thread::sleep(Duration::from_millis(2));
            }
            self.at += step;
            self.platform.clock.set(self.origin + self.at);
            self.platform.pump();
            frames.push(self.shoot()?);
        }
        Ok(frames)
    }

    /// The whole window as drawn now, at the stage's pixels a point.
    pub fn shoot(&self) -> Result<Rgba, String> {
        let buffer = self.shell.window.window().take_snapshot().map_err(|e| e.to_string())?;
        Ok(Rgba { width: buffer.width(), height: buffer.height(), pixels: buffer.as_bytes().to_vec() })
    }
}

impl Drop for AppStage {
    fn drop(&mut self) {
        let _ = self.shell.window.hide();
    }
}

/// The part of a shot inside `region`, in points, at the stage's pixels a point.
pub fn crop(shot: &Rgba, region: Placed) -> Rgba {
    let px = |v: f32| (v * SCALE).round() as u32;
    let (x, y) = (px(region.x).min(shot.width), px(region.y).min(shot.height));
    let (width, height) = (px(region.width).min(shot.width - x), px(region.height).min(shot.height - y));
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in y..y + height {
        let start = ((row * shot.width + x) * 4) as usize;
        pixels.extend_from_slice(&shot.pixels[start..start + (width * 4) as usize]);
    }
    Rgba { width, height, pixels }
}
