use super::*;

/// Every stock's reference lies on a negative's windows, and its place in the catalogue is its own precise
/// stock, of its name and on its dyes.
#[test]
fn a_stocks_reference_lies_on_a_negatives_window_and_names_its_dyes() {
    for (i, stock) in VirtualStock::ALL.into_iter().enumerate() {
        assert_eq!(stock as usize, i, "a stock's entry is its place");
        let r = stock.reference();
        let levels = r.levels();
        assert_eq!(levels.green.black, graduation::value(r.black[1], ConversionMode::Negative));
        assert!(levels.windows().iter().all(|w| w.white > w.black), "{stock:?}: {levels:?}");
        let kind = crate::film::film_stock(Some(stock.catalogue_id())).expect("in the catalogue");
        assert!(!kind.line && kind.offered, "{stock:?}: a precise stock the picker lists, never its line");
        assert_eq!(kind.display_name, stock.name(), "{stock:?}");
        if stock.is_silver() {
            // Silver is neutral: the stock rests on the generic set, as a black and white roll does.
            assert_eq!((crate::dyes::dye_set(kind.id), stock.dye_set()), (None, DyeSet::GenericC41), "{stock:?}");
            assert!(r.stops == [0.0; 3] && r.black.iter().chain(&r.white).all(|v| *v == r.black[0] || *v == r.white[0]), "{stock:?}: a neutral reference");
        } else {
            assert_eq!(crate::dyes::dye_set(kind.id), Some(stock.dye_set()), "{stock:?}");
            assert!(stock.dye_set().profile().is_some_and(|p| stock.profile().starts_with(p)), "{stock:?} develops its own dyes");
        }
    }
    let silver = VirtualStock::ALL.iter().filter(|s| s.is_silver()).count();
    assert_eq!(silver, 27, "every black and white film profiled");
    // Twin: a colour stock is no silver one, and its layers' whites part.
    let portra = VirtualStock::KodakPortra800.reference();
    assert!(!VirtualStock::KodakPortra800.is_silver() && portra.white[0] != portra.white[2]);
    assert_eq!(VirtualStock::ALL[0], VirtualStock::default(), "Portra 800 opens first");
    let r = VirtualStock::KodakPortra800.reference();
    // Twin: read on a slide's axis the same shares land elsewhere.
    assert_ne!(r.levels().green.black, graduation::value(r.black[1], ConversionMode::Positive));
}

/// A tungsten stock is balanced warmer than a daylight one, at about 2800 K.
#[test]
fn a_tungsten_stock_stands_at_its_lamp_and_a_daylight_one_at_d55() {
    let kelvin = |s: VirtualStock| s.white().kelvin;
    assert!((kelvin(VirtualStock::KodakPortra800) - 5500.0).abs() < 60.0, "{}", kelvin(VirtualStock::KodakPortra800));
    for stock in [VirtualStock::KodakVision3_200T, VirtualStock::KodakVision3_500T] {
        assert_eq!(stock.balance(), Balance::Tungsten);
        assert!((2700.0..2900.0).contains(&kelvin(stock)), "{stock:?}: {}", kelvin(stock));
    }
    // Twin: a daylight cine stock stands at D55.
    assert_eq!(VirtualStock::KodakVision3_250D.white(), VirtualStock::KodakPortra800.white());
}

#[test]
fn a_film_missing_its_exposure_reads_at_the_films_speed_and_a_stock_by_its_key() {
    let f: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-portra-800"}"#).unwrap();
    assert_eq!(f, VirtualFilm::default());
    for stock in VirtualStock::ALL {
        let key = serde_json::to_string(&stock).unwrap();
        assert_eq!(serde_json::from_str::<VirtualStock>(&key).unwrap(), stock);
    }
    let gold: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-gold-200","exposure":0.5}"#).unwrap();
    assert_eq!((gold.stock, gold.exposure), (VirtualStock::KodakGold200, 0.5));
    let push: VirtualFilm = serde_json::from_str(r#"{"stock":"kodak-portra-800-push-2-datasheet"}"#).unwrap();
    assert_eq!(push.stock, VirtualStock::KodakPortra800Push2, "a data-sheet twin's key reads as its stock's");
    // Twin: an unknown stock is refused, never read as another.
    assert!(serde_json::from_str::<VirtualFilm>(r#"{"stock":"kodak-gold-900"}"#).is_err());
    assert!(serde_json::from_str::<VirtualFilm>(r#"{"stock":"fujifilm-pro-800-datasheet"}"#).is_err());
}
