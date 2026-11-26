//! `Persona`: the character a year adds up to, each trait measured against its own bar and the one
//! furthest past its bar named; the measure behind it is kept, so the card can show its reason.

use super::{Darkroom, Frames};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Trait {
    /// Works when the house sleeps: a share of the gestures made between 22:00 and 05:00.
    NightDeveloper,
    /// A year mostly in black and white.
    SilverSoul,
    /// A year mostly on slides.
    Projectionist,
    /// Digital photos shot again on film.
    TimeTraveller,
    /// One film above all others.
    Loyalist,
    /// Many films tried.
    Explorer,
    /// Dust healed stroke after stroke: ten specks a frame of the year, on average.
    DustHunter,
    /// Medals given with care.
    Curator,
    /// Many gestures a photo.
    Tinkerer,
    /// Few gestures a photo.
    Purist,
    /// Days in a row at the desk.
    Marathoner,
    /// No trait past its bar: a year on film, plainly.
    #[default]
    FilmPhotographer,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Persona {
    pub kind: Trait,
    /// What the trait was measured on: a share from 0 to 1, or a count, as its card words it.
    pub measure: f64,
    /// The other traits past their bars, the strongest first.
    pub also: Vec<Trait>,
}

/// A share is read on at least this many frames or gestures, so three photos make no character.
pub const ENOUGH: usize = 12;

const NIGHT_SHARE: f64 = 0.35;
const KIND_SHARE: f64 = 0.5;
const LOYAL_SHARE: f64 = 0.6;
const FILMS_TRIED: f64 = 5.0;
const DUST_A_FRAME: f64 = 10.0;
const MEDAL_SHARE: f64 = 0.15;
const GESTURES_MANY: f64 = 20.0;
const GESTURES_FEW: f64 = 4.0;
const STREAK_DAYS: f64 = 7.0;

impl Persona {
    pub fn of(darkroom: &Darkroom, frames: &Frames) -> Persona {
        let share = |part: usize| if frames.count >= ENOUGH { part as f64 / frames.count as f64 } else { 0.0 };
        let gestures: usize = darkroom.by_hour.iter().sum();
        let a_photo = if darkroom.photos >= ENOUGH { darkroom.gestures as f64 / darkroom.photos as f64 } else { f64::NAN };
        let named: usize = frames.stocks.iter().map(|(_, n)| n).sum();
        let medals = frames.gold.len() + frames.silver + frames.bronze;
        // Each trait: what it measures, and how far past its bar that stands (1 is the bar).
        let measured = [
            (Trait::NightDeveloper, darkroom.night_share(), if gestures >= ENOUGH { darkroom.night_share() / NIGHT_SHARE } else { 0.0 }),
            (Trait::SilverSoul, share(frames.monochromes), share(frames.monochromes) / KIND_SHARE),
            (Trait::Projectionist, share(frames.slides), share(frames.slides) / KIND_SHARE),
            (Trait::TimeTraveller, share(frames.digitals), share(frames.digitals) / KIND_SHARE),
            (Trait::Loyalist, frames.top_stock_share(), if named >= ENOUGH && frames.stocks.len() > 1 { frames.top_stock_share() / LOYAL_SHARE } else { 0.0 }),
            (Trait::Explorer, frames.stocks.len() as f64, frames.stocks.len() as f64 / FILMS_TRIED),
            (Trait::DustHunter, frames.dust as f64, if frames.count >= ENOUGH { frames.dust as f64 / frames.count as f64 / DUST_A_FRAME } else { 0.0 }),
            (Trait::Curator, medals as f64, if medals >= 3 { share(medals) / MEDAL_SHARE } else { 0.0 }),
            (Trait::Tinkerer, a_photo, if a_photo.is_nan() { 0.0 } else { a_photo / GESTURES_MANY }),
            (Trait::Purist, a_photo, if a_photo.is_nan() || a_photo <= 0.0 { 0.0 } else { (GESTURES_FEW / a_photo).min(3.0) }),
            (Trait::Marathoner, darkroom.streak as f64, darkroom.streak as f64 / STREAK_DAYS),
        ];
        let mut past: Vec<(Trait, f64, f64)> = measured.into_iter().filter(|(_, _, strength)| *strength >= 1.0).collect();
        // A stable sort: on a tie the trait listed first stands.
        past.sort_by(|a, b| b.2.total_cmp(&a.2));
        let Some((kind, measure, _)) = past.first().copied() else { return Persona::default() };
        Persona { kind, measure, also: past.iter().skip(1).take(2).map(|(t, _, _)| *t).collect() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(count: usize) -> Frames {
        Frames { count, negatives: count, ..Default::default() }
    }

    #[test]
    fn the_trait_furthest_past_its_bar_names_the_year_and_keeps_its_measure() {
        let mut by_hour = [0; 24];
        (by_hour[23], by_hour[14]) = (30, 30);
        let darkroom = Darkroom { by_hour, photos: 20, gestures: 200, streak: 8, ..Default::default() };
        let stocks = vec![("kodak-portra400".to_string(), 30), ("ilford-hp5".to_string(), 10)];
        let year = Frames { stocks, ..frames(40) };
        let persona = Persona::of(&darkroom, &year);
        // Night: 0.5 / 0.35 = 1.43; loyal: 0.75 / 0.6 = 1.25; streak: 8 / 7 = 1.14; ten gestures a photo is neither many nor few.
        assert_eq!(persona.kind, Trait::NightDeveloper);
        assert!((persona.measure - 0.5).abs() < 1e-9);
        assert_eq!(persona.also, [Trait::Loyalist, Trait::Marathoner]);
        // Twin: the same year worked by day is the loyalist's.
        let mut day = [0; 24];
        day[14] = 60;
        let by_day = Persona::of(&Darkroom { by_hour: day, ..darkroom.clone() }, &year);
        assert_eq!((by_day.kind, by_day.also.as_slice()), (Trait::Loyalist, [Trait::Marathoner].as_slice()));
    }

    #[test]
    fn each_kind_of_year_has_its_character() {
        let of = |f: Frames| Persona::of(&Darkroom::default(), &f).kind;
        assert_eq!(of(Frames { monochromes: 30, ..frames(40) }), Trait::SilverSoul);
        assert_eq!(of(Frames { slides: 30, ..frames(40) }), Trait::Projectionist);
        assert_eq!(of(Frames { digitals: 30, ..frames(40) }), Trait::TimeTraveller);
        assert_eq!(of(Frames { stocks: (0..6).map(|i| (format!("s{i}"), 2)).collect(), ..frames(12) }), Trait::Explorer);
        assert_eq!(of(Frames { dust: 600, dusted: 30, ..frames(40) }), Trait::DustHunter);
        assert_eq!(of(Frames { gold: vec!["/a".into(); 6], silver: 4, ..frames(40) }), Trait::Curator);
        let hands = |gestures| Persona::of(&Darkroom { photos: 20, gestures, ..Default::default() }, &frames(20)).kind;
        assert_eq!((hands(500), hands(40)), (Trait::Tinkerer, Trait::Purist));
        // Twin: ten gestures a photo is neither, and a year with no trait past its bar is plainly on film.
        assert_eq!(hands(200), Trait::FilmPhotographer);
        assert_eq!(of(frames(40)), Trait::FilmPhotographer);
    }

    #[test]
    fn three_photos_make_no_character() {
        let few = Frames { count: 3, monochromes: 3, gold: vec!["/a".into(); 3], dust: 90, dusted: 2, stocks: vec![("hp5".into(), 3)], ..Default::default() };
        let mut by_hour = [0; 24];
        by_hour[23] = 5;
        let darkroom = Darkroom { by_hour, photos: 3, gestures: 300, ..Default::default() };
        assert_eq!(Persona::of(&darkroom, &few), Persona::default());
        // Twin: the same shares on enough frames do.
        let many = Frames { count: 30, monochromes: 30, ..Default::default() };
        assert_eq!(Persona::of(&Darkroom::default(), &many).kind, Trait::SilverSoul);
    }
}
