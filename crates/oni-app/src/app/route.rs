//! The panel's routes and the editor's modes: a tool's key opens it and leaves it for the last panel,
//! Escape leaves the innermost mode, every photo opens with none, and Return validates then leaves.

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

    /// `toggleFraming()` and `toggleCorrecting()`: a tool's key, menu item or shortcut opens it, or
    /// leaves it for the last panel; De-dust, a tool of Pro's face, leaves the Lab first.
    pub(super) fn toggle_tool(self: &Rc<Self>, tool: i32) {
        let Some(w) = self.window() else { return };
        if tool == tabs::DEDUST {
            self.leave_lab();
        }
        self.sync_route(w.global::<PanelState>().get_tab());
        let next = self.state.borrow().route.toggled(tool);
        self.route_to(next);
        self.publish_dedust();
    }

    /// The crop or De-dust, each frame's own, left for the last panel; `false` on any other tab.
    pub(super) fn leave_frame_tools(self: &Rc<Self>) -> bool {
        let Some(w) = self.window() else { return false };
        let tab = w.global::<PanelState>().get_tab();
        if tab != tabs::CROP && tab != tabs::DEDUST {
            return false;
        }
        self.sync_route(tab);
        let out = self.state.borrow().route.way_out();
        self.route_to(out);
        self.publish_dedust();
        true
    }

    /// Every photo opens with no mode: Compare closed, the crop and De-dust left for the last panel.
    pub(super) fn leave_modes(self: &Rc<Self>) {
        if self.state.borrow().compare.split.is_some() {
            self.toggle_compare();
        }
        self.leave_frame_tools();
    }

    /// Escape in the editor: the innermost mode left, Compare over the canvas before the panel's
    /// tool; `false` with none open.
    pub(super) fn leave_mode(self: &Rc<Self>) -> bool {
        if self.window().is_none_or(|w| w.global::<App>().get_gallery()) {
            return false;
        }
        if self.state.borrow().compare.split.is_some() {
            self.toggle_compare();
            return true;
        }
        self.leave_frame_tools()
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
            Validated::LeaveCrop => {
                self.leave_frame_tools();
            }
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
