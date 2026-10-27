//! `Updates`: the feed Sparkle reads, its items compared with this build and each archive checked
//! against the EdDSA key before it is kept. It looks on its own; an update installs once asked for.

use base64::Engine as _;
use oni_i18n::{marked, tr};

pub mod prior;
use std::cmp::Ordering;
use std::time::Duration;

/// `SUFeedURL`: the site proxies the Pages feed byte for byte, so a check is counted.
pub const FEED: &str = "https://open-negative-initiative.com/appcast.xml";
/// `SUPublicEDKey`: the public half of the pair the releases are signed with.
pub const PUBLIC_KEY: &str = "Sn54zBsnQ9IN6MZEyfAzYn4fQi03aUihVjaA5Ejn7qM=";
pub const PUBLIC_KEY_BYTES: usize = 32;
/// `SUScheduledCheckInterval`: twelve hours, half Sparkle's default, for a fast release cadence.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(43_200);
/// `SULastCheckTime`, Sparkle's own key, kept under the same name.
pub const LAST_CHECK_KEY: &str = "SULastCheckTime";
/// `SUEnableAutomaticChecks` and `SUAutomaticallyUpdate`: it looks on its own, never installs unasked.
pub const CHECKS_AUTOMATICALLY: bool = true;
pub const INSTALLS_AUTOMATICALLY: bool = false;
const SPARKLE: &str = "http://www.andymatuschak.org/xml-namespaces/sparkle";
const APP: &str = "Open Negative Initiative";

/// `CFBundleShortVersionString`, what the dialogs name: the release stamps it, else the crate's.
pub fn version() -> &'static str {
    option_env!("ONI_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// What every request names itself: the app and its version alone, as Sparkle's agent reads, so
/// the recorder counts a running copy and learns nothing of the Mac.
pub fn user_agent() -> String {
    format!("OpenNegative/{} Sparkle-compatible", version())
}

/// `Updates.reportInstall`'s address on the feed's host, and the version last reported from here.
pub const INSTALLED: &str = "https://open-negative-initiative.com/installed";
pub const LAST_LAUNCHED_KEY: &str = "lastLaunchedVersion";
/// The prior version of a copy that checked before it ever reported: unknown, never guessed.
pub const UNKNOWN_PRIOR: &str = "inconnue";

/// `/installed?v=…[&from=…]`: no `from` is a first installation, one is an update carried out.
pub fn installed_url(version: &str, from: Option<&str>) -> String {
    let mut url = url::Url::parse(INSTALLED).expect("the recorder's address");
    url.query_pairs_mut().append_pair("v", version);
    if let Some(from) = from {
        url.query_pairs_mut().append_pair("from", from);
    }
    url.into()
}

/// `CFBundleVersion`, what the feed is compared with: the release build stamps it, else the
/// repository's sentinel `0`, which every published item outranks.
pub fn build() -> &'static str {
    option_env!("ONI_BUILD").unwrap_or("0")
}

/// A build without its number cannot be published, so it looks only when asked.
pub fn stamped() -> bool {
    build() != "0"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enclosure {
    pub url: String,
    pub length: Option<u64>,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub title: String,
    /// `sparkle:version`, the build number compared.
    pub version: String,
    /// `sparkle:shortVersionString`, the version shown.
    pub display: String,
    pub minimum_system: Option<String>,
    pub link: Option<String>,
    pub enclosure: Option<Enclosure>,
}

/// Every item of the feed, the archive being the item's own enclosure and never a delta's.
pub fn parse(feed: &str) -> Result<Vec<Item>, String> {
    let document = roxmltree::Document::parse(feed).map_err(|e| e.to_string())?;
    let sparkle = |node: roxmltree::Node, name: &str| node.children().find(|c| c.has_tag_name((SPARKLE, name))).and_then(|c| c.text()).map(|t| t.trim().to_string());
    let plain = |node: roxmltree::Node, name: &str| node.children().find(|c| c.tag_name().name() == name && c.tag_name().namespace().is_none()).and_then(|c| c.text()).map(|t| t.trim().to_string());
    Ok(document
        .descendants()
        .filter(|n| n.has_tag_name("item"))
        .filter_map(|item| {
            let enclosure = item.children().find(|c| c.has_tag_name("enclosure"));
            // Older feeds carry the numbers on the enclosure; `generate_appcast` writes them as elements.
            let version = sparkle(item, "version").or_else(|| enclosure.and_then(|e| e.attribute((SPARKLE, "version"))).map(str::to_string))?;
            let display = sparkle(item, "shortVersionString")
                .or_else(|| enclosure.and_then(|e| e.attribute((SPARKLE, "shortVersionString"))).map(str::to_string))
                .unwrap_or_else(|| version.clone());
            Some(Item {
                title: plain(item, "title").unwrap_or_default(),
                version,
                display,
                minimum_system: sparkle(item, "minimumSystemVersion"),
                link: plain(item, "link"),
                enclosure: enclosure.and_then(|e| {
                    Some(Enclosure {
                        url: e.attribute("url")?.to_string(),
                        length: e.attribute("length").and_then(|l| l.parse().ok()),
                        signature: e.attribute((SPARKLE, "edSignature"))?.to_string(),
                    })
                }),
            })
        })
        .collect())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Number,
    Word,
    Separator,
}

fn part_of(c: char) -> Part {
    if c.is_ascii_digit() {
        Part::Number
    } else if c.is_whitespace() || c.is_ascii_punctuation() {
        Part::Separator
    } else {
        Part::Word
    }
}

/// Runs of one kind of character; each separator stands alone.
fn split(version: &str) -> Vec<(Part, String)> {
    let mut parts: Vec<(Part, String)> = Vec::new();
    for c in version.chars() {
        let kind = part_of(c);
        match parts.last_mut() {
            Some((last, text)) if *last == kind && kind != Part::Separator => text.push(c),
            _ => parts.push((kind, c.to_string())),
        }
    }
    parts
}

/// `SUStandardVersionComparator`: numbers by value, a word below a number, a release above its betas.
pub fn compare(a: &str, b: &str) -> Ordering {
    let (pa, pb) = (split(a), split(b));
    for ((ta, xa), (tb, xb)) in pa.iter().zip(&pb) {
        let order = match (ta, tb) {
            (Part::Number, Part::Number) => {
                let (na, nb) = (xa.trim_start_matches('0'), xb.trim_start_matches('0'));
                na.len().cmp(&nb.len()).then_with(|| na.cmp(nb))
            }
            (Part::Word, Part::Word) => xa.cmp(xb),
            (Part::Separator, Part::Separator) => Ordering::Equal,
            (_, Part::Word) => Ordering::Greater,
            (Part::Word, _) => Ordering::Less,
            (Part::Number, _) => Ordering::Greater,
            _ => Ordering::Less,
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    if pa.len() == pb.len() {
        return Ordering::Equal;
    }
    let (longer, shorter, longer_wins) = if pa.len() > pb.len() { (&pa, &pb, Ordering::Greater) } else { (&pb, &pa, Ordering::Less) };
    let missing = longer[shorter.len()].0;
    let last = shorter.last().map(|p| p.0);
    // `1.5` outranks `1.5b3`, and `1.5b` outranks `1.5b.3`; otherwise the longer is the later.
    if missing == Part::Word || (missing == Part::Separator && last == Some(Part::Word)) { longer_wins.reverse() } else { longer_wins }
}

/// The system the app runs on, as `minimumSystemVersion` is written; none where the feed's
/// archives cannot run, which leaves every item out until that system has its own channel.
pub fn system() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let plist = std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist").ok()?;
    let after = plist.split("<key>ProductVersion</key>").nth(1)?;
    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")?;
    Some(after[start..start + end].trim().to_string())
}

/// `didFindValidUpdate`: the latest signed item newer than `build` that this system can run.
pub fn newest(items: &[Item], build: &str, system: Option<&str>) -> Option<Item> {
    let system = system?;
    items
        .iter()
        .filter(|i| i.enclosure.is_some() && compare(&i.version, build) == Ordering::Greater)
        .filter(|i| i.minimum_system.as_deref().is_none_or(|m| compare(system, m) != Ordering::Less))
        .max_by(|a, b| compare(&a.version, &b.version))
        .cloned()
}

/// A missing or truncated key fails silently elsewhere: no archive would ever verify.
pub fn valid_public_key(key: &str) -> bool {
    base64::engine::general_purpose::STANDARD.decode(key).is_ok_and(|k| k.len() == PUBLIC_KEY_BYTES)
}

/// `sparkle:edSignature`: the Ed25519 signature of the archive's bytes, against the public key.
pub fn verify(public_key: &str, signature: &str, archive: &[u8]) -> bool {
    let decode = |text: &str| base64::engine::general_purpose::STANDARD.decode(text.trim()).ok();
    let (Some(key), Some(signature)) = (decode(public_key), decode(signature)) else { return false };
    if key.len() != PUBLIC_KEY_BYTES {
        return false;
    }
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key).verify(archive, &signature).is_ok()
}

/// The archive kept only once its length and its signature hold.
pub fn checked(enclosure: &Enclosure, archive: &[u8], public_key: &str) -> Result<(), &'static str> {
    if enclosure.length.is_some_and(|l| l != archive.len() as u64) || !verify(public_key, &enclosure.signature, archive) {
        return Err(IMPROPERLY_SIGNED);
    }
    Ok(())
}

/// The name the archive is kept under: the link's last segment, never a path out of the folder.
pub fn file_name(url: &str) -> String {
    let name = url.split(['?', '#']).next().unwrap_or("").rsplit('/').next().unwrap_or("");
    let clean: String = name.chars().filter(|c| !matches!(c, '/' | '\\' | ':')).collect();
    if clean.is_empty() || clean.starts_with('.') { "OpenNegative.zip".into() } else { clean }
}

/// `"UPDATE TO 0.2.0"`: the export slot's face while an update waits, and it says to what.
pub fn key_title(display: &str) -> String {
    tr!("UPDATE TO {version}", version = display)
}

pub fn key_help(display: &str) -> String {
    let key = format!("{}E", crate::platform::keys(crate::platform::Platform::THIS).command);
    tr!("Download version {version} · export stays on {key}", version = display, key = key)
}

/// Sparkle's own wording for the dialogs a check that was asked for ends on.
pub fn found(item: &Item) -> (String, String) {
    (
        tr!("A new version of {app} is available!", app = APP),
        tr!("{app} {version} is now available—you have {current}. Would you like to download it now?", app = APP, version = item.display, current = version()),
    )
}

pub fn up_to_date() -> (String, String) {
    (tr("You’re up-to-date!").into(), tr!("{app} {version} is currently the newest version available.", app = APP, version = version()))
}

/// The dialogs' English, translated where shown.
pub const ERROR_TITLE: &str = marked("Update Error!");
pub const CHECK_FAILED: &str = marked("An error occurred in retrieving update information. Please try again later.");
pub const DOWNLOAD_FAILED: &str = marked("An error occurred while downloading the update. Please try again later.");
pub const IMPROPERLY_SIGNED: &str = marked("The update is improperly signed and could not be validated. Please try again later or contact the app developer.");

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::KeyPair;

    const FEED_TEXT: &str = r#"<?xml version="1.0" standalone="yes"?>
<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" version="2.0">
    <channel>
        <title>Open Negative Initiative</title>
        <item>
            <title>1.3.1</title>
            <link>https://github.com/5e1y/open-negative-initiative-releases</link>
            <sparkle:version>1523</sparkle:version>
            <sparkle:shortVersionString>1.3.1</sparkle:shortVersionString>
            <sparkle:minimumSystemVersion>13.0</sparkle:minimumSystemVersion>
            <description><![CDATA[<p>Notes</p>]]></description>
            <enclosure url="https://example.invalid/v1.3.1/OpenNegative-1.3.1.zip" length="25583533" type="application/octet-stream" sparkle:edSignature="dKvZfk7Wc9ZN+d6ILqTUWZIuYK8J3NZ9H+DTXrkqHdtTBTTXvH4c6QYBUCaiPBdb9JKYSRbFI/zxB+wQQB/qDw=="/>
            <sparkle:deltas>
                <enclosure url="https://example.invalid/v1.3.1/OpenNegativeInitiative1523-1446.delta" sparkle:deltaFrom="1446" length="11741674" type="application/octet-stream" sparkle:edSignature="VVXW"/>
            </sparkle:deltas>
        </item>
        <item>
            <title>2.0.0</title>
            <sparkle:version>1600</sparkle:version>
            <sparkle:shortVersionString>2.0.0</sparkle:shortVersionString>
            <sparkle:minimumSystemVersion>27.0</sparkle:minimumSystemVersion>
            <enclosure url="https://example.invalid/v2/OpenNegative-2.0.0.zip" length="1" sparkle:edSignature="AAAA"/>
        </item>
        <item>
            <title>1.2.0</title>
            <enclosure url="https://example.invalid/v1.2/OpenNegative-1.2.0.zip" length="9" sparkle:version="1400" sparkle:shortVersionString="1.2.0" sparkle:edSignature="AAAA"/>
        </item>
    </channel>
</rss>"#;

    #[test]
    fn the_feed_reads_its_items_and_the_newest_runnable_one_is_offered() {
        let items = parse(FEED_TEXT).unwrap();
        assert_eq!(items.iter().map(|i| i.version.as_str()).collect::<Vec<_>>(), ["1523", "1600", "1400"]);
        let first = &items[0];
        assert_eq!((first.display.as_str(), first.minimum_system.as_deref()), ("1.3.1", Some("13.0")));
        let enclosure = first.enclosure.as_ref().unwrap();
        assert!(enclosure.url.ends_with("OpenNegative-1.3.1.zip"), "the item's own archive, never a delta");
        assert_eq!(enclosure.length, Some(25_583_533));
        assert_eq!(items[2].display, "1.2.0", "the older spelling, on the enclosure, reads too");
        // 2.0.0 asks for a later system than this one; 1.3.1 is the newest that runs.
        assert_eq!(newest(&items, "1446", Some("15.6")).map(|i| i.display), Some("1.3.1".into()));
        assert_eq!(newest(&items, "1446", Some("27.1")).map(|i| i.display), Some("2.0.0".into()));
        // Twins: the current build is not offered, nor anything on an unknown system.
        assert_eq!(newest(&items, "1600", Some("27.1")), None);
        assert_eq!(newest(&items, "1523", Some("15.6")), None);
        assert_eq!(newest(&items, "0", None), None);
        assert!(parse("<rss><channel><item>").is_err(), "a torn feed is an error, not an empty one");
    }

    #[test]
    fn the_recorder_hears_a_version_and_nothing_of_the_mac() {
        assert_eq!(installed_url("1.3.0", None), "https://open-negative-initiative.com/installed?v=1.3.0");
        assert_eq!(installed_url("1.3.0", Some("1.2.1")), "https://open-negative-initiative.com/installed?v=1.3.0&from=1.2.1");
        assert_eq!(user_agent(), format!("OpenNegative/{} Sparkle-compatible", version()));
        // Twin: the recorder's pattern reads the version back, and the system's name is never in it.
        assert!(user_agent().starts_with(&format!("OpenNegative/{} ", version())));
        assert!(!user_agent().contains("Mac") && !user_agent().contains(std::env::consts::OS));
    }

    #[test]
    fn versions_compare_as_sparkle_compares_them() {
        use Ordering::*;
        for (a, b, want) in [
            ("1523", "1446", Greater),
            ("10", "9", Greater),
            ("1.3.1", "1.3", Greater),
            ("1.5", "1.5b3", Greater),
            ("1.5b3", "1.5b2", Greater),
            ("1.5b", "1.5b.3", Greater),
            ("1.0", "1.0", Equal),
            ("13.0", "15.6", Less),
            ("0", "1", Less),
        ] {
            assert_eq!(compare(a, b), want, "{a} against {b}");
            assert_eq!(compare(b, a), want.reverse(), "{b} against {a}");
        }
        // Twin: compared as text, 10 would fall below 9.
        assert_eq!("10".cmp("9"), Less);
    }

    #[test]
    fn a_signed_archive_verifies_and_a_forged_or_truncated_one_does_not() {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pair = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
        let key = b64(pair.public_key().as_ref());
        let archive = b"PK\x03\x04 the app, zipped".to_vec();
        let signature = b64(pair.sign(&archive).as_ref());
        assert!(valid_public_key(&key) && valid_public_key(PUBLIC_KEY));
        assert!(verify(&key, &signature, &archive));
        let mut forged = archive.clone();
        forged[5] ^= 1;
        assert!(!verify(&key, &signature, &forged), "one byte changed");
        assert!(!verify(&key, &signature, &archive[..archive.len() - 1]), "a truncated archive");
        assert!(!verify(&key, &signature[..signature.len() - 4], &archive), "a truncated signature");
        let other = ring::signature::Ed25519KeyPair::from_pkcs8(ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap().as_ref()).unwrap();
        assert!(!verify(&b64(other.public_key().as_ref()), &signature, &archive), "another key");
        assert!(!verify(PUBLIC_KEY, &signature, &archive), "signed by anyone but the releases' key");
        assert!(!valid_public_key(&PUBLIC_KEY[..PUBLIC_KEY.len() - 4]) && !valid_public_key(""), "a truncated or empty key");
        // Twin: the length check refuses what the signature would have refused anyway, and more.
        let enclosure = Enclosure { url: "x".into(), length: Some(archive.len() as u64 + 1), signature };
        assert_eq!(checked(&enclosure, &archive, &key), Err(IMPROPERLY_SIGNED));
    }

    /// The real releases, where a copy of them is on this machine: every archive the feed lists
    /// verifies against the app's key, and the same archive with one byte changed does not.
    #[test]
    fn the_published_archives_verify_against_the_apps_key() {
        let Some(root) = std::env::var_os("ONI_RELEASES_DIR").map(std::path::PathBuf::from) else {
            return eprintln!("ONI_RELEASES_DIR unset: the published archives are not checked");
        };
        let items = parse(&std::fs::read_to_string(root.join("appcast.xml")).unwrap()).unwrap();
        let mut seen = 0;
        for enclosure in items.iter().filter_map(|i| i.enclosure.as_ref()) {
            let Ok(mut bytes) = std::fs::read(root.join("updates").join(file_name(&enclosure.url))) else { continue };
            assert_eq!(checked(enclosure, &bytes, PUBLIC_KEY), Ok(()), "{}", enclosure.url);
            let middle = bytes.len() / 2;
            bytes[middle] ^= 1;
            assert_eq!(checked(enclosure, &bytes, PUBLIC_KEY), Err(IMPROPERLY_SIGNED), "twin: {}", enclosure.url);
            seen += 1;
        }
        assert!(seen > 0, "no archive of the feed was found beside it");
    }

    #[test]
    fn the_key_and_the_dialogs_say_what_sparkle_says() {
        assert_eq!(key_title("0.2.0"), "UPDATE TO 0.2.0");
        assert!(!key_title("0.2.0").contains("EXPORT"));
        assert_eq!(file_name("https://x/releases/download/v1.3.1/OpenNegative-1.3.1.zip?raw=1"), "OpenNegative-1.3.1.zip");
        assert_eq!(file_name("https://x/../"), "OpenNegative.zip", "never a name that leaves the folder");
        assert_eq!(up_to_date().0, "You\u{2019}re up-to-date!");
        assert!(CHECKS_AUTOMATICALLY && !INSTALLS_AUTOMATICALLY);
        assert!((3_600..=86_400).contains(&CHECK_INTERVAL.as_secs()));
        assert!(FEED.starts_with("https://"));
    }
}
