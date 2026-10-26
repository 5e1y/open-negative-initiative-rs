//! A preset as one line of text, checksummed, and back.

use super::*;

/// The share code's version and marker: a later format gets its own, and this one stays readable.
pub const SHARE_CODE_PREFIX: &str = "ONI1:";

/// The most a code's JSON inflates to: a preset holds a few KB, and deflate's 1000:1 would let a
/// pasted megabyte claim a gigabyte on the interface thread.
pub const MAX_SHARE_JSON: u64 = 1 << 20;

/// The preset as one line: its carried rows laid on neutral settings, every key still at its
/// default dropped, raw deflate plus an FNV-1a check, then base64url; the codec restores the rest.
pub fn share_code(preset: &Preset) -> Result<String, json::Error> {
    use base64::Engine;
    use std::io::Write;
    let carried = Preset { settings: preset.apply(&PipelineSettings::default()), ..preset.clone() };
    let parse = |text: String| serde_json::from_str::<Value>(&text).map_err(|e| json::Error(e.to_string()));
    let mut object = parse(encode(&carried)?)?;
    let neutral = parse(codec::encode_settings(&PipelineSettings::default())?)?;
    let (Some(settings), Some(rest)) = (object.get("settings"), neutral.get("settings")) else {
        return Err(json::Error("a settings document is an object".into()));
    };
    let kept = departures(settings, rest).unwrap_or_else(|| Value::Object(Default::default()));
    object["settings"] = kept;
    let text = sorted_compact(&object);
    let mut deflater = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    deflater.write_all(text.as_bytes()).map_err(|e| json::Error(e.to_string()))?;
    let mut packed = deflater.finish().map_err(|e| json::Error(e.to_string()))?;
    packed.extend_from_slice(&checksum(text.as_bytes()));
    Ok(format!("{SHARE_CODE_PREFIX}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(packed)))
}

/// The preset a code carries, `None` for anything else. Spaces and line breaks a messaging app
/// wraps a long line with are dropped first; a truncated or edited code fails as a whole.
pub fn from_share_code(text: &str) -> Option<Preset> {
    use base64::Engine;
    use std::io::Read;
    let bare: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let body = bare.strip_prefix(SHARE_CODE_PREFIX)?;
    let engine = base64::engine::GeneralPurpose::new(
        &base64::alphabet::URL_SAFE,
        base64::engine::GeneralPurposeConfig::new().with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent),
    );
    let packed = engine.decode(body).ok()?;
    if packed.len() <= 4 {
        return None;
    }
    let (deflated, check) = packed.split_at(packed.len() - 4);
    let mut json = Vec::new();
    flate2::read::DeflateDecoder::new(deflated).take(MAX_SHARE_JSON + 1).read_to_end(&mut json).ok()?;
    if json.len() as u64 > MAX_SHARE_JSON {
        return None;
    }
    (checksum(&json) == check).then(|| decode(&json)).flatten()
}

/// FNV-1a over the JSON, big-endian: raw deflate carries no check of its own.
pub(super) fn checksum(data: &[u8]) -> [u8; 4] {
    let mut hash: u32 = 2_166_136_261;
    for byte in data {
        hash = (hash ^ u32::from(*byte)).wrapping_mul(16_777_619);
    }
    hash.to_be_bytes()
}
