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
        window.global::<AppSettings>().on_language_picked(move |i| p.pick_language(i));
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
        self.show_phones();
        if let Some(w) = self.window() {
            w.global::<AppSettings>().set_shown(true);
        }
    }

    pub(super) fn close_settings(&self) {
        if let Some(w) = self.window() {
            w.global::<AppSettings>().set_shown(false);
        }
    }

    /// The Settings' language: 0 the system's, then English, French, Spanish; every text set from
    /// here is written again in it, the interface's own changing by itself.
    fn pick_language(self: &Rc<Self>, index: i32) {
        let language = usize::try_from(index - 1).ok().and_then(|i| oni_i18n::Language::ALL.get(i).copied());
        self.remember(preferences::LANGUAGE, language.map_or(serde_json::Value::Null, |l| l.key().into()));
        crate::language::speak(language);
        self.relocalised();
    }

    /// Every text the Rust side lays in the window, written again in the language now spoken.
    fn relocalised(self: &Rc<Self>) {
        self.publish_menus();
        self.publish_bar();
        self.publish_settings();
        self.publish_status();
        self.publish_tree();
        self.publish_navigator();
        self.publish_info();
        self.publish_framing();
        self.publish_dedust();
        self.publish_roll_tab();
        self.publish_presets();
        self.publish_export();
        self.publish_lab();
        self.publish_organising();
        self.publish_projects();
        self.publish_prepare();
    }

    pub(super) fn publish_settings(&self) {
        let Some(w) = self.window() else { return };
        let s = w.global::<AppSettings>();
        let chosen = crate::language::chosen(&self.state.borrow().preferences);
        s.set_language(chosen.and_then(|l| oni_i18n::Language::ALL.iter().position(|a| *a == l)).map_or(0, |i| i as i32 + 1));
        s.set_clicks(!self.state.borrow().muted);
        s.set_keep_zoom(crate::viewport::KEEP_ZOOM.load(Ordering::Relaxed));
        s.set_full_decode(crate::render::FULL_DECODE.load(Ordering::Relaxed));
        s.set_frontier_look(self.frontier_look_line().into());
        s.set_frontier_hand(oni_pipeline::frontier_table::current().is_some());
        s.set_screen_gamut(self.state.borrow().screen_gamut);
        s.set_version(oni_i18n::tr!("Version {version}", version = crate::updates::version()).into());
        let pairing = w.global::<crate::Pairing>();
        s.set_phones(pairing.get_phones());
        s.set_can_forget(pairing.get_can_forget());
    }
}
