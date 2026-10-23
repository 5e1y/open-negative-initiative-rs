//! The Settings sheet the header's gear opens, each switch the same preference as its menu item so
//! the two never disagree, and the key clicks that one of them silences.

use super::*;
use crate::AppSettings;
use std::sync::atomic::Ordering;

impl Ctl {
    pub(super) fn wire_settings(me: &Rc<Self>, window: &MainWindow) {
        let p = me.clone();
        window.global::<AppSettings>().on_flip(move |action| p.act(action));
        let p = me.clone();
        window.global::<AppSettings>().on_close(move || p.act(crate::Act::CloseSettings));
        let p = me.clone();
        window.global::<crate::Clicks>().on_press(move || {
            if !p.state.borrow().muted {
                crate::sound::press();
            }
        });
        let p = me.clone();
        window.global::<crate::Clicks>().on_release(move || {
            if !p.state.borrow().muted {
                crate::sound::release();
            }
        });
    }

    pub(super) fn open_settings(&self) {
        self.publish_settings();
        if let Some(w) = self.window() {
            w.global::<AppSettings>().set_shown(true);
        }
    }

    pub(super) fn close_settings(&self) {
        if let Some(w) = self.window() {
            w.global::<AppSettings>().set_shown(false);
        }
    }

    pub(super) fn publish_settings(&self) {
        let Some(w) = self.window() else { return };
        let s = w.global::<AppSettings>();
        s.set_clicks(!self.state.borrow().muted);
        s.set_keep_zoom(crate::viewport::KEEP_ZOOM.load(Ordering::Relaxed));
        s.set_full_decode(crate::render::FULL_DECODE.load(Ordering::Relaxed));
        s.set_frontier_look(self.frontier_look_line().into());
        s.set_frontier_hand(oni_pipeline::frontier_table::current().is_some());
    }
}
