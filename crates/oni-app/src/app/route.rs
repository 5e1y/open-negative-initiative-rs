//! The panel's routes: ⌘K opens the crop, the crop and De-dust are left for the last panel the
//! hand was in, Return validates the measuring window then the crop, and a balance disarms.

use super::*;
use crate::tabs::{self, Validated};

impl Ctl {
    /// A tab shown by a route of the app's own; the tab bar's reaches `tab_opened` directly.
    fn route_to(self: &Rc<Self>, next: i32) {
        let Some(w) = self.window() else { return };
        let panel = w.global::<PanelState>();
        self.sync_route(panel.get_tab());
        panel.set_tab(next);
        self.tab_opened(next);
    }

    /// A tab set where no route sees it, as the benches do, is read back before any route reads.
    fn sync_route(&self, tab: i32) {
        let mut s = self.state.borrow_mut();
        if s.route.shown != tab {
            s.route.moved(tab);
        }
    }

    /// `openCrop()`: ⌘K and the bar's key open the crop and never leave it.
    pub(super) fn open_crop(self: &Rc<Self>) {
        self.route_to(tabs::CROP);
    }

    /// `toggleFraming()`, which Return reaches from the crop: out to the last panel, disarmed.
    fn leave_crop(self: &Rc<Self>) {
        let out = {
            let mut s = self.state.borrow_mut();
            s.route.armed = false;
            s.route.way_out()
        };
        self.route_to(out);
    }

    /// `toggleCorrecting()`: the Lab left first, then into the dust tool or out to the last panel.
    pub(super) fn toggle_correcting(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        self.leave_lab();
        self.sync_route(w.global::<PanelState>().get_tab());
        let next = self.state.borrow().route.correcting();
        self.route_to(next);
        self.publish_dedust();
    }

    /// `validate(with:)`: Return or Enter, bare and once, on Pro's editor; the Lab takes none.
    pub(super) fn validate_key(self: &Rc<Self>, text: &str, bare: bool, repeat: bool) -> bool {
        let Some(w) = self.window() else { return false };
        if !matches!(text, "\r" | "\n" | "\u{3}") || !bare || repeat || w.global::<App>().get_gallery() || self.in_lab() {
            return false;
        }
        self.sync_route(w.global::<PanelState>().get_tab());
        let validated = self.state.borrow_mut().route.validate();
        match validated {
            Validated::Disarmed => self.publish_framing(),
            Validated::LeaveCrop => self.leave_crop(),
            Validated::Nothing => return false,
        }
        true
    }

    /// A balance key pressed or a balance landing: the window's work is done.
    pub(super) fn disarm_window(&self) {
        self.state.borrow_mut().route.armed = false;
        self.publish_framing();
    }
}
