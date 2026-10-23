//! The Details loupe in the window: the band's region of the full-resolution frame, which Details
//! builds, rendered off this thread with one render in flight and the latest asked kept, then encoded
//! for the screen by the canvas's display kernel.

use super::{Ctl, with_shell};
use crate::display::{Params, Screen};
use crate::latest::{Latest, Taken};
use crate::viewport::Viewport;
use crate::{App, DetailLoupe, PanelState, loupe};
use oni_settings::PipelineSettings;
use slint::{ComponentHandle, Image};
use std::rc::Rc;

/// What a loupe render shows: the settings, the view, the band, the ratio, the frame and the screen.
type Key = (PipelineSettings, Viewport, (f64, f64), f64, u64, Screen);

pub(super) struct Held {
    pub(super) magnification: f64,
    /// The last render asked, which a pulse asking the same does not ask again.
    asked: Option<Key>,
    shown: Option<Key>,
    /// Renders shown, which a bench counts.
    count: u64,
    /// The texture the band shows, rewritten in place while the region keeps its size.
    texture: Option<wgpu::Texture>,
}

impl Default for Held {
    fn default() -> Self {
        Held { magnification: loupe::REST, asked: None, shown: None, count: 0, texture: None }
    }
}

/// A loupe render: the full frame, its settings, what the render shows and the frame's path.
struct Ask {
    full: oni_graph::Source,
    settings: PipelineSettings,
    key: Key,
    path: String,
}

/// The latest render asked, which the loupe's thread takes; a newer one replaces it unseen.
static SLOT: Latest<Ask> = Latest::new();
static RENDERING: std::sync::Once = std::sync::Once::new();

/// The loupe's thread: the latest region asked, rendered, then shown if its frame is still open.
fn render_loupes(engine: crate::render::Shared) {
    while let Taken::Value(Ask { full, settings, key, path }) = SLOT.take(None) {
        let (_, viewport, drawable, m, _, _) = key;
        let made = engine.lock().and_then(|mut e| e.loupe(&full, &settings, &viewport, drawable, m)).map(|laid| laid.map(|(source, _)| source));
        with_shell(move |c| {
            let open = c.state.borrow().open_path() == Some(path.as_str());
            match made {
                Ok(Some(source)) if open => c.show_loupe(&source, m, key),
                Ok(_) => {}
                Err(e) => crate::timing::line(&format!("loupe not rendered: {e}")),
            }
        });
    }
}

impl Ctl {
    pub(super) fn wire_loupe(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let c = Rc::downgrade(me);
        window.global::<DetailLoupe>().on_toggled(move || {
            let Some(c) = c.upgrade() else { return };
            let m = {
                let mut s = c.state.borrow_mut();
                s.loupe.magnification = loupe::toggled(s.loupe.magnification);
                s.loupe.magnification
            };
            if let Some(w) = c.window() {
                w.global::<DetailLoupe>().set_ratio(loupe::label(m).into());
            }
            c.request_loupe();
        });
        window.global::<DetailLoupe>().set_ratio(loupe::label(loupe::REST).into());
    }

    /// The Details tab open on a photo in the editor.
    fn loupe_wanted(&self) -> bool {
        let Some(w) = self.window() else { return false };
        !w.global::<App>().get_gallery() && w.global::<PanelState>().get_tab() == 3 && self.state.borrow().open.is_some()
    }

    /// Asks the loupe's render when what it shows has changed; the full frame is built first.
    pub(super) fn request_loupe(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        let band = w.global::<DetailLoupe>();
        band.set_ratio_shown(self.state.borrow().open.is_some());
        if !self.loupe_wanted() {
            return;
        }
        let scale = f64::from(w.window().scale_factor());
        let drawable = (f64::from(band.get_band_width()) * scale, f64::from(band.get_band_height()) * scale);
        let Some(full) = self.ensure_full() else {
            if self.state.borrow_mut().shown_loupe_cleared() {
                band.set_source(Image::default());
            }
            return;
        };
        let ask = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.open_path().map(str::to_string) else { return };
            let settings = s.live_settings();
            let key: Key = (settings.clone(), s.viewport, drawable, s.loupe.magnification, full.id(), s.screen);
            if s.loupe.shown.as_ref() == Some(&key) || s.loupe.asked.as_ref() == Some(&key) {
                return;
            }
            s.loupe.asked = Some(key.clone());
            Ask { full, settings, key, path }
        };
        let engine = self.engine.clone();
        RENDERING.call_once(move || {
            std::thread::Builder::new().name("loupe".into()).spawn(move || render_loupes(engine)).expect("loupe thread");
        });
        SLOT.put(ask);
    }

    /// The region encoded into a texture of its own, which the band shows.
    fn show_loupe(&self, source: &oni_graph::Source, m: f64, key: Key) {
        let Some(win) = self.window() else { return };
        let Some(view) = source.view() else { return };
        let e = source.extent();
        let (w, h) = (e.w, e.h);
        let image = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let g = &s.gpu;
            let (target, shown) = g.encode(s.loupe.texture.as_ref(), (w, h), view, &Params::new(key.5, w, h, 0), (w, h));
            // A texture the band refuses is not kept, so the next render imports another.
            let image = match shown {
                Ok(image) => {
                    s.loupe.texture = Some(target);
                    image
                }
                Err(_) => {
                    s.loupe.texture = None;
                    Some(Image::default())
                }
            };
            image
        };
        let scale = f64::from(win.window().scale_factor());
        let band = win.global::<DetailLoupe>();
        if let Some(image) = image {
            band.set_source(image);
        }
        band.set_revision(band.get_revision().wrapping_add(1));
        band.set_shown_width((f64::from(w) * m / scale) as f32);
        band.set_shown_height((f64::from(h) * m / scale) as f32);
        let mut s = self.state.borrow_mut();
        s.loupe.shown = Some(key);
        s.loupe.count += 1;
    }
}

impl super::State {
    /// Forgets the loupe's picture once its frame is gone; true when there was one to clear.
    fn shown_loupe_cleared(&mut self) -> bool {
        self.loupe.texture = None;
        self.loupe.asked = None;
        self.loupe.shown.take().is_some()
    }

    /// The loupe's ratio, read by the checks.
    pub fn loupe_magnification(&self) -> f64 {
        self.loupe.magnification
    }

    /// How many loupe renders have been shown.
    pub fn loupes(&self) -> u64 {
        self.loupe.count
    }

    /// The loupe has shown a render of the open frame's full resolution.
    pub fn loupe_shown(&self) -> bool {
        self.loupe.shown.is_some()
    }
}
