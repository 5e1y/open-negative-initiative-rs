//! The medal as the tree writes it and as the podium places it.

use oni_i18n::tr;
use oni_settings::Medal;

/// The medal as the tree's caption writes it.
pub fn medal(label: Medal) -> String {
    match label {
        Medal::None => String::new(),
        Medal::Bronze => tr("Bronze").into(),
        Medal::Silver => tr("Silver").into(),
        Medal::Gold => tr("Gold").into(),
    }
}

/// The medal's place on the podium, the figure struck in it and the key that lays it: gold 1, silver
/// 2, bronze 3, none 0.
pub fn medal_index(label: Medal) -> i32 {
    match label {
        Medal::None => 0,
        Medal::Gold => 1,
        Medal::Silver => 2,
        Medal::Bronze => 3,
    }
}

pub fn medal_of(index: i32) -> Medal {
    match index {
        1 => Medal::Gold,
        2 => Medal::Silver,
        3 => Medal::Bronze,
        _ => Medal::None,
    }
}
