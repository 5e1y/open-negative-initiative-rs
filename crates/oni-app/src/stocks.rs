//! `FilmStockIcon`: each stock's own artwork in its colours, decoded once from the SVG shipped with
//! the app, at a picker's size or drawn once at a row's.

use slint::Image;
use std::cell::RefCell;
use std::collections::HashMap;

const ARTWORK: [(&str, &[u8]); 41] = [
    ("generic-azure", include_bytes!("../../oni-ui/ui/stocks/generic-azure.svg")),
    ("generic-black-and-white", include_bytes!("../../oni-ui/ui/stocks/generic-black-and-white.svg")),
    ("generic-color-negative", include_bytes!("../../oni-ui/ui/stocks/generic-color-negative.svg")),
    ("generic-color-positive", include_bytes!("../../oni-ui/ui/stocks/generic-color-positive.svg")),
    ("generic-redscale", include_bytes!("../../oni-ui/ui/stocks/generic-redscale.svg")),
    ("agfaphoto-apx", include_bytes!("../../oni-ui/ui/stocks/agfaphoto-apx.svg")),
    ("cinestill-400d", include_bytes!("../../oni-ui/ui/stocks/cinestill-400d.svg")),
    ("cinestill-50d", include_bytes!("../../oni-ui/ui/stocks/cinestill-50d.svg")),
    ("cinestill-800t", include_bytes!("../../oni-ui/ui/stocks/cinestill-800t.svg")),
    ("foma-fomapan200", include_bytes!("../../oni-ui/ui/stocks/foma-fomapan200.svg")),
    ("foma-fomapan400", include_bytes!("../../oni-ui/ui/stocks/foma-fomapan400.svg")),
    ("foma-fomapan401", include_bytes!("../../oni-ui/ui/stocks/foma-fomapan401.svg")),
    ("fujifilm-acros", include_bytes!("../../oni-ui/ui/stocks/fujifilm-acros.svg")),
    ("fujifilm-fujicolor", include_bytes!("../../oni-ui/ui/stocks/fujifilm-fujicolor.svg")),
    ("fujifilm-fujicolor-pro", include_bytes!("../../oni-ui/ui/stocks/fujifilm-fujicolor-pro.svg")),
    ("fujifilm-provia100", include_bytes!("../../oni-ui/ui/stocks/fujifilm-provia100.svg")),
    ("fujifilm-superia", include_bytes!("../../oni-ui/ui/stocks/fujifilm-superia.svg")),
    ("fujifilm-velvia100", include_bytes!("../../oni-ui/ui/stocks/fujifilm-velvia100.svg")),
    ("fujifilm-velvia50", include_bytes!("../../oni-ui/ui/stocks/fujifilm-velvia50.svg")),
    ("harman-phoenix", include_bytes!("../../oni-ui/ui/stocks/harman-phoenix.svg")),
    ("harman-phoenixII", include_bytes!("../../oni-ui/ui/stocks/harman-phoenixII.svg")),
    ("harman-red", include_bytes!("../../oni-ui/ui/stocks/harman-red.svg")),
    ("ilford-delta100", include_bytes!("../../oni-ui/ui/stocks/ilford-delta100.svg")),
    ("ilford-delta3200", include_bytes!("../../oni-ui/ui/stocks/ilford-delta3200.svg")),
    ("ilford-delta400", include_bytes!("../../oni-ui/ui/stocks/ilford-delta400.svg")),
    ("ilford-fp4", include_bytes!("../../oni-ui/ui/stocks/ilford-fp4.svg")),
    ("ilford-hp5", include_bytes!("../../oni-ui/ui/stocks/ilford-hp5.svg")),
    ("ilford-ortho", include_bytes!("../../oni-ui/ui/stocks/ilford-ortho.svg")),
    ("ilford-panf", include_bytes!("../../oni-ui/ui/stocks/ilford-panf.svg")),
    ("ilford-sfx", include_bytes!("../../oni-ui/ui/stocks/ilford-sfx.svg")),
    ("ilford-xp2", include_bytes!("../../oni-ui/ui/stocks/ilford-xp2.svg")),
    ("kodak-400tx", include_bytes!("../../oni-ui/ui/stocks/kodak-400tx.svg")),
    ("kodak-colorplus", include_bytes!("../../oni-ui/ui/stocks/kodak-colorplus.svg")),
    ("kodak-ektachrome", include_bytes!("../../oni-ui/ui/stocks/kodak-ektachrome.svg")),
    ("kodak-gold", include_bytes!("../../oni-ui/ui/stocks/kodak-gold.svg")),
    ("kodak-portra", include_bytes!("../../oni-ui/ui/stocks/kodak-portra.svg")),
    ("kodak-ultramax", include_bytes!("../../oni-ui/ui/stocks/kodak-ultramax.svg")),
    ("lomography-lomo-turquoise", include_bytes!("../../oni-ui/ui/stocks/lomography-lomo-turquoise.svg")),
    ("lucky-color200", include_bytes!("../../oni-ui/ui/stocks/lucky-color200.svg")),
    ("mira-color400", include_bytes!("../../oni-ui/ui/stocks/mira-color400.svg")),
    ("mira-color800", include_bytes!("../../oni-ui/ui/stocks/mira-color800.svg")),
];

thread_local! {
    static DECODED: RefCell<HashMap<&'static str, Image>> = RefCell::new(HashMap::new());
    static ROW_ICONS: RefCell<HashMap<(&'static str, u32), Image>> = RefCell::new(HashMap::new());
}

/// Every artwork's root opens on this side, which `sized` sets to the pixels wanted.
const ARTWORK_HEAD: &str = r#"<svg width="48" height="48""#;

/// The stock's artwork, `None` when it has none.
pub fn icon(id: &str) -> Option<Image> {
    let (key, data) = ARTWORK.iter().find(|(k, _)| *k == id)?;
    DECODED.with(|d| {
        if let Some(image) = d.borrow().get(key) {
            return Some(image.clone());
        }
        let image = Image::load_from_svg_data(data).ok()?;
        d.borrow_mut().insert(key, image.clone());
        Some(image)
    })
}

/// The stock's artwork drawn once at `side` pixels, which every row a virtualised tree makes shares:
/// an SVG source is drawn again, filters included, by each row it lands in.
pub fn row_icon(id: &str, side: u32) -> Option<Image> {
    let (key, data) = ARTWORK.iter().find(|(k, _)| *k == id)?;
    ROW_ICONS.with(|cache| {
        if let Some(image) = cache.borrow().get(&(*key, side)) {
            return Some(image.clone());
        }
        let pixels = Image::load_from_svg_data(&sized(data, side)?).ok()?.to_rgba8()?;
        let image = Image::from_rgba8(pixels);
        cache.borrow_mut().insert((*key, side), image.clone());
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

/// `FilmStockCatalog.byBrand`: the brands in the catalogue's order, each with the stocks it offers, as
/// indices into the catalogue.
pub fn by_brand() -> Vec<(&'static str, Vec<usize>)> {
    let mut out: Vec<(&'static str, Vec<usize>)> = Vec::new();
    for (i, kind) in oni_settings::film::offered_stocks() {
        match out.iter_mut().find(|(b, _)| *b == kind.brand) {
            Some((_, list)) => list.push(i),
            None => out.push((kind.brand, vec![i])),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stock_has_its_artwork_and_the_brands_keep_the_catalogues_order() {
        for kind in oni_settings::film::FILM_STOCKS {
            assert!(ARTWORK.iter().any(|(k, _)| *k == kind.icon_name), "{} has no artwork", kind.id);
        }
        assert!(icon("kodak-portra").is_some_and(|i| i.size().width > 0));
        assert!(icon("no-such-stock").is_none(), "twin: an unknown stock has no artwork");
        let brands: Vec<&str> = by_brand().iter().map(|(b, _)| *b).collect();
        assert_eq!(&brands[..3], ["Generic", "AgfaPhoto", "CineStill"]);
        assert_eq!(by_brand().iter().map(|(_, l)| l.len()).sum::<usize>(), oni_settings::film::offered_stocks().count());
    }

    #[test]
    fn the_picker_lists_no_foma_401_yet_its_artwork_still_draws() {
        let listed: Vec<&str> = by_brand().into_iter().flat_map(|(_, l)| l).map(|i| oni_settings::film::FILM_STOCKS[i].id).collect();
        assert!(!listed.contains(&"foma-fomapan401"), "Foma 401 is not offered");
        assert!(row_icon("foma-fomapan401", 44).is_some() && icon("foma-fomapan401").is_some(), "a roll on it still draws its canister");
        // Twin: Foma 400 stays among Foma's stocks.
        let foma = by_brand().into_iter().find(|(b, _)| *b == "Foma").map(|(_, l)| l).unwrap_or_default();
        let names: Vec<&str> = foma.iter().map(|&i| oni_settings::film::FILM_STOCKS[i].display_name).collect();
        assert_eq!(names, ["Foma Fomapan 200", "Foma Fomapan 400"]);
    }

    #[test]
    fn a_row_icon_is_drawn_once_at_the_rows_side() {
        for (id, _) in ARTWORK {
            let image = row_icon(id, 44).unwrap_or_else(|| panic!("{id} draws at a row's side"));
            assert_eq!((image.size().width, image.size().height), (44, 44), "{id}");
        }
        let portra = row_icon("kodak-portra", 44).unwrap().to_rgba8().unwrap();
        assert!(portra.as_slice().iter().any(|p| p.a > 0), "the artwork's pixels are drawn");
        // Twin: another side is drawn apart, another stock draws other pixels, an unknown one none.
        assert_eq!(row_icon("kodak-portra", 22).unwrap().size().width, 22);
        assert_ne!(row_icon("ilford-hp5", 44).unwrap().to_rgba8().unwrap().as_slice(), portra.as_slice());
        assert!(row_icon("no-such-stock", 44).is_none());
        ROW_ICONS.with(|c| assert!(c.borrow().contains_key(&("kodak-portra", 44)), "kept for the next row"));
    }

    /// FNV-1a over an artwork's bytes.
    fn fnv(data: &[u8]) -> u64 {
        data.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
    }

    /// The owner's own artworks, as 1.5.6 shipped them and the site shows them: a stock's canister is
    /// never redrawn here; a new one comes from the owner, its line added below.
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
        for (id, data) in ARTWORK {
            let kept = ORIGINAL.iter().find(|(k, _)| *k == id).map(|(_, h)| *h);
            assert_eq!(kept, Some(fnv(data)), "{id}: not the owner's artwork, 0x{:016x}", fnv(data));
        }
        // Twin: one byte changed in an artwork is seen.
        let mut touched = ARTWORK[0].1.to_vec();
        touched[60] ^= 1;
        assert_ne!(fnv(&touched), ORIGINAL[0].1);
    }
}
