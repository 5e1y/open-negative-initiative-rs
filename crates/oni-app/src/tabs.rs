//! `PanelTab`'s routes: the crop and De-dust are left for the last panel the hand was in, recorded
//! on every route, and Return validates the frames a hand draws, the measuring window first.

pub const ROLL: i32 = 0;
pub const CROP: i32 = 1;
pub const COLOR: i32 = 2;
pub const DEDUST: i32 = 4;

/// `PanelTab.leaving(toward:)`: a way out never lands back on the crop or the dust tool.
pub fn leaving(wanted: i32) -> i32 {
    if wanted == CROP || wanted == DEDUST { COLOR } else { wanted }
}

/// `measuring:` on the canvas: the window drawn while armed on the crop or on Color alone.
pub fn window_shown(armed: bool, tab: i32) -> bool {
    armed && (tab == CROP || tab == COLOR)
}

/// What Return asks of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validated {
    Nothing,
    Disarmed,
    LeaveCrop,
}

/// The tab on screen, `lastPanel` and `measureFraming`, the measuring window armed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub shown: i32,
    pub last_panel: i32,
    pub armed: bool,
}

impl Default for Route {
    fn default() -> Self {
        Route { shown: COLOR, last_panel: COLOR, armed: false }
    }
}

impl Route {
    /// `onChange(of: tab)`: the panel left is recorded unless modal, and leaving the crop disarms.
    pub fn moved(&mut self, next: i32) {
        let old = std::mem::replace(&mut self.shown, next);
        if old == next {
            return;
        }
        if old != CROP && old != DEDUST {
            self.last_panel = old;
        }
        if old == CROP {
            self.armed = false;
        }
    }

    pub fn way_out(&self) -> i32 {
        leaving(self.last_panel)
    }

    /// `toggleCorrecting`: into the dust tool, or out of it to the last panel.
    pub fn correcting(&self) -> i32 {
        if self.shown == DEDUST { self.way_out() } else { DEDUST }
    }

    /// `validate(with:)`: the window first where both are armed, so a second Return leaves the crop.
    pub fn validate(&mut self) -> Validated {
        if self.window_shown() {
            self.armed = false;
            return Validated::Disarmed;
        }
        if self.shown == CROP { Validated::LeaveCrop } else { Validated::Nothing }
    }

    pub fn window_shown(&self) -> bool {
        window_shown(self.armed, self.shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DETAILS: i32 = 3;

    #[test]
    fn every_route_records_the_panel_left_and_never_a_modal_one() {
        let mut r = Route::default();
        r.moved(DETAILS);
        r.moved(CROP);
        assert_eq!(r.way_out(), DETAILS, "the tab bar's route is recorded");
        r.moved(DEDUST);
        assert_eq!((r.last_panel, r.correcting()), (DETAILS, DETAILS), "the crop is never recorded");
        // Twin: recording every tab left would hold the crop, and De-dust would lead to Color.
        assert_ne!(leaving(CROP), r.correcting());
        r.moved(COLOR);
        r.moved(COLOR);
        assert_eq!(r.last_panel, DETAILS, "a tab picked again records nothing");
        r.last_panel = CROP;
        assert_eq!(r.way_out(), COLOR, "a modal panel recorded leads to Color");
    }

    #[test]
    fn return_disarms_the_window_first_then_leaves_the_crop() {
        let mut r = Route { shown: CROP, last_panel: DETAILS, armed: true };
        assert_eq!(r.validate(), Validated::Disarmed);
        assert_eq!(r.validate(), Validated::LeaveCrop, "a second Return leaves framing");
        let mut colour = Route { shown: COLOR, last_panel: COLOR, armed: true };
        assert!(colour.window_shown());
        assert_eq!((colour.validate(), colour.validate()), (Validated::Disarmed, Validated::Nothing));
        let mut details = Route { shown: DETAILS, last_panel: COLOR, armed: true };
        assert!(!details.window_shown(), "Details never draws the window");
        assert_eq!(details.validate(), Validated::Nothing);
        assert!(details.armed, "twin: Return on Details leaves the window armed");
        let mut left = Route { shown: CROP, last_panel: COLOR, armed: true };
        left.moved(COLOR);
        assert!(!left.armed, "leaving the crop disarms");
        let mut kept = Route { shown: COLOR, last_panel: COLOR, armed: true };
        kept.moved(DETAILS);
        assert!(kept.armed, "twin: leaving Color keeps it armed");
    }
}
