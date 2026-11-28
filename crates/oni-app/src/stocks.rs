//! `FilmStockIcon`: each stock's own artwork in its colours, its SVG read from the stocks' files at run time and decoded
//! once per content, at a picker's size or drawn once at a row's; the menus' lists and names, read off the catalogue.

use oni_settings::film::{FilmStockKind, film_stock};
use oni_settings::stocks::{self, Canister};
use slint::Image;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static DECODED: RefCell<HashMap<u64, Image>> = RefCell::new(HashMap::new());
    static ROW_ICONS: RefCell<HashMap<(u64, u32), Image>> = RefCell::new(HashMap::new());
}

/// Every artwork's root opens on this side, which `sized` sets to the pixels wanted.
const ARTWORK_HEAD: &str = r#"<svg width="48" height="48""#;

/// The canister a stock's file names: its own, its line's, or its type's generic stock's where it names none or its
/// SVG is missing.
fn artwork(id: &str) -> Option<Canister> {
    let catalogue = stocks::current();
    let kind = catalogue.film(id)?;
    let own = kind.canister.and_then(|svg| catalogue.canister(svg));
    let generic = || catalogue.film(kind.kind.generic()).and_then(|g| g.canister).and_then(|svg| catalogue.canister(svg));
    own.or_else(generic).cloned()
}

/// The stock's canister, `None` for a stock the catalogue lacks.
pub fn icon(id: &str) -> Option<Image> {
    let art = artwork(id)?;
    DECODED.with(|d| {
        if let Some(image) = d.borrow().get(&art.hash) {
            return Some(image.clone());
        }
        let image = Image::load_from_svg_data(&art.bytes).ok()?;
        d.borrow_mut().insert(art.hash, image.clone());
        Some(image)
    })
}

/// The stock's artwork drawn once at `side` pixels, which every row a virtualised tree makes shares:
/// an SVG source is drawn again, filters included, by each row it lands in.
pub fn row_icon(id: &str, side: u32) -> Option<Image> {
    drawn(&artwork(id)?, side)
}

/// A paper's box drawn once at `side` pixels, as a stock's canister is; none for a paper without one.
pub fn paper_icon(paper: oni_settings::PaperStock, side: u32) -> Option<Image> {
    let catalogue = stocks::current();
    drawn(catalogue.canister(paper.canister()?)?, side)
}

fn drawn(art: &Canister, side: u32) -> Option<Image> {
    ROW_ICONS.with(|cache| {
        if let Some(image) = cache.borrow().get(&(art.hash, side)) {
            return Some(image.clone());
        }
        let pixels = Image::load_from_svg_data(&sized(&art.bytes, side)?).ok()?.to_rgba8()?;
        let image = Image::from_rgba8(pixels);
        cache.borrow_mut().insert((art.hash, side), image.clone());
        Some(image)
    })
}

/// A tree row's stock side in the window's pixels, `RowStyle.slot` at its scale.
pub fn row_side(w: &crate::MainWindow) -> u32 {
    use slint::ComponentHandle;
    (w.global::<crate::DS>().get_control_film_sidebar_stock_icon_size() * w.window().scale_factor()).round().max(1.0) as u32
}

/// The artwork with its root's side set to `side` pixels; `None` for a root of another shape.
fn sized(data: &[u8], side: u32) -> Option<Vec<u8>> {
    let rest = std::str::from_utf8(data).ok()?.strip_prefix(ARTWORK_HEAD)?;
    Some(format!(r#"<svg width="{side}" height="{side}"{rest}"#).into_bytes())
}

/// The stock's name as the screen shows it: a trade name as its file writes it, a generic stock's description
/// translated where the catalogue holds it.
pub fn shown_name(kind: &FilmStockKind) -> &'static str {
    if kind.generic { oni_i18n::tr(kind.display_name) } else { kind.display_name }
}

/// A brand as the screen shows it, the generic stocks' translated.
fn shown_brand(kind: &FilmStockKind) -> &'static str {
    if kind.generic { oni_i18n::tr(kind.brand) } else { kind.brand }
}

/// The shipped generic stocks' words, which their files hold and the screen shows translated.
pub const GENERIC_WORDS: [&str; 7] = [
    oni_i18n::marked("Generic"),
    oni_i18n::marked("Generic Azure"),
    oni_i18n::marked("Generic Black & White"),
    oni_i18n::marked("Generic Colour Negative"),
    oni_i18n::marked("Generic Colour Positive"),
    oni_i18n::marked("Generic Redscale"),
    oni_i18n::marked("Neutral"),
];

/// A key no file defines, as a menu or a hero names it: the key itself, marked missing.
pub fn missing(key: &str) -> String {
    oni_i18n::tr!("{key} (file missing)", key = key)
}

/// What a line, its stock never named, says under its name wherever it is shown; nothing for a precise stock.
pub fn unspecified(kind: &FilmStockKind) -> &'static str {
    if kind.line { oni_i18n::tr("Stock not specified") } else { "" }
}

/// A stock as a hero shows it, `note` under its name; the generic negative's canister and no name for none.
pub fn canister(kind: Option<FilmStockKind>, note: String) -> crate::Canister {
    let artwork = icon(kind.map_or(crate::library::GENERIC_NEGATIVE, |k| k.id));
    crate::Canister {
        name: kind.map_or_else(Default::default, |k| shown_name(&k).into()),
        note: note.into(),
        has_artwork: artwork.is_some(),
        artwork: artwork.unwrap_or_default(),
    }
}

/// A digital photo's film no file defines any more, as its hero shows it: its key marked missing, the generic negative's
/// canister.
pub fn missing_canister(key: &str) -> crate::Canister {
    crate::Canister { name: missing(key).into(), ..canister(None, String::new()) }
}

/// The virtual film a stock of the catalogue is, when a digital photo can be shot on it.
pub fn virtual_stock(id: &str) -> Option<oni_settings::VirtualStock> {
    film_stock(Some(id)).and_then(|k| k.film)
}

/// The brands in the catalogue's order, each with the stocks it offers, as
/// indices into the catalogue; `current`, when a line, stays at its place among the line's stocks.
pub fn by_brand(current: Option<&str>) -> Vec<(&'static str, Vec<usize>)> {
    among(|k| k.offered || (k.line && current == Some(k.id)))
}

/// The stocks a digital photo is offered, by brand as `by_brand` lists them.
pub fn virtual_by_brand() -> Vec<(&'static str, Vec<usize>)> {
    among(|k| k.film.is_some_and(|v| v.entry().offered()))
}

/// The papers by maker, in the catalogue's order, as indices into it.
pub fn papers_by_maker() -> Vec<(&'static str, Vec<usize>)> {
    grouped(stocks::current().papers().iter().map(|p| p.brand).enumerate())
}

fn among(shown: impl Fn(&FilmStockKind) -> bool) -> Vec<(&'static str, Vec<usize>)> {
    grouped(oni_settings::film::stocks().iter().enumerate().filter(|(_, k)| shown(k)).map(|(i, k)| (i, shown_brand(k))))
}

/// Each entry under its brand, the brands in the order they first come.
fn grouped(entries: impl Iterator<Item = (usize, &'static str)>) -> Vec<(&'static str, Vec<usize>)> {
    let mut out: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (i, brand) in entries {
        match out.iter_mut().find(|(b, _)| *b == brand) {
            Some((_, list)) => list.push(i),
            None => out.push((brand, vec![i])),
        }
    }
    out
}

#[cfg(test)]
mod tests;
