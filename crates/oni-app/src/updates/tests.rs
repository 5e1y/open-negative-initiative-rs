//! The feed, the versions compared, the archives verified and the dialogs' words.

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
    assert_eq!(installed_url("1.3.0", None, "mac"), "https://open-negative-initiative.com/installed?v=1.3.0&os=mac");
    assert_eq!(installed_url("1.3.0", Some("1.2.1"), "windows"), "https://open-negative-initiative.com/installed?v=1.3.0&from=1.2.1&os=windows");
    // The recorder keeps `mac` and `windows` alone: the family is never Darwin's or the machine's name.
    assert_eq!(os(), if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "mac" } else { std::env::consts::OS });
    assert_ne!(os(), "macos", "twin: the raw constant would be dropped by the recorder");
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

/// The real releases, where a copy of them is on this machine: every archive either feed lists
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
    // The Windows feed's archives, where the builds of each version are kept on this machine.
    let (Ok(windows), Some(builds)) = (std::fs::read_to_string(root.join("appcast-windows.xml")), std::env::var_os("ONI_BUILDS_DIR")) else { return };
    for item in parse(&windows).unwrap() {
        let enclosure = item.enclosure.as_ref().expect("a Windows item holds its archive");
        let mut bytes = std::fs::read(std::path::Path::new(&builds).join(&item.display).join("ONI-windows.zip")).unwrap();
        assert_eq!(checked(enclosure, &bytes, PUBLIC_KEY), Ok(()), "{}", enclosure.url);
        bytes[0] ^= 1;
        assert_eq!(checked(enclosure, &bytes, PUBLIC_KEY), Err(IMPROPERLY_SIGNED), "twin: {}", enclosure.url);
    }
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

#[test]
fn windows_reads_its_own_feed_and_is_offered_what_it_finds() {
    assert_eq!(feed_for(true), WINDOWS_FEED);
    assert_eq!(feed_for(false), FEED);
    assert!(WINDOWS_FEED.starts_with("https://") && WINDOWS_FEED != FEED);
    // Both feeds pass through the site, which counts each check, never Pages directly.
    assert_eq!(WINDOWS_FEED.rsplit_once('/').map(|(host, _)| host), FEED.rsplit_once('/').map(|(host, _)| host));
    let windows = system_on(true, false, || panic!("Windows reads no Mac version"));
    let item = |version: &str| Item {
        title: version.into(),
        version: version.into(),
        display: version.into(),
        minimum_system: None,
        link: None,
        enclosure: Some(Enclosure { url: "https://x/ONI-windows.zip".into(), length: Some(1), signature: "s".into() }),
    };
    let items = [item("10657"), item("10661")];
    assert_eq!(newest(&items, "10660", windows.as_deref()).map(|i| i.version), Some("10661".into()));
    assert_eq!(system_on(false, true, || Some("15.1".into())).as_deref(), Some("15.1"));
    // Twins: a system with no feed of its own is offered nothing, nor is a build already the newest.
    assert_eq!(newest(&items, "10660", system_on(false, false, || Some("15.1".into())).as_deref()), None);
    assert_eq!(newest(&items, "10661", windows.as_deref()), None);
}
