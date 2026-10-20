//! `localizedCaseInsensitiveCompare` as the English locale orders names: letters first, case and
//! accents only where the letters tie, so "Élan" sits among the E's and never after "Zeiss".

use std::cmp::Ordering;

/// A Latin letter's base letters, accent and case dropped; any other character as it is.
fn base(c: char) -> String {
    let folded = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => "a",
        'ç' | 'Ç' => "c",
        'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => "i",
        'ñ' | 'Ñ' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' => "o",
        'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => "u",
        'ý' | 'ÿ' | 'Ý' => "y",
        'æ' | 'Æ' => "ae",
        'œ' | 'Œ' => "oe",
        'ß' => "ss",
        _ => return c.to_lowercase().collect(),
    };
    folded.to_string()
}

/// `localizedCaseInsensitiveCompare`: the base letters, then the accents, never the case.
pub fn case_insensitive(a: &str, b: &str) -> Ordering {
    let letters = |s: &str| s.chars().map(base).collect::<String>();
    letters(a).cmp(&letters(b)).then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accented_name_sits_among_its_letter_and_case_never_orders() {
        let mut names = vec!["Zeiss", "Élan", "ektar", "Fuji", "émulsion"];
        names.sort_by(|a, b| case_insensitive(a, b));
        assert_eq!(names, ["ektar", "Élan", "émulsion", "Fuji", "Zeiss"]);
        assert_eq!(case_insensitive("Portra", "portra"), Ordering::Equal);
        // Twin: the bytes' order puts the accented names after Z.
        let mut bytes = vec!["Zeiss", "Élan"];
        bytes.sort_by_key(|n| n.to_lowercase());
        assert_eq!(bytes, ["Zeiss", "Élan"]);
    }
}
