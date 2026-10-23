//! The language the app speaks: the Settings' choice, else the system's, set on the interface's
//! bundled catalogues and on the Rust side's words together.

use crate::preferences::{self, Preferences};
use oni_i18n::Language;

/// The language kept in the preferences, none to follow the system.
pub fn chosen(preferences: &Preferences) -> Option<Language> {
    preferences.string(preferences::LANGUAGE).and_then(Language::from_key)
}

/// Speaks `language`, or the system's for none, from the next text drawn; after the window exists,
/// as Slint selects its catalogue on a live component.
pub fn speak(language: Option<Language>) {
    // `ONI_LANGUAGE` stands in for the system's, so the checks read English on any machine.
    let fallback = || std::env::var("ONI_LANGUAGE").ok().and_then(|k| Language::from_key(&k)).unwrap_or_else(Language::system);
    let language = language.unwrap_or_else(fallback);
    oni_i18n::set(language);
    let _ = slint::select_bundled_translation(language.code());
}
