//! Staging a scene: its giant figure fitted to the frame, and the photo it is given, so that every
//! frame shows one of the year's photos and no two scenes show the same while there are others.

use super::fit::fit;
use super::words::figure;
use super::{Kind, Scene};
use oni_library::wrapped::Year;

/// The box a figure holds in, and the sizes it is set between, in the frame's units.
pub const ACROSS: f32 = 292.0;
pub const DOWN: f32 = 112.0;
pub const MOST: f32 = 124.0;
pub const LEAST: f32 = 30.0;
/// The photos spread over the year that follow its gold in the pool.
const SPREAD: usize = 24;

pub fn upper(text: &str) -> String {
    text.to_uppercase()
}

/// A scene whose figure is a number, counted up as it shows.
pub fn counted_scene(eyebrow: &str, count: usize, unit: &str, headline: &str, note: String, photo: Option<String>) -> Scene {
    let whole = format!("{}{unit}", figure(count));
    // A number and its unit stand on one line: the space between them breaks nothing.
    let (size, _) = fit(&whole.replace(' ', "\u{a0}"), ACROSS, DOWN, MOST, LEAST);
    Scene { eyebrow: upper(eyebrow), figure: whole, count: Some(count), unit: unit.into(), size, headline: upper(headline), note, photos: photo.into_iter().collect(), ..Default::default() }
}

/// A scene whose figure is a word or a name, in capitals, broken where its size breaks it.
pub fn worded(eyebrow: &str, word: &str, headline: &str, note: String, photo: Option<String>) -> Scene {
    let (size, broken) = fit(&upper(word), ACROSS, DOWN, MOST, LEAST);
    Scene { kind: Kind::Hero, eyebrow: upper(eyebrow), figure: broken, size, headline: upper(headline), note, photos: photo.into_iter().collect(), ..Default::default() }
}

/// The year's photos in the order scenes are given them: its gold, the latest first, then frames spread
/// over the year; one already shown comes back only once every other has been.
pub struct Pool {
    photos: Vec<String>,
    given: Vec<String>,
}

impl Pool {
    pub fn of(year: &Year) -> Pool {
        let paths = &year.frames.paths;
        let stride = paths.len().div_ceil(SPREAD).max(1);
        let mut photos = year.frames.gold.clone();
        for (path, _) in paths.iter().step_by(stride).rev() {
            if !photos.contains(path) {
                photos.push(path.clone());
            }
        }
        // A year told by its gestures alone still has the photo they were made on.
        if let Some((path, _)) = year.darkroom.most_edited.as_ref().filter(|_| photos.is_empty()) {
            photos.push(path.clone());
        }
        Pool { photos, given: Vec::new() }
    }

    /// The next photo not shown yet; once all were, the one shown longest ago.
    pub fn take(&mut self) -> Option<String> {
        let next = self.photos.iter().find(|p| !self.given.contains(p)).or_else(|| self.given.iter().find(|p| self.photos.contains(p))).cloned()?;
        self.given.retain(|p| *p != next);
        self.given.push(next.clone());
        Some(next)
    }

    /// The photo a scene names as its own, else the pool's next; a photo already shown gives way to one
    /// not shown yet, so no two scenes repeat a photo while the year has others.
    pub fn claim(&mut self, own: Option<&String>) -> Option<String> {
        let unseen = |p: &String| !self.given.contains(p);
        match own {
            Some(own) if unseen(own) || self.photos.iter().all(|p| !unseen(p)) => {
                self.mark(std::slice::from_ref(own));
                Some(own.clone())
            }
            _ => self.take(),
        }
    }

    /// Several photos for one scene, each the pool's next: the year's best first, then those shown
    /// longest ago.
    pub fn deal(&mut self, count: usize) -> Vec<String> {
        let mut dealt: Vec<String> = Vec::new();
        while dealt.len() < count.min(self.photos.len()) {
            let Some(next) = self.take() else { break };
            dealt.push(next);
        }
        dealt
    }

    /// The first `count` of `photos` no scene has shown yet, then those shown longest ago; all counted
    /// as shown.
    pub fn fresh(&mut self, photos: &[String], count: usize) -> Vec<String> {
        let age = |p: &String| self.given.iter().position(|g| g == p).map_or(0, |at| at + 1);
        let mut order: Vec<&String> = photos.iter().collect();
        order.sort_by_key(|p| age(p));
        let dealt: Vec<String> = order.into_iter().take(count).cloned().collect();
        self.mark(&dealt);
        dealt
    }

    /// Photos a scene shows of its own, counted as shown.
    pub fn mark(&mut self, photos: &[String]) {
        for photo in photos {
            self.given.retain(|p| p != photo);
            self.given.push(photo.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_library::wrapped::{Darkroom, Frames, Palette};

    fn year(gold: &[&str], frames: &[&str]) -> Year {
        let frames = Frames { count: frames.len(), gold: gold.iter().map(|s| s.to_string()).collect(), paths: frames.iter().map(|s| (s.to_string(), String::new())).collect(), ..Default::default() };
        Year::new(2026, Darkroom::default(), frames, Palette::default(), &[])
    }

    #[test]
    fn every_scene_gets_a_photo_and_none_comes_back_before_the_others_were_shown() {
        let mut pool = Pool::of(&year(&["/g2", "/g1"], &["/a", "/g1", "/b", "/g2"]));
        assert_eq!(pool.claim(Some(&"/own".to_string())).as_deref(), Some("/own"), "a scene's own photo is its own");
        assert_eq!(pool.deal(3), ["/g2", "/g1", "/b"], "the gold first, then the year from its end");
        // A scene naming a photo already shown takes one not shown yet.
        assert_eq!(pool.claim(Some(&"/g1".to_string())).as_deref(), Some("/a"));
        // All were shown: the one shown longest ago comes back, never the last.
        assert_eq!(pool.take().as_deref(), Some("/g2"));
        assert_eq!(pool.claim(None).as_deref(), Some("/g1"));
        assert_eq!(pool.deal(2), ["/b", "/a"], "a second hand of photos deals those shown longest ago");
        // A wall of photos takes those not shown yet first, in their order, then the one shown longest ago.
        let mut wall = Pool::of(&year(&["/g1", "/g2", "/g3"], &["/g1", "/g2", "/g3"]));
        assert_eq!(wall.take().as_deref(), Some("/g1"));
        assert_eq!(wall.fresh(&["/g1".into(), "/g2".into(), "/g3".into()], 2), ["/g2", "/g3"], "twin: /g1, shown, is not the wall's first");
        assert_eq!(wall.fresh(&["/g1".into(), "/g2".into()], 1), ["/g1"]);
        // Once every photo was shown, a scene's own is its own again.
        assert_eq!(pool.claim(Some(&"/b".to_string())).as_deref(), Some("/b"));
        // A photo a scene claimed is not handed to the next scene.
        let mut pool = Pool::of(&year(&[], &["/a", "/b"]));
        assert_eq!(pool.claim(Some(&"/b".to_string())).as_deref(), Some("/b"));
        assert_eq!(pool.take().as_deref(), Some("/a"), "twin: /b, the pool's first, was claimed");
        // A year with no photo gives none.
        assert_eq!(Pool::of(&year(&[], &[])).take(), None);
    }

    #[test]
    fn a_figure_is_set_as_large_as_its_frame_holds() {
        let rolls = counted_scene("Through the gate", 42, "", "rolls", String::new(), None);
        assert_eq!((rolls.figure.as_str(), rolls.count, rolls.size, rolls.eyebrow.as_str(), rolls.headline.as_str()), ("42", Some(42), MOST, "THROUGH THE GATE", "ROLLS"));
        let film = worded("Your film of the year", "Kodak Gold", "", String::new(), Some("/a".into()));
        assert_eq!((film.figure.as_str(), film.count, film.photos.as_slice()), ("KODAK\nGOLD", None, ["/a".to_string()].as_slice()));
        assert!(film.size < rolls.size && film.size >= LEAST);
        // Twin: a year written as a word is not counted, nor grouped as a number is.
        assert_eq!((worded("", "2026", "", String::new(), None).figure.as_str(), counted_scene("", 2026, "", "", String::new(), None).figure.as_str()), ("2026", "2,026"));
    }

    /// Every line of a staged figure inside its box.
    fn holds(scene: &Scene) -> bool {
        use crate::wrapped::fit::{LEADING, width};
        let lines: Vec<&str> = scene.figure.lines().collect();
        let wide = lines.iter().all(|line| width(line) * scene.size <= ACROSS + 0.01);
        wide && lines.len() as f32 * scene.size * LEADING <= DOWN + 0.01
    }

    #[test]
    fn every_personality_film_and_number_holds_in_its_frame_in_three_languages() {
        let personas = [
            "The Night Developer", "The Silver Soul", "The Projectionist", "The Time Traveller", "The Loyalist", "The Explorer",
            "The Dust Hunter", "The Curator", "The Printer", "The Purist", "The Regular", "The Film Photographer",
        ];
        let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
        let words = ["See you next year", "Frame 0", "Grain", "White balance"];
        for language in oni_i18n::Language::ALL {
            for english in personas.iter().chain(&months).chain(&words) {
                let word = oni_i18n::translated(language, english, None).unwrap_or(english);
                assert!(holds(&worded("", word, "", String::new(), None)), "{language:?}: {word}");
            }
        }
        // Every film of the catalogue, the longest camera and preset names a year may hold.
        let films: Vec<String> = oni_settings::film::stocks().iter().map(|k| k.display_name.to_string()).collect();
        assert!(films.len() > 50);
        let names = ["Olympus OM-D E-M1 Mark III", "Hasselblad 500C/M", "Portra_warm_v2_final_export_definitive", "Donaudampfschifffahrtsgesellschaft"];
        for name in films.iter().map(String::as_str).chain(names) {
            assert!(holds(&worded("", name, "", String::new(), None)), "{name}");
        }
        for count in [7, 42, 1_732, 99_999, 9_999_999] {
            assert!(holds(&counted_scene("", count, " H", "", String::new(), None)), "{count}");
        }
        // Twin: the French explorer at the least size a figure was once held to runs out of its frame.
        let explorer = oni_i18n::translated(oni_i18n::Language::French, "The Explorer", None).unwrap();
        assert!(crate::wrapped::fit::width(&upper(explorer)) * LEAST > ACROSS, "{explorer}");
        assert!(worded("", explorer, "", String::new(), None).size < LEAST);
    }
}
