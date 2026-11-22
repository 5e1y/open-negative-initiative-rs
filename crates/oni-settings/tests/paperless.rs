//! Print on paper's switch as a sidecar keeps it: unwritten while on, so every older paper print reads on its paper and
//! nothing moves; written off, read back off; the paper kept as chosen; a Prepare and a paste carrying it.

use oni_settings::document::{self, Document};
use oni_settings::settings::look_settings;
use oni_settings::{ConversionMode, PaperStock, PipelineSettings};

fn on_royal() -> PipelineSettings {
    PipelineSettings { print_paper: Some(PaperStock::KodakRoyalDigital), ..PipelineSettings::rest(ConversionMode::Negative, true) }
}

#[test]
fn an_older_paper_print_reads_on_its_paper_and_the_switch_is_written_off_alone() {
    let on = on_royal();
    let text = serde_json::to_string(&on).unwrap();
    assert!(text.contains("printPaper") && !text.contains("paperless"), "unwritten while on");
    // A sidecar of the format before the switch: its paper print reads on its paper.
    let older = serde_json::json!({ "format": document::FORMAT, "version": 55, "settings": serde_json::from_str::<serde_json::Value>(&text).unwrap() });
    let read: Document = serde_json::from_value(older).unwrap();
    assert_eq!((read.settings.print_paper, read.settings.paperless), (Some(PaperStock::KodakRoyalDigital), false));
    let off = PipelineSettings { paperless: true, ..on.clone() };
    let text = serde_json::to_string(&off).unwrap();
    assert!(text.contains("\"paperless\":true") && text.contains("kodak-royal-digital"), "off, the paper kept: {text}");
    let back: PipelineSettings = serde_json::from_str(&text).unwrap();
    assert!(back.paperless && back == off);
    // Twin: the switch off is another print than on.
    assert_ne!(back, on);
    assert!(document::VERSION >= 56, "an older build opens a paperless print read-only");
}

#[test]
fn a_prepare_prints_on_its_paper_and_a_paste_carries_the_switch() {
    let off = PipelineSettings { paperless: true, ..on_royal() };
    let prepared = off.clone().on_paper();
    assert_eq!((prepared.print_paper, prepared.paperless), (Some(PaperStock::KodakRoyalDigital), false));
    let pasted = look_settings::copied(&off, &PipelineSettings::rest(ConversionMode::Negative, true));
    assert!(pasted.paperless && pasted.print_paper == off.print_paper);
    assert!(look_settings::FIELDS.contains(&"paperless"));
    // Twin: pasted from a print on paper, the switch comes back on.
    assert!(!look_settings::copied(&on_royal(), &off).paperless);
}
