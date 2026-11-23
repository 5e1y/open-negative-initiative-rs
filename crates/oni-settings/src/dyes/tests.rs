use super::*;
use crate::film::stocks;

#[test]
fn every_colour_negative_names_its_measured_dyes_and_only_a_colour_negative_does() {
    use crate::film::FilmType;
    for kind in stocks().iter() {
        let negative = matches!(kind.kind, FilmType::ColourNegative | FilmType::CineNegative);
        assert_eq!(kind.dyes.is_some(), negative, "{}: a colour negative names its dyes, a slide or black and white none", kind.id);
    }
    assert_eq!(dye_set("kodak-portra"), Some(DyeSet::of("kodak-portra-400")));
    assert_eq!(dye_set("cinestill-800t"), Some(DyeSet::of("kodak-vision3-500t")));
    assert_eq!(dye_set("lucky-color200"), Some(DyeSet::of("generic-c41")), "a stock no profile measures takes the generic set");
    assert_eq!(dye_set("fujifilm-velvia50"), None, "a slide's dyes are never unmixed");
    // Twin: a key the catalogue lacks names no set.
    assert_eq!(dye_set("kodak-portra-900"), None);
}

/// Each precise stock separates on the profile of its own name, each profile reaches a stock the picker lists,
/// and a line keeps the one profile it stood for.
#[test]
fn a_stock_separates_on_its_own_profile_and_a_line_on_the_one_it_stood_for() {
    for kind in stocks().iter().filter(|k| k.offered && !k.id.starts_with("cinestill")) {
        if let Some(set) = dye_set(kind.id).filter(|s| !s.is_generic()) {
            // A push is its stock developed longer, on the stock's own dyes.
            let stock = kind.display_name.split(" (Push").next();
            assert_eq!(set.stock_name(), stock, "{}", kind.id);
        }
    }
    assert_eq!(dye_set("kodak-portra800-push2"), Some(DyeSet::of("kodak-portra-800")));
    for set in DyeSet::all().into_iter().filter(|s| !s.is_generic()) {
        let named = stocks().iter().filter(|k| k.offered && !k.line && dye_set(k.id) == Some(set)).count();
        assert!(named >= 1, "{set:?}: no stock the picker lists separates on it");
    }
    let lines = [
        ("kodak-portra", DyeSet::of("kodak-portra-400")),
        ("kodak-gold", DyeSet::of("kodak-gold-200")),
        ("kodak-ultramax", DyeSet::of("kodak-ultramax-400")),
        ("fujifilm-fujicolor", DyeSet::of("fujifilm-c200")),
        ("fujifilm-fujicolor-pro", DyeSet::of("fujifilm-pro-400h")),
        ("fujifilm-superia", DyeSet::of("fujifilm-xtra-400")),
    ];
    for (id, set) in lines {
        assert_eq!(dye_set(id), Some(set), "{id} renders as it did");
        assert!(crate::film::film_stock(Some(id)).is_some_and(|k| k.line && !k.offered), "{id} is a line, no longer listed");
    }
    assert_eq!(stocks().iter().filter(|k| k.line).count(), lines.len(), "every line is held to the set it stood for");
    // Twin: two speeds of one line separate on two profiles.
    assert_ne!(dye_set("kodak-portra160"), dye_set("kodak-portra800"));
}

#[test]
fn every_set_but_the_generic_names_its_stock_and_reads_by_its_key() {
    for set in DyeSet::all() {
        assert_eq!(set.stock_name().is_some(), !set.is_generic(), "{set:?}");
        let key = serde_json::to_string(&set).unwrap();
        assert_eq!(serde_json::from_str::<DyeSet>(&key).unwrap(), set);
    }
    // A data-sheet twin's key reads as its stock's, written back under the stock's own.
    let twin: DyeSet = serde_json::from_str("\"kodak-portra-800-datasheet\"").unwrap();
    assert_eq!((twin, serde_json::to_string(&twin).unwrap().as_str()), (DyeSet::of("kodak-portra-800"), "\"kodak-portra-800\""));
    // Twin: a key no set carries is kept as stored, never read as another, and unmixes the generic set.
    let lost: DyeSet = serde_json::from_str("\"kodak-portra-900\"").unwrap();
    assert_eq!((lost.key(), lost.is_generic()), ("kodak-portra-900", true));
    let reala: DyeSet = serde_json::from_str("\"fujifilm-superia-reala-datasheet\"").unwrap();
    assert_ne!(reala, DyeSet::of("fujifilm-superia-reala"), "a twin's key only where the set had one");
}
