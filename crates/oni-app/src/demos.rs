//! The help cards' demos: one short square loop a topic, filmed by `oni-demos` from the real
//! controls over the real render, played above the note by the tutorials' player.

use oni_i18n::Language;

/// A demo's ceiling, so thirty of them stay a small part of the app.
pub const CEILING: usize = 400 * 1024;

/// The demo of a help topic, as its `HelpMark` names it, in the language the app speaks; a topic
/// without one shows none.
pub fn clip(topic: &str) -> Option<&'static [u8]> {
    clip_in(topic, oni_i18n::current())
}

/// The topic's demo in `language` when it shows words, else its one demo, filmed in English.
pub fn clip_in(topic: &str, language: Language) -> Option<&'static [u8]> {
    worded(topic, language).or_else(|| english(topic))
}

/// The GIF a topic's demo is written to: `<topic>.gif`, or `<topic>.<language>.gif` for one that
/// shows words in another language than English.
pub fn file(topic: &str, language: Language) -> String {
    match language {
        Language::English => format!("{topic}.gif"),
        other => format!("{topic}.{}.gif", other.key()),
    }
}

/// The topics whose demo shows words, the tabs' names, filmed once per language.
pub const WORDED: &[&str] = &["film-type", "source-encoding"];

fn worded(topic: &str, language: Language) -> Option<&'static [u8]> {
    match (topic, language) {
        ("film-type", Language::French) => Some(include_bytes!("../resources/demos/film-type.fr.gif")),
        ("film-type", Language::Spanish) => Some(include_bytes!("../resources/demos/film-type.es.gif")),
        ("source-encoding", Language::French) => Some(include_bytes!("../resources/demos/source-encoding.fr.gif")),
        ("source-encoding", Language::Spanish) => Some(include_bytes!("../resources/demos/source-encoding.es.gif")),
        _ => None,
    }
}

fn english(topic: &str) -> Option<&'static [u8]> {
    match topic {
        "sharpening" => Some(include_bytes!("../resources/demos/sharpening.gif")),
        "white-balance" => Some(include_bytes!("../resources/demos/white-balance.gif")),
        "crop" => Some(include_bytes!("../resources/demos/crop.gif")),
        "measured-roll" => Some(include_bytes!("../resources/demos/measured-roll.gif")),
        "film-type" => Some(include_bytes!("../resources/demos/film-type.gif")),
        "flat-field" => Some(include_bytes!("../resources/demos/flat-field.gif")),
        "source-encoding" => Some(include_bytes!("../resources/demos/source-encoding.gif")),
        "density-ceiling" => Some(include_bytes!("../resources/demos/density-ceiling.gif")),
        "dye-separation" => Some(include_bytes!("../resources/demos/dye-separation.gif")),
        "balance-strength" => Some(include_bytes!("../resources/demos/balance-strength.gif")),
        "paper-correction" => Some(include_bytes!("../resources/demos/paper-correction.gif")),
        "lighting" => Some(include_bytes!("../resources/demos/lighting.gif")),
        "effects" => Some(include_bytes!("../resources/demos/effects.gif")),
        "curves" => Some(include_bytes!("../resources/demos/curves.gif")),
        "spectrogram" => Some(include_bytes!("../resources/demos/spectrogram.gif")),
        "saturation-curve" => Some(include_bytes!("../resources/demos/saturation-curve.gif")),
        "chroma-denoise" => Some(include_bytes!("../resources/demos/chroma-denoise.gif")),
        "texture" => Some(include_bytes!("../resources/demos/texture.gif")),
        "dust" => Some(include_bytes!("../resources/demos/dust.gif")),
        _ => None,
    }
}

/// The topics that have a demo.
pub const TOPICS: &[&str] = &[
    "sharpening",
    "white-balance",
    "crop",
    "measured-roll",
    "film-type",
    "flat-field",
    "source-encoding",
    "density-ceiling",
    "dye-separation",
    "balance-strength",
    "paper-correction",
    "lighting",
    "effects",
    "curves",
    "spectrogram",
    "saturation-curve",
    "chroma-denoise",
    "texture",
    "dust",
];

pub fn within_ceiling(clip: &[u8]) -> bool {
    clip.len() <= CEILING
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tutorial::Frames;
    use std::time::Duration;

    /// Every clip the app ships: each topic's in each language, a worded one's three files.
    fn every() -> Vec<(String, &'static [u8])> {
        let mut out = Vec::new();
        for &topic in TOPICS {
            for language in Language::ALL {
                out.push((file(topic, language), clip_in(topic, language).expect("a demo")));
            }
        }
        out
    }

    #[test]
    fn each_demo_weighs_at_most_its_ceiling() {
        for (name, bytes) in every() {
            assert!(within_ceiling(bytes), "{name} weighs {} KB", bytes.len() / 1024);
        }
        // Twin: a file a byte past the ceiling is caught.
        assert!(!within_ceiling(&vec![0u8; CEILING + 1]));
    }

    #[test]
    fn each_demo_is_square_at_twenty_a_second_and_loops() {
        for (topic, bytes) in every() {
            let mut frames = Frames::new(bytes).expect("a GIF");
            let (w, h) = frames.size();
            assert_eq!(w, h, "{topic} is square");
            let (first, delay) = frames.next().map(|(p, d)| (p.to_vec(), d)).expect("a first frame");
            assert_eq!(delay.as_millis() % 50, 0, "{topic} runs on 20 fps steps: {delay:?}");
            let mut shown = delay;
            loop {
                let (picture, d) = frames.next().expect("frames go on");
                if picture == first.as_slice() {
                    break;
                }
                shown += d;
                assert!(shown < Duration::from_secs(10), "{topic} never came back to its first frame");
            }
            // Twin: the loop came back after seconds of clip, not at once.
            assert!(shown >= Duration::from_secs(1), "{topic} looped after {shown:?}");
        }
    }

    /// The topics the interface's marks name, from every `demo: "…";` in its `.slint` files.
    fn named(dir: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("the interface's folder").flatten() {
            let path = entry.path();
            if path.is_dir() {
                named(&path, out);
            } else if path.extension().is_some_and(|e| e == "slint") {
                let text = std::fs::read_to_string(&path).expect("a .slint file");
                out.extend(text.lines().filter_map(|l| l.trim().strip_prefix("demo: \"")?.strip_suffix("\";")).map(str::to_string));
            }
        }
    }

    #[test]
    fn each_mark_naming_a_demo_has_its_gif_and_each_gif_a_mark() {
        let mut marks = Vec::new();
        named(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../oni-ui/ui"), &mut marks);
        for mark in &marks {
            assert!(clip(mark).is_some(), "the mark's topic {mark} has no GIF");
        }
        for topic in TOPICS {
            assert!(marks.iter().any(|m| m == topic), "no mark names {topic}");
        }
        // Twin: the reader finds a mark's topic, so an empty list above would be caught.
        assert!(marks.iter().any(|m| m == "film-type") && !marks.iter().any(|m| m == "per-channel-levels"));
    }

    #[test]
    fn a_worded_demo_plays_in_the_app_s_language_and_any_other_in_english() {
        for &topic in WORDED {
            let english = clip_in(topic, Language::English).expect("a demo");
            for language in [Language::French, Language::Spanish] {
                assert_ne!(clip_in(topic, language), Some(english), "{topic} has its own {language:?} film");
            }
            assert_ne!(clip_in(topic, Language::French), clip_in(topic, Language::Spanish), "{topic}: two languages, two films");
        }
        // Twin: a demo showing no words plays its English film in every language.
        assert_eq!(clip_in("lighting", Language::French), clip_in("lighting", Language::English));
        assert_eq!(file("film-type", Language::Spanish), "film-type.es.gif");
        assert_eq!(file("film-type", Language::English), "film-type.gif");
    }

    #[test]
    fn a_topic_without_a_demo_shows_none() {
        assert!(clip("").is_none() && clip("film-stock").is_none());
        assert!(TOPICS.iter().all(|t| clip(t).is_some()));
    }
}
