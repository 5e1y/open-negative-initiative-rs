//! The app's words in the reader's language. English is written in the source; French and Spanish
//! come from the gettext catalogues the Slint interface bundles, so one file holds a language's words.

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

mod po;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    French,
    Spanish,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::English, Language::French, Language::Spanish];

    /// The catalogue's folder, which Slint's `select_bundled_translation` names too; empty for English.
    pub fn code(self) -> &'static str {
        match self {
            Language::English => "",
            Language::French => "fr",
            Language::Spanish => "es",
        }
    }

    /// The preference's word for it, and the language a word names; none for an unknown word.
    pub fn key(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::French => "fr",
            Language::Spanish => "es",
        }
    }

    pub fn from_key(key: &str) -> Option<Language> {
        Language::ALL.into_iter().find(|l| l.key() == key)
    }

    /// The system's language when the app speaks it, else English.
    pub fn system() -> Language {
        sys_locale::get_locale().map_or(Language::English, |l| Language::of_locale(&l))
    }

    /// A locale such as `fr-FR` or `es_419` read by its language alone.
    pub fn of_locale(locale: &str) -> Language {
        let base = locale.split(['-', '_', '@', '.']).next().unwrap_or_default().to_ascii_lowercase();
        Language::ALL.into_iter().find(|l| l.key() == base).unwrap_or(Language::English)
    }

    fn catalogue(self) -> Option<&'static HashMap<String, Vec<String>>> {
        static FRENCH: LazyLock<HashMap<String, Vec<String>>> = LazyLock::new(|| po::parse(include_str!("../lang/fr/LC_MESSAGES/oni-ui.po")));
        static SPANISH: LazyLock<HashMap<String, Vec<String>>> = LazyLock::new(|| po::parse(include_str!("../lang/es/LC_MESSAGES/oni-ui.po")));
        match self {
            Language::English => None,
            Language::French => Some(&FRENCH),
            Language::Spanish => Some(&SPANISH),
        }
    }

    /// Which plural form `n` takes: French counts 0 and 1 as one, English and Spanish 1 alone.
    fn plural(self, n: u64) -> usize {
        match self {
            Language::French => usize::from(n > 1),
            Language::English | Language::Spanish => usize::from(n != 1),
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// The language every `tr` reads from now on.
pub fn set(language: Language) {
    CURRENT.store(Language::ALL.iter().position(|l| *l == language).unwrap_or(0) as u8, Ordering::Relaxed);
}

pub fn current() -> Language {
    Language::ALL[usize::from(CURRENT.load(Ordering::Relaxed)).min(Language::ALL.len() - 1)]
}

/// `english` in the current language; itself where the catalogue lacks it.
pub fn tr(english: &'static str) -> &'static str {
    translated(current(), english, None).unwrap_or(english)
}

/// The singular or the plural for `n`, in the current language, `{n}` standing for the count.
pub fn trn(one: &'static str, many: &'static str, n: u64) -> String {
    let language = current();
    let form = language.plural(n);
    let chosen = translated(language, one, Some(form)).unwrap_or(if n == 1 { one } else { many });
    chosen.replace("{n}", &n.to_string())
}

fn translated(language: Language, english: &str, form: Option<usize>) -> Option<&'static str> {
    let forms = language.catalogue()?.get(english)?;
    let text = forms.get(form.unwrap_or(0))?;
    (!text.is_empty()).then_some(text.as_str())
}

/// A text kept in a constant and translated where it is shown, `tr(constant)`: the mark is what
/// `scripts/i18n` reads, as a constant cannot call `tr`.
pub const fn marked(english: &'static str) -> &'static str {
    english
}

/// `tr` on a template whose `{name}` places take the values given, as `format!` would.
#[macro_export]
macro_rules! tr {
    ($english:literal $(,)?) => { $crate::tr($english).to_string() };
    ($english:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut text = $crate::tr($english).to_string();
        $(text = text.replace(concat!("{", stringify!($name), "}"), &$value.to_string());)+
        text
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locale_is_read_by_its_language() {
        assert_eq!(Language::of_locale("fr-FR"), Language::French);
        assert_eq!(Language::of_locale("es_419"), Language::Spanish);
        assert_eq!(Language::of_locale("de-DE"), Language::English, "a language the app does not speak reads English");
        // Twin: a region alone never names the language.
        assert_ne!(Language::of_locale("en-FR"), Language::French);
    }

    #[test]
    fn french_counts_zero_as_one_and_spanish_does_not() {
        assert_eq!((Language::French.plural(0), Language::French.plural(1), Language::French.plural(2)), (0, 0, 1));
        assert_eq!((Language::Spanish.plural(0), Language::Spanish.plural(1)), (1, 0));
        assert_ne!(Language::French.plural(0), Language::Spanish.plural(0), "twin: the two rules differ at zero");
    }
}
