//! The panel's routes and the editor's modes: a tool's key, DONE or Return keeps its work and leaves it
//! for the last panel, its Cancel or Escape puts its work back first, and every photo opens with no mode.

use super::*;
use crate::PanelTab;
use crate::tabs::Validated;

impl Ctl {
    /// A tab shown by a route of the app's own; the tab bar's reaches `tab_opened` directly.
    fn route_to(self: &Rc<Self>, next: PanelTab) {
        let Some(w) = self.window() else { return };
        let panel = w.global::<PanelState>();
        self.sync_route(panel.get_tab());
        panel.set_tab(next);
        self.tab_opened(next);
    }

    /// A tab set where no route sees it, as the benches do, is read back before any route reads.
    fn sync_route(&self, tab: PanelTab) {
        if self.state.borrow().route.shown != tab {
            self.route_moved(tab);
        }
    }

    /// The route moved: a canvas tool entered keeps its frame's settings as found, for its Cancel.
    pub(super) fn route_moved(&self, tab: PanelTab) {
        let mut s = self.state.borrow_mut();
        let left = s.route.shown;
        s.route.moved(tab);
        if tab != left {
            let found = matches!(tab, PanelTab::Crop | PanelTab::Dedust).then(|| s.store.settings().clone());
            if let Some(f) = s.frame.as_mut() {
                f.tool_entry = found;
            }
        }
    }

    /// `toggleFraming()` and `toggleCorrecting()`: a tool's key, menu item or shortcut opens it, or
    /// leaves it for the last panel; De-dust, a tool of Pro's face, leaves the Lab first.
    pub(super) fn toggle_tool(self: &Rc<Self>, tool: PanelTab) {
        let Some(w) = self.window() else { return };
        if tool == PanelTab::Dedust {
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
        if !matches!(tab, PanelTab::Crop | PanelTab::Dedust) {
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
        if self.state.borrow().split().is_some() {
            self.toggle_compare();
        }
        self.leave_frame_tools();
    }

    /// Escape or a tool's Cancel: what the crop or De-dust changed on its frame put back as the tool
    /// found it, one undo step, then the tool left; `false` on any other tab.
    pub(super) fn cancel_frame_tools(self: &Rc<Self>) -> bool {
        let Some(w) = self.window() else { return false };
        let tab = w.global::<PanelState>().get_tab();
        if !matches!(tab, PanelTab::Crop | PanelTab::Dedust) {
            return false;
        }
        self.sync_route(tab);
        let found = {
            let s = self.state.borrow();
            let now = s.store.settings();
            match s.frame.as_ref().and_then(|f| f.tool_entry.as_ref()) {
                Some(was) if tab == PanelTab::Crop && was.geometry != now.geometry => Some(oni_settings::PipelineSettings { geometry: was.geometry, ..now.clone() }),
                Some(was) if tab == PanelTab::Dedust && was.corrections != now.corrections => Some(oni_settings::PipelineSettings { corrections: was.corrections.clone(), ..now.clone() }),
                _ => None,
            }
        };
        if let Some(found) = found {
            self.write(Command::Write(Box::new(found)));
        }
        self.leave_frame_tools()
    }

    /// Escape the photos hear beside a photo: Compare left, else the crop or De-dust cancelled, else
    /// the Lab, else the editor for the gallery; `false` in the gallery, where nothing stands open.
    pub(super) fn escape(self: &Rc<Self>) -> bool {
        if self.window().is_none_or(|w| w.global::<App>().get_gallery()) {
            return false;
        }
        let compare = self.state.borrow().split().is_some();
        if compare {
            self.toggle_compare();
        } else if !self.cancel_frame_tools() {
            if self.in_lab() {
                self.leave_lab();
            } else {
                self.act(crate::Act::ToggleMode);
            }
        }
        true
    }

    /// `validate(with:)`: Return or Enter, bare and once, on Pro's editor, a tool's DONE; the Lab takes none.
    pub(super) fn validate_key(self: &Rc<Self>, text: &str, bare: bool, repeat: bool) -> bool {
        let Some(w) = self.window() else { return false };
        if !matches!(text, "\r" | "\n" | "\u{3}") || !bare || repeat || w.global::<App>().get_gallery() || self.in_lab() {
            return false;
        }
        self.sync_route(w.global::<PanelState>().get_tab());
        let validated = self.state.borrow_mut().route.validate();
        match validated {
            Validated::Disarmed => self.publish_framing(),
            Validated::Leave => {
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
