//! Presets in this format: an earlier file brought once, verified, a newer one left; share codes
//! `ONI2:` over this format, `ONI1:` read through the settings' chain for ever.

use super::*;

/// `json`, raw-deflated, checked and wrapped under `prefix`, as each version builds a code.
fn coded(prefix: &str, json: &str) -> String {
    use base64::Engine;
    use std::io::Write;
    let mut deflater = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    deflater.write_all(json.as_bytes()).unwrap();
    let mut packed = deflater.finish().unwrap();
    packed.extend_from_slice(&checksum(json.as_bytes()));
    format!("{prefix}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&packed))
}

const KEYPAD: &str = concat!(
    r#"{"preset":{"id":"3676F1B6-B28F-4B26-9125-71C8039CD81B","name":"Keys","note":"","parameters":["sharpen"],"tint":"teal"},"#,
    r#""settings":{"scanner":{"d":3,"halves":{"d":1}},"sharpen":{"amount":1.5}},"version":19}"#
);

#[test]
fn an_earlier_code_reads_through_the_chain_and_a_new_one_is_of_this_format() {
    let read = from_share_code(&coded(EARLIER_PREFIX, KEYPAD)).unwrap();
    assert_eq!((read.name.as_str(), read.settings.sharpen.amount, read.settings.scanner.d), ("Keys", 1.5, 7));
    // Twin: the same body read as this format's, without the chain, loses its half step.
    let typed = KEYPAD.replace(r#""version":19"#, &format!(r#""format":"{FORMAT}","version":{VERSION}"#));
    let as_current = from_share_code(&coded(SHARE_CODE_PREFIX, &typed)).unwrap();
    assert_ne!(as_current.settings.scanner.d, 7);
    // A new code says its format, and reads back as the same look, its keypad in steps.
    let mut keyed = read.clone();
    keyed.parameters = BTreeSet::from(["sharpen".to_string()]);
    let code = share_code(&keyed).unwrap();
    assert!(code.starts_with("ONI2:"), "{code}");
    assert!(from_share_code(&code).unwrap().same_look(&keyed));
}

#[test]
fn an_earlier_preset_file_is_brought_once_and_a_newer_one_left() {
    let dir = folder("format");
    std::fs::create_dir_all(&dir).unwrap();
    let earlier = dir.join("Keys.json");
    std::fs::write(&earlier, KEYPAD).unwrap();
    let before = load(&dir);
    assert_eq!(brought(&dir), 1);
    let text = std::fs::read_to_string(&earlier).unwrap();
    assert!(text.contains(&format!(r#""format": "{FORMAT}""#)), "{text}");
    assert_eq!(load(&dir), before, "the same preset, read again from this format");
    assert_eq!(brought(&dir), 0, "once");
    // Twin: a preset file a newer version wrote is skipped, never rewritten.
    let newer = dir.join("Later.json");
    let later = text.replace(&format!(r#""version": {VERSION}"#), &format!(r#""version": {}"#, VERSION + 1));
    std::fs::write(&newer, &later).unwrap();
    assert_eq!(brought(&dir), 0);
    assert_eq!(std::fs::read_to_string(&newer).unwrap(), later);
    assert_eq!(load(&dir).len(), 1, "the newer one is not listed");
    let _ = std::fs::remove_dir_all(&dir);
}
