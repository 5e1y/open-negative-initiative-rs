//! `BugReport` and `ReportSeal`: the hand's message, the app's version and the console log sealed to
//! the maintainers' X25519 key, posted to the site's `/report`, which opens a public issue.

use base64::Engine;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use std::io::{Read, Write};
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

/// Matches the site's own bound, so an accepted message is never refused on arrival.
pub const MESSAGE_LIMIT: usize = 5_000;
/// Where the community's reports show.
pub const ISSUES: &str = "https://github.com/5e1y/open-negative-initiative-releases/issues";
/// The feed's host, whose `/report` takes the post.
pub const ENDPOINT: &str = "https://open-negative-initiative.com/report";
/// The maintainers' public key; its private half never ships.
pub const PUBLIC_KEY: &str = "THO8Dl7aIWKvLe/Nxi4ZK/HrjMM6/iLGqKOydzmbshQ=";
pub const PREFIX: &str = "oni-log-v1:";
/// Leaves room for a 5 000-character message under GitHub's 65 536-character issue body.
pub const BLOB_LIMIT: usize = 58_000;
const INFO: &[u8] = b"ONI report log v1";

/// Swift's key from its raw base64.
pub fn recipient(raw: &str) -> Option<PublicKey> {
    let bytes: [u8; 32] = base64::engine::general_purpose::STANDARD.decode(raw).ok()?.try_into().ok()?;
    Some(PublicKey::from(bytes))
}

/// `HKDF<SHA256>` over the shared secret, salted by both public keys, as CryptoKit derives it.
fn derived(shared: &[u8; 32], sender: &PublicKey, recipient: &PublicKey) -> [u8; 32] {
    let salt = [sender.as_bytes().as_slice(), recipient.as_bytes().as_slice()].concat();
    let mut key = [0u8; 32];
    hkdf::Hkdf::<sha2::Sha256>::new(Some(&salt), shared).expand(INFO, &mut key).expect("32 bytes is a valid length");
    key
}

/// Raw deflate, then sealed with a key of its own: two reports share no secret.
pub fn seal(text: &str, recipient: &PublicKey) -> Result<String, String> {
    let mut deflater = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    deflater.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    let packed = deflater.finish().map_err(|e| e.to_string())?;
    let ephemeral = EphemeralSecret::random_from_rng(OsRng);
    let sender = PublicKey::from(&ephemeral);
    let shared = ephemeral.diffie_hellman(recipient);
    let cipher = ChaCha20Poly1305::new(&derived(shared.as_bytes(), &sender, recipient).into());
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, packed.as_slice()).map_err(|e| e.to_string())?;
    let blob = [sender.as_bytes().as_slice(), nonce.as_slice(), sealed.as_slice()].concat();
    Ok(format!("{PREFIX}{}", base64::engine::general_purpose::STANDARD.encode(blob)))
}

/// The maintainers' side, for checks: a blob opened with the private key.
pub fn open(blob: &str, key: &StaticSecret) -> Option<String> {
    let raw = base64::engine::general_purpose::STANDARD.decode(blob.trim().strip_prefix(PREFIX)?).ok()?;
    if raw.len() <= 32 + 12 {
        return None;
    }
    let sender = PublicKey::from(<[u8; 32]>::try_from(&raw[..32]).ok()?);
    let shared = key.diffie_hellman(&sender);
    let cipher = ChaCha20Poly1305::new(&derived(shared.as_bytes(), &sender, &PublicKey::from(key)).into());
    let packed = cipher.decrypt(Nonce::from_slice(&raw[32..44]), &raw[44..]).ok()?;
    let mut text = String::new();
    flate2::read::DeflateDecoder::new(packed.as_slice()).read_to_string(&mut text).ok()?;
    Some(text)
}

/// Cut from its oldest end, a quarter of the lines at a time, until the seal fits.
pub fn fitted(text: &str, recipient: &PublicKey) -> Result<String, String> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    loop {
        let blob = seal(&lines.join("\n"), recipient)?;
        if blob.len() <= BLOB_LIMIT {
            return Ok(blob);
        }
        if lines.len() <= 1 {
            return Err("the log is too large to seal".into());
        }
        let cut = (lines.len() / 4).max(1);
        lines.drain(..cut);
    }
}

/// The session's console with no path left in it, sealed; `None` when it cannot be read or sealed.
pub fn sealed_log(log: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(log).ok()?;
    let home = std::env::var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).unwrap_or_default();
    fitted(&crate::anonymity::scrubbed(&text, &home), &recipient(PUBLIC_KEY)?).ok()
}

/// `BugReport.Failure`'s words.
pub fn failure(code: u16) -> String {
    match code {
        429 => "Too many reports right now. Try again in an hour.".into(),
        code => format!("The report was not accepted ({code}). Try again later."),
    }
}

/// The post's body: the message cut to the site's bound, the version, the sealed log when ticked.
pub fn body(message: &str, version: &str, log: Option<&str>) -> String {
    let mut fields = serde_json::Map::new();
    fields.insert("message".into(), message.chars().take(MESSAGE_LIMIT).collect::<String>().into());
    fields.insert("version".into(), version.into());
    if let Some(log) = log {
        fields.insert("log".into(), log.into());
    }
    serde_json::Value::Object(fields).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sealed_log_opens_with_the_private_key_alone() {
        let key = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&key);
        let text = "line one\nline two ✓";
        let blob = seal(text, &public).unwrap();
        assert!(blob.starts_with(PREFIX));
        assert_eq!(open(&blob, &key).as_deref(), Some(text));
        assert!(!blob.contains("line"), "the blob carries no readable word");
        // Twins: another key reads nothing, and a blob changed on the way fails as a whole.
        assert_eq!(open(&blob, &StaticSecret::random_from_rng(OsRng)), None);
        let mut bytes = blob.into_bytes();
        let last = bytes.len() - 3;
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        assert_eq!(open(&String::from_utf8(bytes).unwrap(), &key), None);
        // Two seals of one text share nothing.
        assert_ne!(seal(text, &public).unwrap(), seal(text, &public).unwrap());
    }

    #[test]
    fn a_long_log_loses_its_oldest_lines_until_it_fits() {
        let key = StaticSecret::random_from_rng(OsRng);
        // Incompressible lines, so the seal outgrows the limit.
        let lines: Vec<String> = (0..4000).map(|i| format!("{i:05} {:x}", (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))).collect();
        let blob = fitted(&lines.join("\n"), &PublicKey::from(&key)).unwrap();
        assert!(blob.len() <= BLOB_LIMIT);
        let kept = open(&blob, &key).unwrap();
        assert!(kept.ends_with(lines.last().unwrap()), "the newest line stays");
        assert!(!kept.starts_with("00000 "), "twin: the oldest went first");
    }

    #[test]
    fn the_message_is_cut_to_the_sites_bound() {
        let long = "é".repeat(MESSAGE_LIMIT + 10);
        let sent: serde_json::Value = serde_json::from_str(&body(&long, "0.1.0", None)).unwrap();
        assert_eq!(sent["message"].as_str().unwrap().chars().count(), MESSAGE_LIMIT);
        assert!(sent.get("log").is_none(), "an unticked log is absent, not empty");
        assert_eq!(recipient(PUBLIC_KEY).map(|k| k.to_bytes().len()), Some(32));
        assert_eq!(failure(429), "Too many reports right now. Try again in an hour.");
        assert_eq!(failure(500), "The report was not accepted (500). Try again later.");
    }

    /// Against CryptoKit itself: `ONI_SEAL_CROSS` names a tool built from the Swift `ReportSeal`'s
    /// steps, which opens a Rust seal and seals one the Swift way for Rust to open.
    #[test]
    fn a_seal_crosses_to_cryptokit_and_back() {
        let Some(tool) = std::env::var_os("ONI_SEAL_CROSS") else { return eprintln!("ONI_SEAL_CROSS unset: CryptoKit is not consulted") };
        let key = StaticSecret::random_from_rng(OsRng);
        let text = "Rust to CryptoKit ✓\nsecond line";
        let sealed_at = std::env::temp_dir().join(format!("oni-seal-{}.txt", std::process::id()));
        std::fs::write(&sealed_at, seal(text, &PublicKey::from(&key)).unwrap()).unwrap();
        let raw = base64::engine::general_purpose::STANDARD.encode(key.to_bytes());
        let out = std::process::Command::new(tool).args([raw.as_str(), sealed_at.to_str().unwrap(), "CryptoKit to Rust"]).output().unwrap();
        let out = String::from_utf8(out.stdout).unwrap();
        let opened: String = out.split("OPENED:").nth(1).unwrap().split("SEALED:").next().unwrap().trim_end().to_string();
        assert_eq!(opened, text, "CryptoKit opens what Rust sealed");
        let sealed = out.split("SEALED:").nth(1).unwrap().trim();
        assert_eq!(open(sealed, &key).as_deref(), Some("CryptoKit to Rust"));
        assert_eq!(open(sealed, &StaticSecret::random_from_rng(OsRng)), None, "twin: not with another key");
        let _ = std::fs::remove_file(sealed_at);
    }
}
