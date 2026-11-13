//! The welcome's film on its stage: the page's film looped by the shared clip player, its last frame
//! still under Reduce Motion; a page without one, or the sheet closing, drops the decoder and the texture.

use crate::app::Ctl;
use crate::app::help::player::{Pace, Player};
use crate::welcome::Film;
use crate::{Help, Motion};
use slint::{ComponentHandle, Image};

impl Ctl {
    /// `Help.welcome-turned`: the page now shown, -1 as the sheet closes.
    pub(super) fn welcome_turned(&self, page: i32) {
        let Some(w) = self.window() else { return };
        let film = Film::of_page(page);
        if self.state.borrow().help.clip.as_ref().map(|(f, _)| *f) == film {
            return;
        }
        let pace = if w.global::<Motion>().get_reduced() { Pace::Still } else { Pace::Loop };
        let clip = film.and_then(|f| (self.state.borrow().help.films)(f));
        let playing = film.zip(clip).and_then(|(film, clip)| Some((film, Player::start(clip, pace, Ctl::welcome_frame)?)));
        let started = playing.is_some();
        let stopped = std::mem::replace(&mut self.state.borrow_mut().help.clip, playing).is_some();
        // The page turned to shows nothing of the film before it, even for the few milliseconds its own takes.
        if stopped {
            w.global::<Help>().set_welcome_clip(Image::default());
        }
        if started {
            self.welcome_frame();
        }
    }

    /// The film's next frame on the stage, when the player has one.
    fn welcome_frame(&self) {
        let Some(w) = self.window() else { return };
        let shown = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let Some((_, player)) = s.help.clip.as_mut() else { return };
            player.step(&s.gpu)
        };
        let Some(shown) = shown else { return };
        let h = w.global::<Help>();
        if let Some(image) = shown.image {
            h.set_welcome_clip(image);
        }
        h.set_welcome_clip_revision(shown.revision);
    }
}
