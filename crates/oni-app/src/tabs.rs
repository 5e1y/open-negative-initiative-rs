//! `PanelTab`'s routes: the crop and De-dust are left for the last panel the hand was in, recorded
//! on every route, and Return validates the frames a hand draws, the measuring window first, then DONE.

use crate::PanelTab;

/// `PanelTab.leaving(toward:)`: a way out never lands back on the crop or the dust tool.
pub fn leaving(wanted: PanelTab) -> PanelTab {
    if matches!(wanted, PanelTab::Crop | PanelTab::Dedust) { PanelTab::Color } else { wanted }
}

/// `measuring:` on the canvas: the window drawn while armed on the crop or on Color alone.
pub fn window_shown(armed: bool, tab: PanelTab) -> bool {
    armed && matches!(tab, PanelTab::Crop | PanelTab::Color)
}

/// What Return asks of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validated {
    Nothing,
    Disarmed,
    Leave,
}

/// The tab on screen, `lastPanel` and `measureFraming`, the measuring window armed.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub shown: PanelTab,
    pub last_panel: PanelTab,
    pub armed: bool,
}

impl Default for Route {
    fn default() -> Self {
        Route { shown: PanelTab::Color, last_panel: PanelTab::Color, armed: false }
    }
}

impl Route {
    /// `onChange(of: tab)`: the panel left is recorded unless modal, and leaving the crop disarms.
    pub fn moved(&mut self, next: PanelTab) {
        let old = std::mem::replace(&mut self.shown, next);
        if old == next {
            return;
        }
        if !matches!(old, PanelTab::Crop | PanelTab::Dedust) {
            self.last_panel = old;
        }
        if old == PanelTab::Crop {
            self.armed = false;
        }
    }

    pub fn way_out(&self) -> PanelTab {
        leaving(self.last_panel)
    }

    /// `toggleFraming` and `toggleCorrecting`: into the crop or the dust tool, or out of it to the last panel.
    pub fn toggled(&self, tool: PanelTab) -> PanelTab {
        if self.shown == tool { self.way_out() } else { tool }
    }

    /// `validate(with:)`: the window first where both are armed, so a second Return leaves the crop;
    /// on either tool Return is its DONE.
    pub fn validate(&mut self) -> Validated {
        if self.window_shown() {
            self.armed = false;
            return Validated::Disarmed;
        }
        if matches!(self.shown, PanelTab::Crop | PanelTab::Dedust) { Validated::Leave } else { Validated::Nothing }
    }

    pub fn window_shown(&self) -> bool {
        window_shown(self.armed, self.shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_records_the_panel_left_and_never_a_modal_one() {
        let mut r = Route::default();
        r.moved(PanelTab::Details);
        r.moved(PanelTab::Crop);
        assert_eq!(r.way_out(), PanelTab::Details, "the tab bar's route is recorded");
        assert_eq!(r.toggled(PanelTab::Crop), PanelTab::Details, "the crop's key leaves it as De-dust's does");
        r.moved(PanelTab::Dedust);
        assert_eq!((r.last_panel, r.toggled(PanelTab::Dedust)), (PanelTab::Details, PanelTab::Details), "the crop is never recorded");
        assert_eq!(r.toggled(PanelTab::Crop), PanelTab::Crop, "another tool's key opens its own");
        // Twin: recording every tab left would hold the crop, and De-dust would lead to Color.
        assert_ne!(leaving(PanelTab::Crop), r.toggled(PanelTab::Dedust));
        r.moved(PanelTab::Color);
        r.moved(PanelTab::Color);
        assert_eq!(r.last_panel, PanelTab::Details, "a tab picked again records nothing");
        r.last_panel = PanelTab::Crop;
        assert_eq!(r.way_out(), PanelTab::Color, "a modal panel recorded leads to Color");
    }

    #[test]
    fn return_disarms_the_window_first_then_leaves_either_tool() {
        let mut r = Route { shown: PanelTab::Crop, last_panel: PanelTab::Details, armed: true };
        assert_eq!(r.validate(), Validated::Disarmed);
        assert_eq!(r.validate(), Validated::Leave, "a second Return leaves framing");
        let mut dust = Route { shown: PanelTab::Dedust, last_panel: PanelTab::Color, armed: true };
        assert!(!dust.window_shown());
        assert_eq!(dust.validate(), Validated::Leave, "Return is De-dust's DONE too");
        let mut colour = Route { shown: PanelTab::Color, last_panel: PanelTab::Color, armed: true };
        assert!(colour.window_shown());
        assert_eq!((colour.validate(), colour.validate()), (Validated::Disarmed, Validated::Nothing));
        let mut details = Route { shown: PanelTab::Details, last_panel: PanelTab::Color, armed: true };
        assert!(!details.window_shown(), "Details never draws the window");
        assert_eq!(details.validate(), Validated::Nothing);
        assert!(details.armed, "twin: Return on Details leaves the window armed");
        let mut left = Route { shown: PanelTab::Crop, last_panel: PanelTab::Color, armed: true };
        left.moved(PanelTab::Color);
        assert!(!left.armed, "leaving the crop disarms");
        let mut kept = Route { shown: PanelTab::Color, last_panel: PanelTab::Color, armed: true };
        kept.moved(PanelTab::Details);
        assert!(kept.armed, "twin: leaving Color keeps it armed");
    }
}
