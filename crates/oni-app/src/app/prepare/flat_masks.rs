//! The Flat-Field step's masks: a frame of the roll taken as one (`takeRollFlatField`), or a
//! picture chosen by Load… (`FlatFieldBlock.load`), each decoded off this thread.

use super::*;
use crate::app::masks;
use crate::flat_fields::{self, Entry, Failure};
use oni_settings::frame::FlatFieldMask;

impl Ctl {
    /// `takeRollFlatField`: the picked frame decoded as the roll's mask at the whole amount, then
    /// left out of the measurement it would skew; never offered to move out of the library.
    pub(in crate::app) fn take_roll_mask(self: &Rc<Self>) {
        let (path, support) = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.prepare.flat_pick.clone().filter(|_| !s.masking.setting) else { return };
            s.masking.setting = true;
            // The decode reads the frame's sidecar, the open frame's as the hand left it.
            let _ = s.store.flush();
            (path, s.support.clone())
        };
        self.publish_prepare();
        self.publish_roll_tab();
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let result = flat_fields::load(&support, Path::new(&path), |p| masks::picture(&engine, p));
            with_shell(move |c| c.roll_mask_taken(&path, result));
        });
    }

    fn roll_mask_taken(self: &Rc<Self>, path: &str, result: Result<Entry, Failure>) {
        self.state.borrow_mut().masking.setting = false;
        self.masks_changed();
        match result {
            Ok(entry) => {
                let r = &mut self.state.borrow_mut().prepare;
                r.flat.mask = FlatFieldMask::Custom;
                r.flat.custom = Some(entry.id);
                r.flat.amount = FlatField::default().amount;
                if let Some(f) = r.frames.iter_mut().find(|f| f.path == path) {
                    f.ticked = false;
                }
            }
            Err(failure) => self.advise(tr("No Mask Set"), failure.message()),
        }
        self.publish_prepare();
        self.publish_roll_tab();
    }

    /// `FlatFieldBlock.load`: a picture of the empty panel chosen, decoded off this thread, then
    /// named as the roll's mask, its Amount kept.
    pub(in crate::app) fn load_roll_mask(self: &Rc<Self>) {
        if self.state.borrow().prepare.loading_mask {
            return;
        }
        // Taken out first: a modal panel runs a nested event loop, which may reach the state.
        let chooser = std::mem::replace(&mut self.state.borrow_mut().masking.chooser, Box::new(|| None));
        let chosen = chooser();
        self.state.borrow_mut().masking.chooser = chooser;
        let Some(file) = chosen else { return };
        let support = {
            let mut s = self.state.borrow_mut();
            s.prepare.loading_mask = true;
            s.support.clone()
        };
        self.publish_prepare();
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let result = flat_fields::load(&support, &file, |p| masks::picture(&engine, p));
            with_shell(move |c| c.roll_mask_loaded(result));
        });
    }

    fn roll_mask_loaded(self: &Rc<Self>, result: Result<Entry, Failure>) {
        self.state.borrow_mut().prepare.loading_mask = false;
        self.masks_changed();
        match result {
            Ok(entry) => {
                let r = &mut self.state.borrow_mut().prepare;
                r.flat.mask = FlatFieldMask::Custom;
                r.flat.custom = Some(entry.id);
            }
            Err(failure) => self.advise(tr("No Mask Loaded"), failure.message()),
        }
        self.publish_prepare();
        self.publish_roll_tab();
    }
}
