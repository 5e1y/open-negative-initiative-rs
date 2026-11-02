//! `Updates`: the feed Sparkle reads, its items compared with this build and each archive checked
//! against the EdDSA key before it is kept. It looks on its own; an update installs once asked for.

use base64::Engine as _;
use oni_i18n::{marked, tr};

pub mod prior;
use std::cmp::Ordering;
use std::time::Duration;

/// `SUFeedURL`: the site proxies the Pages feed byte for byte, so a check is counted.
pub const FEED: &str = "https://open-negative-initiative.com/appcast.xml";
/// Windows' feed, proxied by the site like `FEED`, each item holding the Windows archive alone: no Mac
/// reader of `FEED`, Swift's Sparkle included, ever meets an enclosure it cannot run.
pub const WINDOWS_FEED: &str = "https://open-negative-initiative.com/appcast-windows.xml";
/// The Windows every build runs on at least, DirectX 12's; the Windows feed names no minimum.
const WINDOWS_FLOOR: &str = "10.0";

/// The feed this system reads.
pub fn feed() -> &'static str {
    feed_for(cfg!(windows))
}

fn feed_for(windows: bool) -> &'static str {
    if windows { WINDOWS_FEED } else { FEED }
}
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

/// `/installed?v=…[&from=…]&os=…`: no `from` is a first installation, one is an update carried out;
/// `os` is the system's family alone, which millions of machines share.
pub fn installed_url(version: &str, from: Option<&str>, os: &str) -> String {
    let mut url = url::Url::parse(INSTALLED).expect("the recorder's address");
    url.query_pairs_mut().append_pair("v", version);
    if let Some(from) = from {
        url.query_pairs_mut().append_pair("from", from);
    }
    url.query_pairs_mut().append_pair("os", os);
    url.into()
}

/// The family `installed_url` names: `mac` or `windows`, the recorder keeping nothing else.
pub fn os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "mac",
        other => other,
    }
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

/// The system the app runs on, as `minimumSystemVersion` is written; none where no feed carries
/// an archive it can run.
pub fn system() -> Option<String> {
    system_on(cfg!(windows), cfg!(target_os = "macos"), mac_version)
}

fn system_on(windows: bool, mac: bool, mac_version: impl FnOnce() -> Option<String>) -> Option<String> {
    if windows {
        Some(WINDOWS_FLOOR.to_string())
    } else if mac {
        mac_version()
    } else {
        None
    }
}

fn mac_version() -> Option<String> {
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
mod tests;
