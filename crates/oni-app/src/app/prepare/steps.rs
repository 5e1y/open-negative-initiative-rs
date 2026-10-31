//! The steps: walking them, picking and flagging frames, the window, and the draft each saves.

use super::*;

impl Ctl {
    // MARK: - The steps

    pub(in crate::app) fn step_to(self: &Rc<Self>, step: Step) {
        let changed = {
            let r = &mut self.state.borrow_mut().prepare;
            if !r.shown || r.progress.is_some() || step.skipped(r.capture, r.walked()) {
                return;
            }
            let changed = r.step != step;
            r.step = step;
            changed
        };
        if changed {
            self.save_draft();
        }
        self.publish_prepare();
    }

    pub(in crate::app) fn roll_advance(self: &Rc<Self>) {
        let (step, capture, film) = {
            let r = &self.state.borrow().prepare;
            (r.step, r.capture, r.walked())
        };
        if step == Step::Window {
            return self.measure_roll();
        }
        self.step_to(neighbour(step, 1, capture, film));
    }

    /// `onCancel`: Stop while measuring, else the roll left.
    pub(in crate::app) fn roll_cancel(self: &Rc<Self>) {
        let measuring = {
            let r = &self.state.borrow().prepare;
            if r.progress.is_some() {
                r.stop.store(true, Ordering::Relaxed);
            }
            r.progress.is_some()
        };
        if !measuring {
            self.leave_roll();
        }
    }

    /// Escape and, on the window step, the arrows; any other key goes on to the menus.
    pub(in crate::app) fn prepare_key(self: &Rc<Self>, text: &str, command: bool, shift: bool, bare: bool) -> bool {
        if text == "\u{1b}" {
            self.roll_cancel();
            return true;
        }
        let (step, measuring) = {
            let r = &self.state.borrow().prepare;
            (r.step, r.progress.is_some())
        };
        if step != Step::Window || measuring || command || !(bare || shift) {
            return false;
        }
        match text {
            "\u{F702}" => self.walk_window(-1),
            "\u{F703}" => self.walk_window(1),
            _ => return false,
        }
        true
    }

    pub(in crate::app) fn walk_window(self: &Rc<Self>, by: i32) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            let last = measured(&r.frames, r.walked()).len().saturating_sub(1) as i32;
            r.at = (r.at as i32 + by).clamp(0, last.max(0)) as usize;
        }
        self.publish_prepare();
    }

    pub(in crate::app) fn walk_base(self: &Rc<Self>, by: i32) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            let last = r.frames.len().saturating_sub(1) as i32;
            r.base_frame = (r.base_frame as i32 + by).clamp(0, last.max(0)) as usize;
            r.pointer = None;
        }
        self.publish_prepare();
    }

    /// `RollPanel.click`: the flat-field's pick, or the scan the base step shows.
    pub(in crate::app) fn cell_clicked(self: &Rc<Self>, i: usize) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            let Some(path) = r.frames.get(i).map(|f| f.path.clone()) else { return };
            match r.step {
                Step::FlatField => r.flat_pick = Some(path),
                Step::Base => {
                    r.base_frame = i;
                    r.pointer = None;
                }
                _ => return,
            }
        }
        self.publish_prepare();
    }

    pub(in crate::app) fn roll_mask_chosen(self: &Rc<Self>, i: Option<usize>) {
        {
            let mut s = self.state.borrow_mut();
            let options = crate::app::roll_tab::mask_options(&s.prepare.flat, &s.masks);
            let Some((line, _)) = i.and_then(|i| options.get(i)).cloned() else { return };
            s.prepare.flat = crate::app::roll_tab::chose(&s.prepare.flat, line);
        }
        self.publish_prepare();
    }
}
