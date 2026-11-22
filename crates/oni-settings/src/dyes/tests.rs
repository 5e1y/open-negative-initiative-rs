use super::*;
use crate::film::FILM_STOCKS;

#[test]
fn every_colour_negative_names_its_measured_dyes_and_only_a_colour_negative_does() {
    use crate::film::FilmType;
    for (kind, (id, set)) in FILM_STOCKS.iter().zip(STOCK_DYE_SETS) {
        assert_eq!(kind.id, id, "the dye sets follow the catalogue's order");
        let negative = matches!(kind.kind, FilmType::ColourNegative | FilmType::CineNegative);
        assert_eq!(set.is_some(), negative, "{id}: a colour negative names its dyes, a slide or black and white none");
    }
    assert_eq!(dye_set("kodak-portra"), Some(DyeSet::KodakPortra400));
    assert_eq!(dye_set("cinestill-800t"), Some(DyeSet::KodakVision3_500T));
    assert_eq!(dye_set("lucky-color200"), Some(DyeSet::GenericC41), "a stock no profile measures takes the generic set");
    assert_eq!(dye_set("fujifilm-velvia50"), None, "a slide's dyes are never unmixed");
    // Twin: a key the catalogue lacks names no set.
    assert_eq!(dye_set("kodak-portra-900"), None);
}

/// Each precise stock separates on the profile of its own name, each profile reaches a stock the picker lists,
/// and a line keeps the one profile it stood for.
#[test]
fn a_stock_separates_on_its_own_profile_and_a_line_on_the_one_it_stood_for() {
    for kind in FILM_STOCKS.iter().filter(|k| k.offered && !k.id.starts_with("cinestill")) {
        if let Some(set) = dye_set(kind.id).filter(|s| !s.is_generic()) {
            // A push is its stock developed longer, on the stock's own dyes.
            let stock = kind.display_name.split(" (Push").next();
            assert_eq!(set.stock_name(), stock, "{}", kind.id);
        }
    }
    assert_eq!(dye_set("kodak-portra800-push2"), Some(DyeSet::KodakPortra800));
    for set in DyeSet::ALL.into_iter().filter(|s| !s.is_generic()) {
        let named = FILM_STOCKS.iter().filter(|k| k.offered && !k.line && dye_set(k.id) == Some(set)).count();
        assert!(named >= 1, "{set:?}: no stock the picker lists separates on it");
    }
    let lines = [
        ("kodak-portra", DyeSet::KodakPortra400),
        ("kodak-gold", DyeSet::KodakGold200),
        ("kodak-ultramax", DyeSet::KodakUltraMax400),
        ("fujifilm-fujicolor", DyeSet::FujifilmC200),
        ("fujifilm-fujicolor-pro", DyeSet::FujifilmPro400H),
        ("fujifilm-superia", DyeSet::FujifilmXtra400),
    ];
    for (id, set) in lines {
        assert_eq!(dye_set(id), Some(set), "{id} renders as it did");
        assert!(crate::film::film_stock(Some(id)).is_some_and(|k| k.line && !k.offered), "{id} is a line, no longer listed");
    }
    assert_eq!(FILM_STOCKS.iter().filter(|k| k.line).count(), lines.len(), "every line is held to the set it stood for");
    // Twin: two speeds of one line separate on two profiles.
    assert_ne!(dye_set("kodak-portra160"), dye_set("kodak-portra800"));
}

#[test]
fn every_set_but_the_generic_names_its_profile_and_reads_by_its_key() {
    for set in DyeSet::ALL {
        assert_eq!(set.profile().is_some(), !set.is_generic(), "{set:?}");
        assert_eq!(set.stock_name().is_some(), set.profile().is_some(), "{set:?}");
        let key = serde_json::to_string(&set).unwrap();
        assert_eq!(serde_json::from_str::<DyeSet>(&key).unwrap(), set);
    }
    // A data-sheet twin's key reads as its stock's, written back under the stock's own.
    let twin: DyeSet = serde_json::from_str("\"kodak-portra-800-datasheet\"").unwrap();
    assert_eq!((twin, serde_json::to_string(&twin).unwrap().as_str()), (DyeSet::KodakPortra800, "\"kodak-portra-800\""));
    // Twin: a key no set carries is refused, never read as another.
    assert!(serde_json::from_str::<DyeSet>("\"kodak-portra-900\"").is_err());
    assert!(serde_json::from_str::<DyeSet>("\"fujifilm-superia-reala-datasheet\"").is_err());
}
