//! `DSSection`'s fold remembered per title under `sectionFold.<title>`: "folded" or "open" once the
//! hand has folded the block, absent for the block's own default.

use super::Ctl;
use crate::{Editors, PanelState};
use slint::ComponentHandle;
use std::rc::Rc;

pub const PREFIX: &str = "sectionFold.";

/// The editors' blocks, which hold their fold in `Editors`; each opens folded by default.
const EDITORS: [&str; 4] = ["Curves", "Per-Channel Levels", "Spectrogram Finishing", "Saturation Curve"];

/// `DSSection.folded`: an empty or absent value is the default; any other but "folded" is open.
pub fn folded(stored: Option<&str>, by_default: bool) -> bool {
    match stored {
        None | Some("") => by_default,
        Some(value) => value == "folded",
    }
}

impl Ctl {
    pub(super) fn wire_folds(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let panel = window.global::<PanelState>();
        let c = Rc::downgrade(me);
        panel.on_fold_of(move |title, by_default| {
            // A binding may be evaluated while the state is held: the default then, never a panic.
            let Some(c) = c.upgrade() else { return by_default };
            let Ok(s) = c.state.try_borrow() else { return by_default };
            folded(s.preferences.string(&format!("{PREFIX}{title}")), by_default)
        });
        panel.set_folds_read(true);
        let c = Rc::downgrade(me);
        panel.on_fold_changed(move |title, folded| {
            if let Some(c) = c.upgrade() {
                c.remember(&format!("{PREFIX}{title}"), (if folded { "folded" } else { "open" }).into());
            }
        });
        let stored: Vec<bool> = {
            let s = me.state.borrow();
            EDITORS.iter().map(|t| folded(s.preferences.string(&format!("{PREFIX}{t}")), true)).collect()
        };
        let editors = window.global::<Editors>();
        editors.set_curves_folded(stored[0]);
        editors.set_levels_folded(stored[1]);
        editors.set_spectrogram_folded(stored[2]);
        editors.set_saturation_folded(stored[3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_fold_wins_over_the_default_and_an_empty_one_leaves_it() {
        assert!(folded(None, true) && !folded(None, false));
        assert!(folded(Some(""), true), "empty is the default");
        assert!(folded(Some("folded"), false));
        assert!(!folded(Some("open"), true));
        assert!(!folded(Some("anything"), true), "twin: only \"folded\" folds");
    }
}
