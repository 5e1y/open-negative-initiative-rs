//! The catalogue's artworks: each stock's and paper's drawn, the owner's own, at every side a list asks.

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

/// Every shipped canister passes the app's SVG reader and draws something at a row's side.
#[test]
fn every_shipped_canister_draws_at_a_rows_side() {
    for (name, bytes) in shipped_svgs() {
        let pixels = sized(bytes, 44).and_then(|svg| Image::load_from_svg_data(&svg).ok()).and_then(|i| i.to_rgba8());
        let inked = pixels.is_some_and(|p| p.as_bytes().chunks(4).any(|px| px[3] > 0));
        assert!(inked, "{name} draws nothing through the SVG reader");
    }
    // Twin: a file the reader refuses is seen.
    assert!(sized(b"<svg", 44).and_then(|svg| Image::load_from_svg_data(&svg).ok()).is_none());
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
    let neutral = oni_settings::film::film_stock(Some("generic-neutral")).unwrap();
    assert!(shipped_svg("generic-neutral.svg").is_none() && neutral.canister.is_none());
    assert_eq!(artwork(neutral.id).map(|a| a.hash), artwork(neutral.kind.generic()).map(|a| a.hash));
}

/// A stock drawn with its line's or its kind's generic canister draws it by its own key, at any size.
#[test]
fn a_stock_draws_the_canister_its_catalogue_line_names() {
    let pixels = |id: &str| row_icon(id, 44).and_then(|i| i.to_rgba8()).map(|p| p.as_slice().to_vec());
    assert!(pixels("kodak-portra400").is_some() && pixels("kodak-portra400") == pixels("kodak-portra"), "Portra 400 draws the Portra canister");
    assert!(pixels("generic-neutral") == pixels("generic-color-negative") && icon("generic-neutral").is_some(), "an undrawn negative draws its kind's");
    // Twin: a stock of its own draws its own, never its brand's line.
    assert_ne!(pixels("kodak-gold200"), pixels("kodak-portra"));
}

/// Every paper draws its own box, named by its file, as a stock draws its canister.
#[test]
fn every_paper_draws_its_box() {
    for paper in oni_settings::PaperStock::all() {
        assert_eq!(paper.canister(), Some(format!("{}.svg", paper.key())).as_deref(), "{} names its box", paper.key());
        assert!(paper_icon(paper, 44).is_some(), "{} draws its box", paper.key());
    }
    // Twin: a paper no file defines draws nothing.
    assert!(paper_icon(oni_settings::PaperStock::of("oni-standard"), 44).is_none());
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

/// A digital photo's sheet lists the stocks an offered virtual film stands for, every one of them, and no other.
#[test]
fn the_offered_virtual_films_are_listed_alone() {
    let listed: Vec<&str> = virtual_by_brand().into_iter().flat_map(|(_, l)| l).map(|i| oni_settings::film::stocks()[i].id).collect();
    let offered = oni_settings::VirtualStock::all().into_iter().filter(|v| v.entry().offered());
    let virtual_ids: Vec<&str> = offered.map(|v| v.catalogue_id()).collect();
    assert_eq!(listed.len(), virtual_ids.len());
    assert!(virtual_ids.iter().all(|id| listed.contains(id) && virtual_stock(id).is_some()), "{listed:?}");
    // Twin: a roll's stock no virtual film stands for is neither listed nor taken.
    assert!(!listed.contains(&"agfaphoto-apx") && virtual_stock("agfaphoto-apx").is_none());
    assert!(by_brand(None).into_iter().flat_map(|(_, l)| l).count() > listed.len());
    // Every colour negative the scanner reads states its couplers and is offered, a black and white one too.
    assert!(listed.contains(&"kodak-vision3-50d") && listed.contains(&"fujifilm-160nc") && listed.contains(&"kodak-portra400") && listed.contains(&"ilford-hp5"));
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
const ORIGINAL: [(&str, u64); 75] = [
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
    ("foma-cine-ortho400", 0x0b74b92ae9808a92),
    ("foma-fomapan-cine100", 0x02882fe3ab8e893c),
    ("foma-fomapan-cine400", 0x70c5294ce26194ea),
    ("foma-fomapan100", 0xcc226324abfc9ffa),
    ("foma-ortho400", 0x5eec145ae0a2ae0b),
    ("foma-retropan320", 0x952d8ee3cf9c2591),
    ("fujifilm-natura1600", 0x7b6f9cd06660523a),
    ("fujifilm-neopan-ortho", 0x4ed2b6d21945ef9c),
    ("fujifilm-neopan1600", 0x60fb5c96649fbde9),
    ("fujifilm-neopan400", 0x4e9f5bb0e9f59df9),
    ("fujifilm-nexia-a200", 0xc1a21600a2c59e20),
    ("fujifilm-nexia-zoom-master800", 0x10e9ad3f2bdec65b),
    ("fujifilm-nexia400", 0x6de143df0dd35944),
    ("fujifilm-reala-ace", 0x05100fef45ba93c1),
    ("kodak-320txp", 0x487fcec4c6e8a446),
    ("kodak-double-x", 0x083a392b7659f4d0),
    ("kodak-ektar100", 0x41f608ddba4c7749),
    ("kodak-kodachrome64", 0x35f1c8b1534dfaaa),
    ("kodak-tmax-p3200", 0x61cca7f08a0a4bdc),
    ("kodak-tmax100", 0x88d83fb2a655d2f9),
    ("kodak-tmax400", 0x51451e0839455ecb),
    ("kodak-verita200d", 0x70981ac419c45f93),
    ("kodak-vision3-200t", 0x3d865976c30348f2),
    ("kodak-vision3-250d", 0x24ba945f98509b3a),
    ("kodak-vision3-500t", 0x5541511bd8fdf404),
    ("kodak-vision3-50d", 0xfa8436ea0f877f71),
    ("fujifilm-crystal-archive-digital-dpii", 0x9b169c5d7627c02f),
    ("fujifilm-crystal-archive-digital-pearl", 0x6f12c6168cbae786),
    ("fujifilm-frontier-sp-3000", 0xfc6535dc2534ef1f),
    ("kodak-ektacolor-edge", 0xfefb9c0b6739fc52),
    ("kodak-endura-premier", 0x03ba7a99b39053a9),
    ("kodak-endura-premier-metallic", 0x50ebd375d7bb1192),
    ("kodak-photo-book", 0xd3d766ba64de097b),
    ("kodak-royal-digital", 0x517c0636e61d6522),
];

#[test]
fn every_canister_is_the_owners_own_artwork() {
    for (id, hash) in ORIGINAL {
        assert_eq!(shipped_svg(&format!("{id}.svg")).map(fnv), Some(hash), "{id}: not the owner's artwork");
    }
    for (name, _) in shipped_svgs() {
        let key = name.trim_end_matches(".svg");
        let named = oni_settings::film::film_stock(Some(key)).is_some() || oni_settings::PaperStock::all().iter().any(|p| p.key() == key);
        assert!(named, "{name} names no stock or paper of the catalogue: its file takes the stock's key");
    }
    assert_eq!(shipped_svgs().count(), ORIGINAL.len());
    // Twin: one byte changed in an artwork is seen.
    let mut touched = shipped_svg("generic-azure.svg").unwrap().to_vec();
    touched[60] ^= 1;
    assert_ne!(fnv(&touched), ORIGINAL[0].1);
}
