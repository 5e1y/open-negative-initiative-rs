//! The Auto levels key run off the interface thread, its targets landed as one undo step.

use super::super::{Ctl, with_shell};
use crate::PanelState;
use oni_i18n::tr;
use oni_settings::PipelineSettings;
use slint::ComponentHandle;
use std::path::Path;
use std::rc::Rc;

impl Ctl {
    /// The Auto levels key: each target read off this thread on its own settings, landed as one
    /// undo step; a slide's lands on one RGB black and white, gains at rest.
    pub(in crate::app) fn auto_levels(self: &Rc<Self>) {
        if self.busy_said(tr("Auto Levels")) {
            return;
        }
        let frames: Vec<(String, PipelineSettings)> = {
            let s = self.state.borrow();
            if s.levelling {
                return;
            }
            let open = s.open_path().map(str::to_string);
            self.together_paths()
                .unwrap_or_else(|| s.balance_targets())
                .into_iter()
                .map(|path| {
                    let settings = if Some(&path) == open.as_ref() {
                        s.store.settings().clone()
                    } else {
                        let mosaic = oni_decode::decodes_via_mosaic(Path::new(&path));
                        oni_settings::sidecar::read(Path::new(&path), mosaic, true).map(|l| l.settings).unwrap_or_default()
                    };
                    (path, settings)
                })
                .collect()
        };
        if frames.is_empty() {
            return;
        }
        self.state.borrow_mut().levelling = true;
        self.publish_levelling();
        std::thread::spawn(move || {
            crate::decode_gate::in_background();
            let found = crate::measuring::levels_outcomes(&frames);
            with_shell(move |c| {
                c.state.borrow_mut().levelling = false;
                c.publish_levelling();
                if found.is_empty() || c.busy_said(tr("Auto Levels")) {
                    return;
                }
                let targets: Vec<String> = found.keys().cloned().collect();
                {
                    let mut s = c.state.borrow_mut();
                    if let Some(e) = s.store.editor() {
                        e.auto_levels(&found);
                    }
                    s.last_change = Some(std::time::Instant::now());
                }
                c.landed(&targets);
            });
        });
    }

    pub(super) fn publish_levelling(&self) {
        if let Some(w) = self.window() {
            w.global::<PanelState>().set_levelling(self.state.borrow().levelling);
        }
    }
}
