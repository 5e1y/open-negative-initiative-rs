use super::*;

/// Every stock's reference lies on a negative's windows, and its place in the catalogue is its own precise
/// stock, of its name and on its dyes.
#[test]
fn a_stocks_reference_lies_on_a_negatives_window_and_names_its_dyes() {
    for stock in VirtualStock::all() {
        let r = stock.reference();
        let levels = r.levels();
        assert_eq!(levels.green.black, graduation::value(r.black[1], ConversionMode::Negative));
        assert!(levels.windows().iter().all(|w| w.white > w.black), "{stock:?}: {levels:?}");
        let kind = crate::film::film_stock(Some(stock.catalogue_id())).expect("in the catalogue");
        assert!(!kind.line && kind.offered != stock.entry().neutral, "{stock:?}: a precise stock a roll's picker lists, never its line; no roll is shot on a neutral film");
        assert_eq!(kind.display_name, stock.name(), "{stock:?}");
        if stock.is_silver() || stock.entry().reversal || stock.entry().neutral {
            // Silver and a neutral film are neutral and a slide is projected, never read as a negative: each rests on the generic set.
            assert_eq!((crate::dyes::dye_set(kind.id), stock.dye_set()), (None, DyeSet::of("generic-c41")), "{stock:?}");
            assert!(r.stops == [0.0; 3] && r.black.iter().chain(&r.white).all(|v| *v == r.black[0] || *v == r.white[0]), "{stock:?}: a neutral reference");
        } else {
            assert_eq!(crate::dyes::dye_set(kind.id), Some(stock.dye_set()), "{stock:?}");
            assert!(stock.key().starts_with(stock.dye_set().key()), "{stock:?} develops its own dyes");
        }
    }
    let silver = VirtualStock::all().iter().filter(|s| s.is_silver()).count();
    assert_eq!(silver, 27, "every black and white film profiled");
    // Twin: a colour stock is no silver one, and its layers' whites part.
    let portra = VirtualStock::of("kodak-portra-800").reference();
    assert!(!VirtualStock::of("kodak-portra-800").is_silver() && portra.white[0] != portra.white[2]);
    assert!(VirtualStock::all().contains(&VirtualStock::default()), "Portra 800, which a digital photo opens on, is listed");
    let r = VirtualStock::of("kodak-portra-800").reference();
    // Twin: read on a slide's axis the same shares land elsewhere.
    assert_ne!(r.levels().green.black, graduation::value(r.black[1], ConversionMode::Positive));
}

/// A tungsten stock is balanced warmer than a daylight one, at about 2800 K.
#[test]
fn a_tungsten_stock_stands_at_its_lamp_and_a_daylight_one_at_d55() {
    let kelvin = |s: VirtualStock| s.white().unwrap().kelvin;
    assert!((kelvin(VirtualStock::of("kodak-portra-800")) - 5500.0).abs() < 60.0, "{}", kelvin(VirtualStock::of("kodak-portra-800")));
    for stock in [VirtualStock::of("kodak-vision3-200t"), VirtualStock::of("kodak-vision3-500t")] {
        assert_eq!(stock.balance(), Balance::Tungsten);
        assert!((2700.0..2900.0).contains(&kelvin(stock)), "{stock:?}: {}", kelvin(stock));
    }
    // Twin: a daylight cine stock stands at D55.
    assert_eq!(VirtualStock::of("kodak-vision3-250d").white(), VirtualStock::of("kodak-portra-800").white());
}

#[test]
fn a_film_missing_its_exposure_reads_at_the_films_speed_and_a_stock_by_its_key() {
    let f: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-portra-800"}"#).unwrap();
    assert_eq!(f, VirtualFilm::default());
    for stock in VirtualStock::all() {
        let key = serde_json::to_string(&stock).unwrap();
        assert_eq!(serde_json::from_str::<VirtualStock>(&key).unwrap(), stock);
    }
    let gold: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-gold-200","exposure":0.5}"#).unwrap();
    assert_eq!((gold.stock, gold.exposure), (VirtualStock::of("kodak-gold-200"), 0.5));
    let push: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-portra-800-push-2-datasheet"}"#).unwrap();
    assert_eq!(push.stock, VirtualStock::of("kodak-portra-800-push-2"), "a data-sheet twin's key reads as its stock's");
    // Twin: an unknown stock is kept as stored, never read as another.
    let lost: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-gold-900"}"#).unwrap();
    assert_eq!((lost.stock.key(), lost.stock.is_known()), ("kodak-gold-900", false));
    let twin: VirtualFilm = serde_json::from_str(r#"{"stock":"fujifilm-pro-800-datasheet"}"#).unwrap();
    assert!(!twin.stock.is_known(), "a twin's key only where the stock had one");
}

#[test]
fn picking_a_film_lays_its_effects_and_preparing_keeps_the_photos() {
    use crate::PipelineSettings;
    let stock = VirtualStock::of("kodak-portra-800");
    let rests = stock.effects();
    let mut photo = PipelineSettings::rest(ConversionMode::Digital, true);
    (photo.grain.size, photo.halation.amount) = (40.0, 0.2);
    let picked = photo.picking(stock);
    assert_eq!((picked.halation, picked.bloom, picked.grain.size), (rests.halation, rests.bloom, rests.grain_size));
    assert_eq!((picked.gains, picked.levels), (photo.on_stock(stock).gains, photo.on_stock(stock).levels), "its reference as Prepare lays it");
    // Twin: Prepare places the film's reference and keeps the photo's own effects.
    let prepared = photo.on_stock(stock);
    assert_eq!((prepared.grain.size, prepared.halation.amount), (40.0, 0.2));
}
