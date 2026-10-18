//! Updates through an injected fetcher, never the network: the feed read on its own, the UPDATE key,
//! Sparkle's dialogs, an archive kept only once its EdDSA signature holds, and the FEEDBACKS key.

use base64::Engine as _;
use oni_app::app::{Options, Shell, Updater};
use oni_app::preferences::Preferences;
use oni_app::{Advice, Bar, Confirm, Help};
use oni_core::{Archive, EditorStore};
use ring::signature::KeyPair;
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::common;

const ARCHIVE_URL: &str = "https://example.invalid/download/v9.1.0/OpenNegative-9.1.0.zip";

fn feed(items: &[(&str, &str, usize, &str)]) -> String {
    let items: String = items
        .iter()
        .map(|(build, display, length, signature)| {
            format!(
                "<item><title>{display}</title><sparkle:version>{build}</sparkle:version><sparkle:shortVersionString>{display}</sparkle:shortVersionString>\
                 <sparkle:minimumSystemVersion>13.0</sparkle:minimumSystemVersion>\
                 <enclosure url=\"{ARCHIVE_URL}\" length=\"{length}\" type=\"application/octet-stream\" sparkle:edSignature=\"{signature}\"/></item>"
            )
        })
        .collect();
    format!("<?xml version=\"1.0\"?><rss xmlns:sparkle=\"http://www.andymatuschak.org/xml-namespaces/sparkle\" version=\"2.0\"><channel>{items}</channel></rss>")
}

pub(crate) fn updates_are_looked_for_on_their_own_shown_on_the_key_and_only_ever_downloaded() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the updates are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-updates-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (support, downloads) = (root.join("Support"), root.join("Downloads"));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);

    // A key pair of the check's own: the archive signed with it, a forged one beside it.
    let rng = ring::rand::SystemRandom::new();
    let pair = ring::signature::Ed25519KeyPair::from_pkcs8(ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap().as_ref()).unwrap();
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let archive = b"PK\x03\x04 Open Negative Initiative 9.1.0".to_vec();
    let signature = b64(pair.sign(&archive).as_ref());
    let mut forged = archive.clone();
    forged[6] ^= 1;

    let asked: Arc<Mutex<Vec<String>>> = Arc::default();
    let served = Arc::new(Mutex::new(feed(&[("1523", "9.1.0", archive.len(), &signature), ("1400", "1.2.0", 1, "AAAA")])));
    let body = Arc::new(Mutex::new(Ok::<Vec<u8>, String>(forged.clone())));
    let revealed: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
    let updater = |automatic: bool| {
        let (asked, served, body, revealed) = (asked.clone(), served.clone(), body.clone(), revealed.clone());
        Updater {
            fetch: Arc::new(move |url: &str| {
                asked.lock().unwrap().push(url.to_string());
                if url == oni_app::updates::FEED { Ok(served.lock().unwrap().clone().into_bytes()) } else { body.lock().unwrap().clone() }
            }),
            downloads: downloads.clone(),
            reveal: Arc::new(move |p: &Path| revealed.lock().unwrap().push(p.to_path_buf())),
            build: "1446".into(),
            system: Some("15.6".into()),
            public_key: b64(pair.public_key().as_ref()),
            automatic,
        }
    };
    let shell = |updater: Option<Updater>| {
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), updates: updater, ..Options::default() };
        let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
        rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
        rig.window.set_size(slint::PhysicalSize::new(1280, 800));
        shell.window.show().unwrap();
        shell
    };

    // Never checked on this machine: the feed is read at once, on its own, and the key names 9.1.0.
    let first = shell(Some(updater(true)));
    let bar = first.window.global::<Bar>();
    rig.until("the scheduled check", Duration::from_secs(10), || bar.get_update_title() != "");
    assert_eq!(bar.get_update_title().as_str(), "UPDATE TO 9.1.0");
    assert_eq!(bar.get_update_help().as_str(), "Download version 9.1.0 · export stays on ⌘E");
    assert_eq!(*asked.lock().unwrap(), [oni_app::updates::FEED]);
    assert!(!first.window.global::<Confirm>().get_shown(), "a check on its own opens no dialog");
    assert!(Preferences::load(&support).string("SULastCheckTime").is_some());
    println!("shot {}", rig.shot("updates-key").display());

    // The key asks again and the dialog offers the download; a forged archive is refused and never kept.
    bar.invoke_act("update".into());
    let confirm = first.window.global::<Confirm>();
    rig.until("the dialog", Duration::from_secs(10), || confirm.get_shown());
    assert_eq!(confirm.get_title().as_str(), "A new version of Open Negative Initiative is available!");
    assert!(confirm.get_message().contains("Open Negative Initiative 9.1.0 is now available\u{2014}you have "));
    assert_eq!(confirm.get_key().as_str(), "DOWNLOAD");
    confirm.invoke_answered(true);
    let advice = first.window.global::<Advice>();
    rig.until("the refusal", Duration::from_secs(10), || advice.get_shown());
    assert_eq!((advice.get_title().as_str(), advice.get_message().as_str()), ("Update Error!", oni_app::updates::IMPROPERLY_SIGNED));
    assert!(!downloads.join("OpenNegative-9.1.0.zip").exists(), "twin: a forged archive never lands on the disk");
    assert!(revealed.lock().unwrap().is_empty());
    advice.invoke_closed();

    // The genuine archive is kept in Downloads, shown, and nothing is installed or opened.
    *body.lock().unwrap() = Ok(archive.clone());
    bar.invoke_act("update".into());
    rig.until("the dialog again", Duration::from_secs(10), || confirm.get_shown());
    confirm.invoke_answered(true);
    let kept = downloads.join("OpenNegative-9.1.0.zip");
    rig.until("the archive kept", Duration::from_secs(10), || !revealed.lock().unwrap().is_empty());
    assert_eq!(std::fs::read(&kept).unwrap(), archive);
    assert_eq!(*revealed.lock().unwrap(), [kept.clone()]);
    assert!(!bar.get_updating());
    // A second download keeps both files rather than writing over the first.
    bar.invoke_act("update".into());
    rig.until("the third dialog", Duration::from_secs(10), || confirm.get_shown());
    confirm.invoke_answered(true);
    rig.until("the second copy", Duration::from_secs(10), || revealed.lock().unwrap().len() == 2);
    assert_eq!(revealed.lock().unwrap()[1], downloads.join("OpenNegative-9.1.0 2.zip"));
    assert_eq!(std::fs::read(&kept).unwrap(), archive);

    // Check for Updates… with a feed that went down: the error, and the key still standing.
    *served.lock().unwrap() = "<rss><channel><item>".into();
    first.ctl.act("check-updates");
    rig.until("the error", Duration::from_secs(10), || advice.get_shown());
    assert_eq!((advice.get_title().as_str(), advice.get_message().as_str()), ("Update Error!", oni_app::updates::CHECK_FAILED));
    assert_eq!(bar.get_update_title().as_str(), "UPDATE TO 9.1.0");
    advice.invoke_closed();
    // Nothing newer than this build: up to date, and the key goes back to export.
    *served.lock().unwrap() = feed(&[("1400", "1.2.0", 1, "AAAA")]);
    first.ctl.act("check-updates");
    rig.until("up to date", Duration::from_secs(10), || advice.get_shown());
    assert_eq!(advice.get_title().as_str(), "You\u{2019}re up-to-date!");
    assert_eq!(bar.get_update_title().as_str(), "");
    advice.invoke_closed();

    // FEEDBACKS opens Report a Bug.
    bar.invoke_act("report".into());
    assert!(first.window.global::<Help>().get_report_shown());
    drop(first);

    // Checked a moment ago: a new launch waits out the interval and asks nothing.
    let asked_before = asked.lock().unwrap().len();
    let second = shell(Some(updater(true)));
    rig.settle();
    assert_eq!(asked.lock().unwrap().len(), asked_before, "twin: a check not due reads nothing");
    drop(second);
    // A development build looks only when asked; with no updater, a check asked for fails offline.
    std::fs::remove_file(support.join("preferences.json")).unwrap();
    let manual = shell(Some(updater(false)));
    rig.settle();
    assert_eq!(asked.lock().unwrap().len(), asked_before);
    drop(manual);
    let offline = shell(None);
    offline.ctl.act("check-updates");
    rig.pump();
    let advice = offline.window.global::<Advice>();
    assert_eq!((advice.get_shown(), advice.get_title().as_str()), (true, "Update Error!"));
    assert_eq!(asked.lock().unwrap().len(), asked_before, "nothing reaches a fetcher that was not handed");
    let _ = std::fs::remove_dir_all(&root);
}
