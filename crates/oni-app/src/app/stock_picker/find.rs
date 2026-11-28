//! The stock sheet's search and brand filter: the words typed, each sought in a stock's brand and name, and the one
//! brand chosen beside them, none for every brand.

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Find {
    pub words: String,
    pub brand: Option<String>,
}

impl Find {
    /// Whether a stock of `brand` named `name` shows: of the brand chosen, if any, and every word typed in its brand or
    /// its name.
    pub fn shows(&self, brand: &str, name: &str) -> bool {
        let found = format!("{brand} {name}").to_lowercase();
        self.brand.as_deref().is_none_or(|b| b == brand) && self.words.split_whitespace().all(|w| found.contains(&w.to_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_is_sought_in_the_brand_and_the_name_within_the_brand_chosen() {
        let find = Find { words: "kodak 400".into(), brand: None };
        assert!(find.shows("Kodak", "Portra 400") && find.shows("Kodak", "Tri-X 400"));
        // Twins: a word in neither leaves it out, another brand chosen leaves out every stock the words find.
        assert!(!find.shows("Ilford", "HP5 Plus 400") && !find.shows("Kodak", "Portra 800"));
        let ilford = Find { words: "kodak portra".into(), brand: Some("Ilford".into()) };
        assert!(!ilford.shows("Kodak", "Portra 400") && !ilford.shows("Ilford", "HP5 Plus 400"));
        assert!(Find { words: String::new(), brand: Some("Ilford".into()) }.shows("Ilford", "HP5 Plus 400"));
    }
}
