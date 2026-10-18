//! The Help menu on the headless window: What's New's pills read once and kept until the session
//! ends, the welcome sheet once per edition, and Report a Bug through an injected reporter.

use oni_app::app::{Options, Shell};
use oni_app::preferences::Preferences;
use oni_app::{Help, News};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::common;

type Sent = Arc<Mutex<Vec<(String, Option<String>)>>>;

pub(crate) fn the_help_menus_surfaces_behave_as_swifts() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Help menu is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-help-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let support = root.join("Support");
    let log = oni_app::timing::log_path(&support);
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    std::fs::write(&log, "a session's line\n").unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let sent: Sent = Arc::default();
    let refuse = Arc::new(Mutex::new(true));
    let shell = |support: &PathBuf| {
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let (log, refusing) = (sent.clone(), refuse.clone());
        let reporter: oni_app::app::Reporter = Arc::new(move |message: &str, sealed: Option<String>| {
            if *refusing.lock().unwrap() {
                return Err("Too many reports right now. Try again in an hour.".into());
            }
            log.lock().unwrap().push((message.to_string(), sealed));
            Ok(())
        });
        let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), reporter, ..Options::default() };
        let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
        rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
        rig.window.set_size(slint::PhysicalSize::new(1280, 800));
        shell.window.show().unwrap();
        shell
    };

    // What's New: every note news on a fresh Mac; a hover reads one, which keeps its pill until quit.
    let first = shell(&support);
    let news = first.window.global::<News>();
    assert_eq!((news.get_lighting().as_str(), news.get_machine().as_str()), ("UPDATED", "NEW"));
    news.invoke_read("lighting.1.3.1".into());
    rig.pump();
    assert_eq!(news.get_lighting().as_str(), "UPDATED", "read, but kept this session");
    let seen = Preferences::load(&support).value("whatsNewSeen").cloned();
    assert_eq!(seen, Some(serde_json::json!(["lighting.1.3.1"])));

    // The welcome sheet: due on a fresh Mac, closed by Return, not due again.
    assert!(first.ctl.welcome_due());
    first.welcome_if_due();
    let help = first.window.global::<Help>();
    assert!(help.get_welcome_shown());
    rig.key("\r");
    assert!(!help.get_welcome_shown());
    assert_eq!(Preferences::load(&support).string("welcomeSeen"), Some("1.2.1"));

    // Report a Bug: a refused post stays on the sheet with its reason; an accepted one thanks.
    first.ctl.act("report");
    rig.pump();
    assert!(help.get_report_shown() && !help.get_report_sent());
    help.set_report_message("The export stops at the third photo.".into());
    help.invoke_report_send();
    rig.until("the refusal", Duration::from_secs(10), || !help.get_report_sending() && help.get_report_failure() != "");
    assert_eq!(help.get_report_failure().as_str(), "Too many reports right now. Try again in an hour.");
    assert!(!help.get_report_sent(), "twin: a refused report is not thanked");
    *refuse.lock().unwrap() = false;
    help.invoke_report_send();
    rig.until("the post", Duration::from_secs(10), || help.get_report_sent());
    let posted = sent.lock().unwrap().clone();
    assert_eq!(posted.len(), 1);
    assert_eq!(posted[0].0, "The export stops at the third photo.");
    assert!(posted[0].1.as_deref().is_some_and(|l| l.starts_with(oni_app::report::PREFIX)), "the log goes sealed");
    help.invoke_report_closed();
    drop(first);

    // The next launch: the read note is gone, the others stay, and the sheet is not due.
    let second = shell(&support);
    let news = second.window.global::<News>();
    assert_eq!((news.get_lighting().as_str(), news.get_machine().as_str()), ("", "NEW"));
    assert!(!second.ctl.welcome_due());
    second.ctl.act("whats-new");
    rig.pump();
    assert_eq!(news.get_lighting().as_str(), "UPDATED", "Show What's New Again brings every note back");
    let _ = std::fs::remove_dir_all(&root);
}
