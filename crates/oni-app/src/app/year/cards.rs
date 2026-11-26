//! The year's cards: each scene's taken off the window at a story's size, saved into the year's folder
//! in Downloads or laid on the pasteboard, and what was done said under the keys.

use crate::Wrapped;
use crate::app::{Ctl, with_shell};
use crate::wrapped::share;
use oni_i18n::{tr, trn};
use slint::ComponentHandle;
use std::path::{Path, PathBuf};
use std::rc::Rc;

impl Ctl {
    /// "Save All Cards", or one card: written into the year's folder in Downloads with no question asked,
    /// then shown there, and said under the keys.
    pub(super) fn save_wrapped(self: &Rc<Self>, scene: Option<usize>) {
        let Some(year) = self.state.borrow().year.story.as_ref().map(|s| s.year) else { return };
        self.wrapped_notice(tr("Saving your cards…"));
        let folder = share::folder(&crate::system::downloads(), year);
        let shown = folder.clone();
        self.save_wrapped_into(&folder, scene, move |written| {
            with_shell(move |c| {
                let notice = match &written {
                    Ok(saved) => {
                        match saved.as_slice() {
                            [one] => crate::system::reveal(one),
                            _ => crate::system::show_folder(&shown),
                        }
                        let cards = trn("{n} card saved", "{n} cards saved", saved.len() as u64);
                        tr!("{cards} in Downloads, in {folder}", cards = cards, folder = oni_i18n::named(&share::folder_name(year)))
                    }
                    Err(e) => tr!("The cards cannot be saved: {e}", e = e),
                };
                c.wrapped_notice(&notice);
            });
        });
    }

    fn wrapped_notice(&self, text: &str) {
        if let Some(w) = self.window() {
            w.global::<Wrapped>().set_notice(oni_i18n::spoken(text).into());
        }
    }

    /// A scene's card, 1080 by 1920, as the window draws it: the card laid alone and told whole at its
    /// full size, the window's own picture taken a view's worth at a time and laid together. The story
    /// is back where it stood before a frame is drawn.
    fn card_picture(&self, scene: usize) -> Result<Vec<u8>, String> {
        let Some(index) = self.state.borrow().year.story.as_ref().map(|s| s.index) else { return Err(tr("the summary is closed").into()) };
        let Some(w) = self.window() else { return Err(tr("the summary is closed").into()) };
        let scale = w.window().scale_factor();
        let g = w.global::<Wrapped>();
        // The view's place in the window's picture, on whole pixels.
        let px = |points: f32| (points * scale).round().max(0.0) as usize;
        let view = ((px(g.get_view_x()), px(g.get_view_y())), (px(g.get_view_width()), px(g.get_view_height())));
        let tiles = share::tiles(share::CARD, view.1);
        if tiles.is_empty() {
            return Err(tr("the window is too small").into());
        }
        g.set_still_width(share::CARD.0 as f32 / scale);
        g.set_still(true);
        g.set_index(scene as i32);
        let mut card = vec![0u8; share::CARD.0 * share::CARD.1 * 4];
        let taken = tiles.iter().try_for_each(|(x, y)| {
            g.set_still_x(-(*x as f32) / scale);
            g.set_still_y(-(*y as f32) / scale);
            let shot = w.window().take_snapshot().map_err(|e| e.to_string())?;
            share::lay(&mut card, share::CARD, (*x, *y), shot.as_bytes(), shot.width() as usize, view);
            Ok(())
        });
        g.set_index(index as i32);
        g.set_still(false);
        taken.map(|()| card)
    }

    /// One scene's card, or every scene's, taken off the window one card a turn of the interface's loop,
    /// so the window stays live; each is brought to sRGB, encoded and written into `folder` off this
    /// thread as it is taken, and `done` told there what was written.
    pub fn save_wrapped_into(self: &Rc<Self>, folder: &Path, scene: Option<usize>, done: impl FnOnce(Result<Vec<PathBuf>, String>) + Send + 'static) {
        let Some((year, count)) = self.state.borrow().year.story.as_ref().map(|s| (s.year, s.scenes.len())) else { return done(Ok(Vec::new())) };
        let mut scenes = scene.map_or(0..count, |one| one.min(count.saturating_sub(1))..(one + 1).min(count));
        let (folder, screen) = (folder.to_path_buf(), self.state.borrow().screen);
        let (cards, taken) = std::sync::mpsc::channel::<Result<(usize, Vec<u8>), String>>();
        std::thread::spawn(move || {
            let made = std::fs::create_dir_all(&folder).map_err(|e| e.to_string());
            done(made.and_then(|()| taken.iter().map(|card| card.and_then(|(scene, card)| card_written(card, &folder, year, scene, screen))).collect()))
        });
        let timer = Rc::new(slint::Timer::default());
        let (me, held) = (Rc::downgrade(self), timer.clone());
        // The sender leaves with the timer's closure: the writer then knows every card was handed over.
        let mut cards = Some(cards);
        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(1), move || {
            let next = me.upgrade().zip(scenes.next()).map(|(ctl, scene)| ctl.card_picture(scene).map(|card| (scene, card)));
            // A card that could not be taken is handed over too, as the failure the writer reports.
            let sent = match (next, &cards) {
                (Some(card), Some(to)) => {
                    let taken = card.is_ok();
                    to.send(card).is_ok() && taken
                }
                _ => false,
            };
            if !sent {
                cards = None;
                held.stop();
            }
        });
    }

    /// A scene's card on the pasteboard, to paste anywhere: brought to sRGB off this thread, then laid.
    pub fn copy_wrapped(&self, scene: usize) {
        let screen = self.state.borrow().screen;
        match self.card_picture(scene) {
            Ok(card) => {
                std::thread::spawn(move || {
                    let card = crate::thumbnails::from_screen(card, screen);
                    let forms = share::tiff(&card, share::CARD.0, share::CARD.1).zip(share::dib(&card, share::CARD.0, share::CARD.1));
                    with_shell(move |c| c.card_copied(forms));
                });
            }
            Err(e) => self.wrapped_notice(&tr!("The card cannot be copied: {e}", e = e)),
        }
    }

    fn card_copied(&self, forms: Option<(Vec<u8>, Vec<u8>)>) {
        let Some((tiff, dib)) = forms else { return self.wrapped_notice(&tr!("The card cannot be copied: {e}", e = tr("the card cannot be encoded"))) };
        {
            let mut s = self.state.borrow_mut();
            crate::pasteboard::write_picture(&mut *s.pasteboard, &tiff, &dib);
            s.board_seen = Some(s.pasteboard.change_count());
        }
        self.wrapped_notice(tr("Card copied: paste it where you post."));
    }

    /// The picture the pasteboard holds, which a check reads back.
    pub fn board_picture(&self) -> Option<Vec<u8>> {
        self.state.borrow().pasteboard.data(crate::pasteboard::PICTURE)
    }
}

/// A scene's card, taken off a window drawn for `screen`, brought to sRGB and written into `folder`.
fn card_written(card: Vec<u8>, folder: &Path, year: i32, scene: usize, screen: crate::display::Screen) -> Result<PathBuf, String> {
    let card = crate::thumbnails::from_screen(card, screen);
    let data = share::jpeg(&card, share::CARD.0, share::CARD.1).ok_or_else(|| tr("the card cannot be encoded").to_string())?;
    let path = folder.join(share::file_name(year, scene));
    oni_settings::atomic::write_atomically(&path, &data).map_err(|e| e.to_string())?;
    Ok(path)
}
