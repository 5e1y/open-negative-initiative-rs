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
        assert!(!kind.line && kind.offered, "{stock:?}: a precise stock the picker lists, never its line");
        assert_eq!(kind.display_name, stock.name(), "{stock:?}");
        if stock.is_silver() || stock.entry().reversal {
            // Silver is neutral and a slide is projected, never read as a negative: each rests on the generic set.
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
    let kelvin = |s: VirtualStock| s.white().kelvin;
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
fn a_reference_spanned_to_a_rolls_lights_keeps_its_black_and_a_grey_on_one_share() {
    let r = VirtualStock::of("kodak-portra-800").reference();
    let narrow = r.spanned(0.75);
    assert_eq!(narrow.black, r.black, "the bare film stays the black");
    assert_eq!(narrow.stops, r.stops);
    // A grey entering every window of the reference at one share enters every spanned one at one share too.
    let share = |s: &StockReference, grey: [f32; 3]| -> [f32; 3] { std::array::from_fn(|c| (grey[c] - s.black[c]) / (s.white[c] - s.black[c])) };
    let grey: [f32; 3] = std::array::from_fn(|c| r.black[c] + 0.4 * (r.white[c] - r.black[c]));
    let shares = share(&narrow, grey);
    assert!(shares.iter().all(|v| (v - 0.4 / 0.75).abs() < 1e-5), "{shares:?}");
    assert_eq!(r.spanned(1.0), r, "the reference's own span changes nothing");
    // Twin: a white moved on one channel alone tints that grey.
    let mut tinted = narrow;
    tinted.white[0] += 0.05;
    assert!((share(&tinted, grey)[0] - share(&tinted, grey)[1]).abs() > 1e-3);
    // A share at or under the black holds the minimum span.
    assert!((0..3).all(|c| r.spanned(0.0).white[c] - r.black[c] >= crate::tone::Print::MINIMUM_SPAN - 1e-6));
}
