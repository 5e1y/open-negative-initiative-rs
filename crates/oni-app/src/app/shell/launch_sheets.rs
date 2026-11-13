//! The sheets a launch owes, shown one at a time and never one over another: the welcome first, as it
//! carries on from the splash, then the conversion notice once it closes.

use super::*;
use crate::{Help, Upgraded};

/// A sheet the launch may owe, in the order they show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::app) enum LaunchSheet {
    Welcome,
    Converted,
}

const ORDER: [LaunchSheet; 2] = [LaunchSheet::Welcome, LaunchSheet::Converted];

impl Ctl {
    /// The first sheet the launch owes after `closed`, from the first for `None`, raised while no
    /// launch sheet shows.
    pub(in crate::app) fn launch_sheet_after(&self, closed: Option<LaunchSheet>) {
        let Some(w) = self.window() else { return };
        if w.global::<Help>().get_welcome_shown() || w.global::<Upgraded>().get_shown() {
            return;
        }
        for sheet in ORDER.into_iter().filter(|sheet| closed.is_none_or(|closed| *sheet > closed)) {
            let raised = match sheet {
                LaunchSheet::Welcome => self.welcome_due() && self.open_welcome(),
                LaunchSheet::Converted => self.tell_migration_if_due(),
            };
            if raised {
                return;
            }
        }
    }
}
