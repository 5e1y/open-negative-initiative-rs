//! The welcome's films: the app at work, filmed by `oni-demos` once per language as it shows words,
//! each held under its ceiling so the six stay a small part of the app.

use oni_i18n::Language;
use std::io::Cursor;

/// Every film's size in pixels: the 560 × 502 stage at two pixels a point.
pub const SIZE: (u32, u32) = (1120, 1004);
/// The films together, two in three languages.
pub const TOTAL: usize = 6 * 1024 * 1024;

/// The welcome's films: the app's own, in its language; a check hands its own.
pub type Films = Box<dyn Fn(Film) -> Option<&'static [u8]>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Film {
    Interface,
    Dedust,
}

/// Each film shipped, in each language, as `resources/welcome/<film>[.<language>].gif` (`demos::file`).
const SHIPPED: &[(Film, Language, &[u8])] = &[];

impl Film {
    pub const ALL: [Film; 2] = [Film::Interface, Film::Dedust];

    /// The film a page of the welcome shows; the cover and the last page show none.
    pub fn of_page(page: i32) -> Option<Film> {
        match page {
            1 => Some(Film::Interface),
            2 => Some(Film::Dedust),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Film::Interface => "interface",
            Film::Dedust => "dedust",
        }
    }

    /// Its file's ceiling: the interface's carries the former interface before the new one.
    pub fn ceiling(self) -> usize {
        match self {
            Film::Interface => 1200 * 1024,
            Film::Dedust => 900 * 1024,
        }
    }
}

/// The film in `language`; none until it is filmed.
pub fn clip(film: Film, language: Language) -> Option<&'static [u8]> {
    SHIPPED.iter().find(|(f, l, _)| (*f, *l) == (film, language)).map(|(_, _, bytes)| *bytes)
}

/// Whether a file may ship as `film`: within its ceiling, a GIF at the stage's size.
pub fn fits(film: Film, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > film.ceiling() {
        return Err(format!("{} weighs {} bytes, over its ceiling of {}", film.name(), bytes.len(), film.ceiling()));
    }
    let decoder = gif::DecodeOptions::new().read_info(Cursor::new(bytes)).map_err(|e| format!("{} is not a GIF: {e}", film.name()))?;
    let size = (u32::from(decoder.width()), u32::from(decoder.height()));
    if size != SIZE {
        return Err(format!("{} is {} × {}, not {} × {}", film.name(), size.0, size.1, SIZE.0, SIZE.1));
    }
    Ok(())
}

/// The films `found` ships checked whole: each fits, each in every language or in none, the whole
/// within `TOTAL`; their weight.
pub fn weigh(found: impl Fn(Film, Language) -> Option<&'static [u8]>) -> Result<usize, String> {
    let mut total = 0;
    for film in Film::ALL {
        let files: Vec<&[u8]> = Language::ALL.iter().filter_map(|l| found(film, *l)).collect();
        if !files.is_empty() && files.len() != Language::ALL.len() {
            return Err(format!("{} ships in {} of the {} languages", film.name(), files.len(), Language::ALL.len()));
        }
        for bytes in files {
            fits(film, bytes)?;
            total += bytes.len();
        }
    }
    if total > TOTAL {
        return Err(format!("the films weigh {total} bytes, over {TOTAL}"));
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GIF at `width` × `height` padded to `length` bytes: a header is all `fits` reads past the weight.
    fn file(width: u16, height: u16, length: usize) -> &'static [u8] {
        let mut bytes = crate::tutorial::synthetic(width, height, &[0]);
        assert!(bytes.len() <= length);
        bytes.resize(length, 0);
        Box::leak(bytes.into_boxed_slice())
    }

    #[test]
    fn every_shipped_film_fits_and_speaks_every_language() {
        weigh(clip).unwrap();
        assert_eq!((Film::of_page(1), Film::of_page(2)), (Some(Film::Interface), Some(Film::Dedust)));
        assert_eq!((Film::of_page(0), Film::of_page(3), Film::of_page(-1)), (None, None, None), "the cover, the last page, the sheet closed");
    }

    #[test]
    fn a_file_a_byte_over_its_ceiling_or_off_size_fails() {
        for film in Film::ALL {
            assert_eq!(fits(film, file(1120, 1004, film.ceiling())), Ok(()), "{film:?} at its ceiling ships");
            assert!(fits(film, file(1120, 1004, film.ceiling() + 1)).is_err(), "twin: {film:?} a byte over fails");
            assert!(fits(film, file(1119, 1004, film.ceiling() / 2)).is_err(), "twin: {film:?} off the stage's size fails");
        }
    }

    #[test]
    fn a_film_missing_a_language_or_the_six_too_heavy_fail() {
        let all = |film: Film, _: Language| Some(file(1120, 1004, film.ceiling() / 2));
        assert!(weigh(all).is_ok(), "every film in every language, light enough");
        let english = |film: Film, l: Language| (l == Language::English).then(|| file(1120, 1004, film.ceiling() / 2));
        assert!(weigh(english).is_err(), "twin: a film in English alone fails");
        let full = |film: Film, _: Language| Some(file(1120, 1004, film.ceiling()));
        assert!(weigh(full).is_err(), "twin: the six at their ceilings weigh more than {TOTAL}");
    }
}
