use super::*;

#[test]
fn every_readout_writes_its_number_one_way() {
    assert_eq!(Field::Exposure.spec().readout.format(1.5), "+1.50 EV");
    assert_eq!(Field::ColorDensity.spec().readout.format(-0.25), "\u{2212}25%");
    assert_eq!(Field::Temperature.readout(1.0), "Warm +1.00 EV");
    assert_eq!(Field::Tint.readout(-1.5), "Green \u{2212}1.50 EV");
    assert_eq!((Field::Highlights.readout(0.25), Field::Shadows.readout(-0.5)), ("+25%".to_string(), "\u{2212}50%".to_string()));
    assert_eq!(Field::Contrast.readout(0.15), "+15%");
    assert_eq!(Field::Chrome.spec().readout.format(0.5), "50%");
    assert_eq!(Field::BalanceStrength.spec().readout.format(0.67), "67%");
    assert_eq!(Field::SharpenRadius.spec().readout.format(2.092), "2.1 px");
    assert_eq!(Field::Angle.spec().readout.format(-1.0), "\u{2212}1.00°");
    assert_eq!(Field::DyeSeparation.readout(0.6), "60%");
    // Every slider names what its number counts, the ceiling's bare density aside.
    let bare = |text: &str| text.ends_with(|c: char| c.is_ascii_digit());
    for f in Field::ALL {
        let shown = f.readout(f.spec().range.1);
        assert_eq!(bare(&shown), f == Field::DensityCeiling, "{f:?} reads {shown}");
    }
    // Twin: a number written without its unit is caught.
    assert!(bare(&number(0.25, 2, true, Unit::Bare)));
    // The black point reads in the sense its slider moves: stored negative, shown positive.
    assert!(Field::BlackPoint.readout(-0.1).starts_with('+'), "{}", Field::BlackPoint.readout(-0.1));
    // Twin: the stored value's own sign is what the slider never shows.
    assert!(Field::BlackPoint.spec().readout.format(-0.1).starts_with('\u{2212}'));
}

#[test]
fn a_colour_readout_names_the_end_it_leans_to_beside_its_true_unit() {
    assert_eq!((Field::Temperature.readout(-0.3), Field::Tint.readout(0.12)), ("Cool \u{2212}0.30 EV".into(), "Magenta +0.12 EV".into()));
    assert_eq!((Field::FlatTemperature.readout(0.2), Field::FlatTint.readout(-0.05)), ("Warm +20%".into(), "Green \u{2212}5%".into()));
    // At rest, or rounding to it, the readout leans nowhere and names no end.
    assert_eq!((Field::Temperature.readout(0.0), Field::Tint.readout(-0.004)), ("0.00 EV".into(), "0.00 EV".into()));
    // Only a colour axis names an end: Exposure's stops lean toward no hue.
    assert_eq!(Field::Exposure.readout(0.3), "+0.30 EV");
    // Twin: the stops alone, as the formatter writes them, leave the side to the reader.
    assert_eq!(Field::Temperature.spec().readout.format(-0.3), "\u{2212}0.30 EV");
}

#[test]
fn a_value_at_rest_reads_without_a_sign() {
    // The black point rests a hair off zero: it reads 0%, never a signed zero.
    assert_eq!(Field::BlackPoint.readout(Print::RESTING_BLACK_POINT), "0%");
    assert_eq!((Field::Exposure.readout(0.0), Field::Exposure.readout(-0.0)), ("0.00 EV".to_string(), "0.00 EV".to_string()));
    assert_eq!(number(-0.004, 2, true, Unit::Bare), "0.00");
    assert_eq!(number(-0.006, 2, true, Unit::Bare), "\u{2212}0.01");
    // Twin: shown, the rest is a negative zero, which Rust's own signed format writes with a hyphen.
    let shown = Field::BlackPoint.shown(Print::RESTING_BLACK_POINT) * 100.0;
    assert_eq!(format!("{shown:+.0}%"), "-0%");
}

#[test]
fn every_unit_sits_where_the_others_do() {
    assert_eq!(number(85.0, 0, false, Unit::Percent), "85%");
    assert_eq!(number(-4.0, 0, true, Unit::Degrees), "\u{2212}4°");
    assert_eq!(number(2.0, 2, false, Unit::Times), "2.00×");
    assert_eq!(number(12.0, 0, false, Unit::Pixels), "12 px");
    assert_eq!(number(0.3, 1, true, Unit::Stops), "+0.3 EV");
    // Twin: the percent spaced from its number is the old way, no longer written.
    assert_ne!(Readout::Percent.format(0.85), "85 %");
}

#[test]
fn every_field_has_its_place_in_the_slint_store() {
    let store = include_str!("../../../oni-ui/ui/store.slint");
    for f in Field::ALL {
        assert_eq!(Field::from_index(f.index()), Some(f));
        for suffix in ["fraction", "readout", "value"] {
            assert!(store.contains(&format!("{}-{suffix};", f.slint_name())), "{f:?} has no {suffix}");
        }
    }
    // The panel passes each slider's own index back.
    let panel = panel_source();
    for f in Field::ALL {
        // Source Balance names each row by the ends the mode gives it, so its label is the panel's.
        let named = |l: &str| l.contains(&format!("label: @tr(\"{}\");", f.spec().label)) || f.block() == Some(Block::SourceBalance);
        let wired = panel.lines().any(|l| {
            l.contains(&format!("field: {}; label: ", f.index())) && l.contains(&format!("fraction: Store.{}-fraction;", f.slint_name())) && named(l)
        });
        assert!(wired, "{f:?} is not wired by its index, label and fraction");
    }
    // Twin: a field the store never declared is caught.
    assert!(!store.contains("grain-fraction;"));
}

#[test]
fn each_slider_spans_its_knobs_range_on_a_mosaic_negative() {
    let s = oni_settings::PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    for f in Field::ALL {
        assert_eq!(f.knob().range(&s), Some(f.spec().range), "{f:?}");
    }
    // Twin: on a plain file the ceiling's axis is another, which the spec does not claim to follow.
    let plain = oni_settings::PipelineSettings::rest(oni_settings::ConversionMode::Negative, false);
    assert_ne!(Field::DensityCeiling.knob().range(&plain), Some(Field::DensityCeiling.spec().range));
}

#[test]
fn a_double_click_on_the_ceiling_lands_on_the_frames_own_rest() {
    use oni_settings::{ConversionMode, PipelineSettings};
    let slide = PipelineSettings::rest(ConversionMode::Positive, true);
    assert_eq!(Field::DensityCeiling.base(&slide), slide.density_ceiling);
    let negative = PipelineSettings::rest(ConversionMode::Negative, true);
    assert_eq!(Field::DensityCeiling.base(&negative), negative.density_ceiling);
    // Twin: the two rests differ, so a base read for the negative alone fails on the slide.
    assert_ne!(slide.density_ceiling, Field::DensityCeiling.spec().base);
    assert_eq!(Field::Exposure.base(&slide), 0.0, "Exposure's base is `Tone.neutral`, never the mode's rest");
    let mut measured = negative.clone();
    let id = oni_settings::Id::new();
    measured.roll_reference = Some(oni_settings::frame::RollReference { id, stops: [0.3, 0.0, -0.2], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
    assert_eq!(Field::GainRed.double_click_base(&measured), 0.0, "a double click lands on Gains.base");
    // Twin: the block's reset lands on the roll's stops.
    assert_eq!(Field::GainRed.base(&measured), 0.3);
    // A slide's temperature and tint rest on its measured gains', a double click on Gains.base.
    let mut gained = slide.clone();
    gained.measured_gains = Some([0.4, 0.1, -0.2]);
    assert!((Field::GainTemperature.base(&gained) - 0.3).abs() < 1e-6 && (Field::GainTint.base(&gained) - 0.0).abs() < 1e-6);
    assert_eq!(Field::GainTemperature.double_click_base(&gained), 0.0);
    assert!(Field::GainTint.base(&PipelineSettings { measured_gains: Some([0.0, 0.3, 0.0]), ..slide }) > 0.29, "twin: green raised reads as tint");
}

/// The panel's Slint, its tabs' files after it.
pub(crate) fn panel_source() -> String {
    [
        include_str!("../../../oni-ui/ui/panel.slint"),
        include_str!("../../../oni-ui/ui/panel/state.slint"),
        include_str!("../../../oni-ui/ui/panel/parts.slint"),
        include_str!("../../../oni-ui/ui/panel/roll_tab.slint"),
        include_str!("../../../oni-ui/ui/panel/crop_tab.slint"),
        include_str!("../../../oni-ui/ui/panel/dedust_tab.slint"),
        include_str!("../../../oni-ui/ui/panel/color_tab.slint"),
        include_str!("../../../oni-ui/ui/panel/details_tab.slint"),
    ]
    .concat()
}

/// The `title:` of each block in one pane of the panel, in order.
fn pane_titles(panel: &str, pane: &str) -> Vec<String> {
    let body = &panel[panel.find(&format!("component {pane} ")).expect("the pane")..];
    let next = ["\ncomponent ", "\nexport component "].iter().filter_map(|m| body[1..].find(m)).min();
    let body = &body[..next.map_or(body.len(), |i| i + 1)];
    body.lines().filter_map(|l| l.trim().strip_prefix("title: @tr(\"")?.split('"').next().map(str::to_string)).collect()
}

/// Whether `shown` keeps `order`'s relative order, every one of its blocks found there.
fn follows(shown: &[String], order: &[String]) -> bool {
    let mut at = 0;
    shown.iter().all(|t| match order[at..].iter().position(|o| o == t) {
        Some(i) => {
            at += i + 1;
            true
        }
        None => false,
    })
}

/// `SettingsPane.body`'s sections as drawn: `manualBalance` is a property laid where the body
/// names it, though its text comes later in the file.
fn swift_roll_order(text: &str) -> Vec<String> {
    let sections = |t: &str| -> Vec<String> { t.split("DSSection(\"").skip(1).filter_map(|t| t.split('"').next().map(str::to_string)).collect() };
    let body = &text[text.find("var body").expect("body")..text.find("private var rollBlock").expect("rollBlock")];
    let manual = &text[text.find("private var manualBalance").expect("manualBalance")..text.find("// MARK: - Keys").expect("keys")];
    let at = body.find("manualBalance\n").expect("the body names manualBalance");
    [sections(&body[..at]), sections(manual), sections(&body[at..])].concat()
}

#[test]
fn the_roll_tab_lays_its_blocks_in_the_swift_order() {
    // `SettingsPane.body`, read from the Swift source when it is beside this checkout.
    let swift: Vec<String> = match crate::swift_sources().and_then(|d| std::fs::read_to_string(d.join("UI/SettingsPane.swift")).ok()) {
        Some(text) => swift_roll_order(&text),
        None => ["Measured Roll", "Film Type", "Flat-Field", "Source Encoding", "Source Balance", "Density Ceiling", "Per-Channel Levels", "Dye Separation", "Balance Strength"]
            .map(String::from)
            .to_vec(),
    };
    // Three blocks are named for a photographer, each read under Swift's title.
    let swift_title = |t: String| match t.as_str() {
        "Filtration" => "Source Balance".to_string(),
        "Highlight Detail" => "Density Ceiling".to_string(),
        "Scan Profile" => "Source Encoding".to_string(),
        _ => t,
    };
    let shown: Vec<String> = pane_titles(&panel_source(), "RollPane").into_iter().map(swift_title).collect();
    assert!(shown.len() >= 6, "{shown:?}");
    assert!(follows(&shown, &swift), "the Roll tab shows {shown:?}, Swift lays {swift:?}");
    // Twin: Dye Separation and Balance Strength ahead of the ceiling break the order.
    let early: Vec<String> = ["Measured Roll", "Film Type", "Flat-Field", "Dye Separation", "Balance Strength", "Density Ceiling"].map(String::from).to_vec();
    assert!(!follows(&early, &swift));
}

#[test]
fn the_black_point_runs_the_other_way() {
    let f = Field::BlackPoint;
    assert_eq!(f.track(), (-Print::BLACK_POINT_RANGE.1, -Print::BLACK_POINT_RANGE.0));
    assert_eq!(f.shown(0.1), -0.1);
    assert_eq!(Field::WhitePoint.shown(0.1), 0.1);
}

/// The `field:` indices wired in the Store block titled `title`, up to the next block's title.
fn wired_in(panel: &str, title: &str) -> Vec<i32> {
    let body = &panel[panel.find(&format!(": Block {{\n        title: @tr(\"{title}\");")).expect("the block") + 1..];
    let next = body.find("title: @tr(\"").and_then(|t| body[t + 1..].find("title: @tr(\"").map(|i| t + 1 + i));
    let body = &body[..next.unwrap_or(body.len())];
    body.split("field: ").skip(1).filter_map(|t| t.split(';').next()?.parse().ok()).collect()
}

/// Whether the Pro face lays the light, then the colour and the paper's, then the effects.
fn light_first(titles: &[String]) -> bool {
    titles.windows(4).any(|w| w == ["Lighting", "Colour", "Paper Correction", "Effects"])
}

#[test]
fn the_pro_face_lays_the_light_first_contrast_with_it() {
    let panel = panel_source();
    let titles = pane_titles(&panel, "ColorPane");
    assert!(light_first(&titles), "{titles:?}");
    // Contrast is drawn, and reset, with the light; the effects keep the rest.
    let indices = |b: Block| b.fields().iter().map(|f| f.index()).collect::<Vec<_>>();
    assert_eq!(wired_in(&panel, "Lighting"), indices(Block::Lighting));
    assert_eq!(wired_in(&panel, "Effects"), indices(Block::Effects));
    assert_eq!(Field::Contrast.block(), Some(Block::Lighting));
    // Twin: the colour ahead of the light, Contrast among the effects, are both caught.
    let colour_first = ["Machine", "Colour", "Lighting", "Colour", "Paper Correction", "Lighting", "Effects"].map(String::from);
    assert!(!light_first(&colour_first));
    let old = ": Block {\n        title: @tr(\"Effects\");\n        StoreSlider { field: 8; }\n        StoreSlider { field: 9; }\n        StoreSlider { field: 10; }\n";
    assert_ne!(wired_in(old, "Effects"), indices(Block::Effects));
}

#[test]
fn balance_strength_stands_under_its_key_alone_in_the_panel() {
    let panel = panel_source();
    let colour = wired_in(&panel, "Colour");
    assert_eq!(colour.first(), Some(&Field::BalanceStrength.index()), "the dose right under the key: {colour:?}");
    let wired = |text: &str| text.matches(&format!("field: {}; ", Field::BalanceStrength.index())).count();
    assert_eq!(wired(&panel), 1, "one Balance strength in the whole panel");
    // Its reset is the slider's own: the Colour block's reset leaves the roll's share alone.
    assert_eq!(Field::BalanceStrength.block(), None);
    assert_eq!(Field::Temperature.block(), Some(Block::Color));
    // Twin: a second one, as the Roll tab laid it, is counted.
    assert_eq!(wired(&format!("{panel}StoreSlider {{ field: 11; label: @tr(\"Strength\"); }}")), 2);
}

/// Each Store block's `block:` index and the `Store.<name>-modified` flag it shows, as laid.
fn reset_indices(panel: &str) -> Vec<(i32, String)> {
    let mut pending = None;
    let mut out = Vec::new();
    for line in panel.lines().map(str::trim) {
        if line.starts_with("title: @tr(\"") {
            pending = None;
        } else if let Some(n) = line.strip_prefix("block: ").and_then(|t| t.strip_suffix(';')) {
            pending = n.parse().ok();
        } else if let Some(name) = line.strip_prefix("modified: Store.").and_then(|t| t.strip_suffix("-modified;")) {
            out.push((pending.expect("a Store block names its index"), name.to_string()));
        }
    }
    out
}

#[test]
fn each_blocks_reset_key_clears_the_block_it_shows() {
    let named = [
        (Block::Color, "color"),
        (Block::Lighting, "lighting"),
        (Block::Effects, "effects"),
        (Block::DensityCeiling, "density-ceiling"),
        (Block::DyeSeparation, "dye-separation"),
        (Block::Sharpening, "sharpening"),
        (Block::ChromaDenoise, "chroma-denoise"),
        (Block::Texture, "texture"),
        (Block::PaperCorrection, "paper-correction"),
        (Block::SourceBalance, "source-balance"),
        (Block::FlatField, "flat-field"),
    ];
    let of = |name: &str| named.iter().find(|(_, n)| *n == name).map(|(b, _)| *b);
    let laid = reset_indices(&panel_source());
    assert_eq!(laid.len(), Block::ALL.len(), "{laid:?}");
    for (index, name) in &laid {
        assert_eq!(Block::from_index(*index), of(name), "block {index} shows {name}");
    }
    // Twin: Sharpening at the index it held beside a Balance Strength block resets Chroma Denoise.
    assert_ne!(Block::from_index(6), of("sharpening"));
}
