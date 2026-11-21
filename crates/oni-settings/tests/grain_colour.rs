//! The grain's colour in a sidecar: absent before its format and read as its rest, a moved one kept to
//! the bit, the rest written as nothing; each verdict with a twin.

use oni_settings::blocks::GrainColour;
use oni_settings::document::{self, VERSION};
use oni_settings::migrate::{self, Facts, Refusal, chain};
use oni_settings::{ConversionMode, Document, PipelineSettings};
use serde_json::{Value, json};

fn grained() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Negative, true);
    s.grain.amount = 0.4;
    s
}

/// `settings` written at `version`, its grain's colour key dropped as a file before it kept none.
fn stored(settings: &PipelineSettings, version: i64) -> Vec<u8> {
    let mut object = serde_json::to_value(Document::of(settings.clone())).unwrap();
    object["version"] = json!(version);
    if let Some(Value::Object(grain)) = object["settings"].get_mut("grain") {
        grain.remove("colour");
    }
    serde_json::to_vec(&object).unwrap()
}

fn read(bytes: &[u8]) -> PipelineSettings {
    migrate::reading(bytes, &Facts::bare(true), false).unwrap().0.document.settings
}

#[test]
fn a_grained_frame_from_before_reads_the_colours_rest() {
    assert_eq!(VERSION, chain::DIGITAL_ON_FILM_VERSION);
    let mut mono = grained();
    mono.grain.colour = GrainColour { black: 0.0, white: 0.0 };
    for version in [chain::GRAIN_COLOUR_VERSION - 2, chain::GRAIN_COLOUR_VERSION - 1] {
        let s = read(&stored(&mono, version));
        assert_eq!((s.grain.amount, s.grain.colour), (0.4, GrainColour::REST), "version {version}");
    }
    // Twin: the same frame of this format keeps the monochrome grain it names.
    let mut object = serde_json::to_value(Document::of(mono.clone())).unwrap();
    object["version"] = json!(VERSION);
    assert_eq!(read(&serde_json::to_vec(&object).unwrap()).grain.colour, mono.grain.colour);
}

#[test]
fn a_colour_of_the_random_hue_reads_its_numbers_carried() {
    assert!(chain::GRAIN_COLOUR_VERSION < chain::GRAIN_HUE_VERSION);
    let mut moved = grained();
    moved.grain.colour = GrainColour { black: 1.5, white: 0.3 };
    let mut object = serde_json::to_value(Document::of(moved.clone())).unwrap();
    object["version"] = json!(chain::GRAIN_HUE_VERSION - 1);
    let bytes = serde_json::to_vec(&object).unwrap();
    assert_eq!(migrate::standing(&bytes), migrate::Standing::Older(chain::GRAIN_HUE_VERSION - 1));
    assert_eq!(read(&bytes).grain.colour, moved.grain.colour);
    // Twin: a step rescaling the numbers to another unit moves them.
    assert_ne!(read(&bytes).grain.colour, GrainColour { black: 1.5 / 2.0, white: 0.3 / 2.0 });
}

#[test]
fn a_moved_colour_round_trips_to_the_bit_and_the_rest_writes_nothing() {
    let mut moved = grained();
    moved.grain.colour = GrainColour { black: 2.718_281_7, white: 0.333_333_34 };
    let back = document::decode(&document::encode(&Document::of(moved.clone())).unwrap()).unwrap().settings.grain.colour;
    assert_eq!((back.black.to_bits(), back.white.to_bits()), (moved.grain.colour.black.to_bits(), moved.grain.colour.white.to_bits()));
    let text = |s: &PipelineSettings| String::from_utf8(document::encode(&Document::of(s.clone())).unwrap()).unwrap();
    assert!(!text(&grained()).contains("\"colour\""), "the rest writes a colour");
    // Twin: a moved colour is written.
    assert!(text(&moved).contains("\"colour\""));
}

#[test]
fn a_file_past_this_format_is_refused_as_newer() {
    let bytes = stored(&grained(), VERSION + 1);
    assert!(matches!(migrate::reading(&bytes, &Facts::bare(true), false), Err(Refusal::Newer(v)) if v == VERSION + 1));
    // Twin: this format's own file reads.
    assert!(migrate::reading(&stored(&grained(), VERSION), &Facts::bare(true), false).is_ok());
}
