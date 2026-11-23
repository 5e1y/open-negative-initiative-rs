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
    let art = artwork(id)?;
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
pub const GENERIC_WORDS: [&str; 6] = [
    oni_i18n::marked("Generic"),
    oni_i18n::marked("Generic Azure"),
    oni_i18n::marked("Generic Black & White"),
    oni_i18n::marked("Generic Colour Negative"),
    oni_i18n::marked("Generic Colour Positive"),
    oni_i18n::marked("Generic Redscale"),
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

/// `FilmStockCatalog.byBrand`: the brands in the catalogue's order, each with the stocks it offers, as
/// indices into the catalogue; `current`, when a line, stays at its place among the line's stocks.
pub fn by_brand(current: Option<&str>) -> Vec<(&'static str, Vec<usize>)> {
    among(|k| k.offered || (k.line && current == Some(k.id)))
}

/// The stocks a digital photo can be shot on, by brand as `by_brand` lists them.
pub fn virtual_by_brand() -> Vec<(&'static str, Vec<usize>)> {
    among(|k| k.film.is_some())
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
mod tests {
    use super::*;

    /// A shipped SVG by its file's name.
    fn shipped_svg(name: &str) -> Option<&'static [u8]> {
        stocks::shipped_files().iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
    }

    fn shipped_svgs() -> impl Iterator<Item = (&'static str, &'static [u8])> {
        stocks::shipped_files().iter().copied().filter(|(n, _)| n.ends_with(".svg"))
    }

    #[test]
    fn every_stock_has_its_artwork_and_the_brands_keep_the_catalogues_order() {
        for kind in oni_settings::film::stocks().iter() {
            assert!(artwork(kind.id).is_some(), "{} has no artwork", kind.id);
        }
        assert!(icon("kodak-portra").is_some_and(|i| i.size().width > 0));
        assert!(icon("no-such-stock").is_none(), "twin: an unknown stock has no artwork");
        let brands: Vec<&str> = by_brand(None).iter().map(|(b, _)| *b).collect();
        assert_eq!(&brands[..3], ["Generic", "AgfaPhoto", "CineStill"]);
        assert_eq!(by_brand(None).iter().map(|(_, l)| l.len()).sum::<usize>(), oni_settings::film::offered_stocks().len());
    }

    #[test]
    fn the_picker_lists_no_foma_401_yet_its_artwork_still_draws() {
        let listed: Vec<&str> = by_brand(None).into_iter().flat_map(|(_, l)| l).map(|i| oni_settings::film::stocks()[i].id).collect();
        assert!(!listed.contains(&"foma-fomapan401"), "Foma 401 is not offered");
        assert!(row_icon("foma-fomapan401", 44).is_some() && icon("foma-fomapan401").is_some(), "a roll on it still draws its canister");
        // Twin: Foma 400 stays among Foma's stocks.
        let foma = by_brand(None).into_iter().find(|(b, _)| *b == "Foma").map(|(_, l)| l).unwrap_or_default();
        let names: Vec<&str> = foma.iter().map(|&i| oni_settings::film::stocks()[i].display_name).collect();
        assert_eq!(names, ["Foma Cine Ortho 400", "Foma Fomapan 100", "Foma Fomapan 200", "Foma Fomapan 400", "Foma Fomapan Cine 100", "Foma Fomapan Cine 400", "Foma Ortho 400", "Foma Retropan 320"]);
    }

    /// The sheet lists a line's stocks in its place, and a roll still on the line finds it shown among them.
    #[test]
    fn the_picker_lists_a_lines_stocks_and_shows_the_line_a_roll_is_on() {
        let names = |current: Option<&str>| -> Vec<&str> {
            let kodak = by_brand(current).into_iter().find(|(b, _)| *b == "Kodak").map(|(_, l)| l).unwrap_or_default();
            kodak.iter().map(|&i| oni_settings::film::stocks()[i].display_name).collect()
        };
        let listed = names(None);
        assert!(["Kodak Portra 160", "Kodak Portra 400", "Kodak Portra 800", "Kodak Gold 200", "Kodak Vision3 250D"].iter().all(|n| listed.contains(n)), "{listed:?}");
        assert!(!listed.contains(&"Kodak Portra"), "the line is no longer offered");
        let on_line = names(Some("kodak-portra"));
        let at = on_line.iter().position(|n| *n == "Kodak Portra").expect("the roll's line is shown");
        assert_eq!(on_line[at + 1], "Kodak Portra 160", "at its catalogue place, before its stocks");
        assert_eq!(on_line.len(), listed.len() + 1);
        // Twin: a current stock already offered is listed once, and Foma 401, withdrawn, never.
        assert_eq!(names(Some("kodak-portra400")), listed);
        assert!(!by_brand(Some("foma-fomapan401")).into_iter().flat_map(|(_, l)| l).any(|i| oni_settings::film::stocks()[i].id == "foma-fomapan401"));
        let stock = oni_settings::film::film_stock(Some("kodak-portra400")).unwrap();
        assert!(icon(stock.id).is_some() && stock.canister == Some("kodak-portra.svg"), "a stock draws its line's canister");
    }

    /// A stock whose own artwork lies in the folder draws it: a dropped SVG asks its file to name it.
    #[test]
    fn a_stock_with_its_own_artwork_draws_it() {
        for kind in oni_settings::film::stocks().iter() {
            let svg = format!("{}.svg", kind.id);
            assert!(shipped_svg(&svg).is_none() || kind.canister == Some(svg.as_str()), "{svg} is drawn: its file names it");
        }
        // Twin: a stock still undrawn stands on its type's generic canister.
        let ektar = oni_settings::film::film_stock(Some("kodak-ektar100")).unwrap();
        assert!(shipped_svg("kodak-ektar100.svg").is_none() && ektar.canister.is_none());
        assert_eq!(artwork(ektar.id).map(|a| a.hash), artwork(ektar.kind.generic()).map(|a| a.hash));
    }

    /// A stock drawn with its line's or its kind's generic canister draws it by its own key, at any size.
    #[test]
    fn a_stock_draws_the_canister_its_catalogue_line_names() {
        let pixels = |id: &str| row_icon(id, 44).and_then(|i| i.to_rgba8()).map(|p| p.as_slice().to_vec());
        assert!(pixels("kodak-portra400").is_some() && pixels("kodak-portra400") == pixels("kodak-portra"), "Portra 400 draws the Portra canister");
        assert!(pixels("kodak-ektar100") == pixels("generic-color-negative") && icon("kodak-ektar100").is_some(), "an undrawn negative draws its kind's");
        // Twin: a stock of its own draws its own, never its brand's line.
        assert_ne!(pixels("kodak-gold200"), pixels("kodak-portra"));
    }

    /// Every paper of the catalogue listed once under its file's maker, each maker once.
    #[test]
    fn the_papers_are_listed_once_each_under_its_maker() {
        let listed = papers_by_maker();
        let mut all: Vec<usize> = listed.iter().flat_map(|(_, l)| l.iter().copied()).collect();
        all.sort();
        let papers = oni_settings::PaperStock::all();
        assert_eq!(all, (0..papers.len()).collect::<Vec<_>>());
        for (maker, listed) in &listed {
            assert!(listed.iter().all(|&i| papers[i].brand() == *maker && papers[i].name().starts_with(&format!("{maker} "))), "{maker}");
        }
        let makers: Vec<&str> = listed.iter().map(|(m, _)| *m).collect();
        assert!(makers.contains(&"Kodak") && makers.contains(&"Fujifilm"), "{makers:?}");
        // Twin: grouped by a word other than the maker's, the papers would part into more lines than makers.
        let words = grouped(papers.iter().map(|p| p.name().split(' ').nth(1).unwrap_or_default()).enumerate());
        assert!(words.len() > makers.len());
        assert_eq!(makers.len(), makers.iter().collect::<std::collections::BTreeSet<_>>().len(), "each maker once");
    }

    /// A digital photo's sheet lists the stocks a virtual film stands for, every one of them, and no other.
    #[test]
    fn the_virtual_films_are_listed_alone() {
        let listed: Vec<&str> = virtual_by_brand().into_iter().flat_map(|(_, l)| l).map(|i| oni_settings::film::stocks()[i].id).collect();
        let virtual_ids: Vec<&str> = oni_settings::VirtualStock::all().iter().map(|v| v.catalogue_id()).collect();
        assert_eq!(listed.len(), virtual_ids.len());
        assert!(virtual_ids.iter().all(|id| listed.contains(id) && virtual_stock(id).is_some()), "{listed:?}");
        // Twin: a roll's stock no virtual film stands for is neither listed nor taken.
        assert!(!listed.contains(&"agfaphoto-apx") && virtual_stock("agfaphoto-apx").is_none());
        assert!(by_brand(None).into_iter().flat_map(|(_, l)| l).count() > listed.len());
    }

    #[test]
    fn a_row_icon_is_drawn_once_at_the_rows_side() {
        for kind in oni_settings::film::stocks().iter() {
            let image = row_icon(kind.id, 44).unwrap_or_else(|| panic!("{} draws at a row's side", kind.id));
            assert_eq!((image.size().width, image.size().height), (44, 44), "{}", kind.id);
        }
        let portra = row_icon("kodak-portra", 44).unwrap().to_rgba8().unwrap();
        assert!(portra.as_slice().iter().any(|p| p.a > 0), "the artwork's pixels are drawn");
        // Twin: another side is drawn apart, another stock draws other pixels, an unknown one none.
        assert_eq!(row_icon("kodak-portra", 22).unwrap().size().width, 22);
        assert_ne!(row_icon("ilford-hp5", 44).unwrap().to_rgba8().unwrap().as_slice(), portra.as_slice());
        assert!(row_icon("no-such-stock", 44).is_none());
        let portra = artwork("kodak-portra").unwrap().hash;
        ROW_ICONS.with(|c| assert!(c.borrow().contains_key(&(portra, 44)), "kept for the next row"));
    }

    /// FNV-1a over an artwork's bytes.
    fn fnv(data: &[u8]) -> u64 {
        data.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
    }

    /// The owner's own artworks, as 1.5.6 shipped them and the site shows them: a stock's canister is
    /// never redrawn here; a new one comes from the owner, dropped in the folder under its stock's key.
    const ORIGINAL: [(&str, u64); 41] = [
        ("generic-azure", 0xe5b9ddefcbbbf16e),
        ("generic-black-and-white", 0x86243de8f26df732),
        ("generic-color-negative", 0xf6f28336b3522056),
        ("generic-color-positive", 0x5fd6fff2118caf2f),
        ("generic-redscale", 0xa9ae41ca9f053c77),
        ("agfaphoto-apx", 0x740fd82587c10707),
        ("cinestill-400d", 0x38465f5f75bf98a7),
        ("cinestill-50d", 0x9101619774a6367b),
        ("cinestill-800t", 0xc7c9a5b325390a82),
        ("foma-fomapan200", 0xb73440c5cf23c91a),
        ("foma-fomapan400", 0xa00b38c7944cb65a),
        ("foma-fomapan401", 0x8a7966dacf724681),
        ("fujifilm-acros", 0x13c15dd1df6b0683),
        ("fujifilm-fujicolor", 0x34f5e3f9cff589e8),
        ("fujifilm-fujicolor-pro", 0x28ead441b2e2700d),
        ("fujifilm-provia100", 0x9a8e6086328b8a0b),
        ("fujifilm-superia", 0x9d91d655c41a47d6),
        ("fujifilm-velvia100", 0x09fe65b2450bd3f5),
        ("fujifilm-velvia50", 0x4955efb137ae8308),
        ("harman-phoenix", 0xf1e9044ff46a9081),
        ("harman-phoenixII", 0x084f04319fa97c6a),
        ("harman-red", 0xf79fd2af7ee569c0),
        ("ilford-delta100", 0xd9e9a2ddc24b2e43),
        ("ilford-delta3200", 0x0f5f3013545801d7),
        ("ilford-delta400", 0xa710f3c955ca8430),
        ("ilford-fp4", 0x3130019319f259de),
        ("ilford-hp5", 0x24cc54097b67dd73),
        ("ilford-ortho", 0x7ea35ce2af4e399e),
        ("ilford-panf", 0xbec50c93f7a944c8),
        ("ilford-sfx", 0xd2217db2be5d78ca),
        ("ilford-xp2", 0xfabb2c8aee9f158c),
        ("kodak-400tx", 0x208bafdb52ae11f8),
        ("kodak-colorplus", 0xb059b10a1d19e37b),
        ("kodak-ektachrome", 0xc97a6203db1764ca),
        ("kodak-gold", 0x913b1003f34ae2e0),
        ("kodak-portra", 0xa1a54c69338816d3),
        ("kodak-ultramax", 0x3588e36a6777b053),
        ("lomography-lomo-turquoise", 0xdc1ebf9910c73708),
        ("lucky-color200", 0x0915466f940ab916),
        ("mira-color400", 0x607a0d4bf47a937e),
        ("mira-color800", 0xff728fefb330f751),
    ];

    #[test]
    fn every_canister_is_the_owners_own_artwork() {
        for (id, hash) in ORIGINAL {
            assert_eq!(shipped_svg(&format!("{id}.svg")).map(fnv), Some(hash), "{id}: not the owner's artwork");
        }
        for (name, _) in shipped_svgs() {
            let named = oni_settings::film::film_stock(Some(name.trim_end_matches(".svg"))).is_some();
            assert!(named, "{name} names no stock of the catalogue: its file takes the stock's key");
        }
        assert_eq!(shipped_svgs().count(), ORIGINAL.len());
        // Twin: one byte changed in an artwork is seen.
        let mut touched = shipped_svg("generic-azure.svg").unwrap().to_vec();
        touched[60] ^= 1;
        assert_ne!(fnv(&touched), ORIGINAL[0].1);
    }
}
