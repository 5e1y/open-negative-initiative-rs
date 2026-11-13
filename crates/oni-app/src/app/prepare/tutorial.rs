//! The step's demo under its controls, played on every step that has one unless put away, once, resting
//! on its last frame, by the shared clip player.

use super::*;
use crate::app::player::{Pace, Player};
use crate::tutorial;

pub(in crate::app) struct Playing {
    step: usize,
    player: Player,
}

impl Ctl {
    /// Hide: the clip shown is put away, and the next steps show none; Show How plays the step's
    /// clip from its first frame again, and the next steps their own.
    pub(in crate::app) fn toggle_tutorial(&self) {
        let (step, shown) = {
            let r = &self.state.borrow().prepare;
            (r.step.index(), r.tutorial.is_some())
        };
        self.state.borrow_mut().prepare.tutorial_hidden = shown;
        self.play_tutorial((!shown).then_some(step));
    }

    /// Each step plays its own clip from its start, played once and resting on its last, unless the
    /// clips are put away; on another step, or none, the clip shown goes.
    pub(in crate::app) fn keep_tutorial(&self, step: Option<usize>) {
        let (playing, hidden) = {
            let r = &self.state.borrow().prepare;
            (r.tutorial.as_ref().map(|p| p.step), r.tutorial_hidden)
        };
        let wanted = step.filter(|_| !hidden);
        if playing != wanted {
            self.play_tutorial(wanted);
        }
    }

    /// The clip of `step` from its start, or none shown.
    fn play_tutorial(&self, step: Option<usize>) {
        let playing = step.and_then(|step| Some(Playing { step, player: Player::start(tutorial::clip(step)?, Pace::Once, Ctl::tutorial_frame)? }));
        let started = playing.is_some();
        let stopped = std::mem::replace(&mut self.state.borrow_mut().prepare.tutorial, playing).is_some();
        if stopped && let Some(w) = self.window() {
            w.global::<Preparing>().set_tutorial(Image::default());
        }
        if started {
            self.tutorial_frame();
        }
    }

    /// The clip's next frame on the panel, when the player has one.
    fn tutorial_frame(&self) {
        let Some(w) = self.window() else { return };
        let shown = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let Some(p) = s.prepare.tutorial.as_mut() else { return };
            p.player.step(&s.gpu)
        };
        let Some(shown) = shown else { return };
        let p = w.global::<Preparing>();
        if let Some(image) = shown.image {
            p.set_tutorial(image);
        }
        p.set_tutorial_revision(shown.revision);
    }
}
