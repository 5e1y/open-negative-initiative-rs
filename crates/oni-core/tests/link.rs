//! The phone's presses through `oni-link`'s Host land on a store opened on a roll #8 frame, and the
//! screen the phone draws is the store's.

use oni_core::link::{Link, Outward, drive};
use oni_core::{Archive, EditorStore, Surface};
use oni_link::{Command, Display, Effect, Host, Identity, Letter, Machine, PairingCode, Remote};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::timeout;

async fn until(display: &mut watch::Receiver<Display>, done: impl Fn(&Display) -> bool) -> Display {
    timeout(Duration::from_secs(5), async {
        loop {
            if done(&display.borrow_and_update()) {
                return display.borrow().clone();
            }
            display.changed().await.expect("remote alive");
        }
    })
    .await
    .expect("display reached the expected state")
}

#[tokio::test]
async fn presses_from_the_phone_edit_the_open_frame_and_its_screen_comes_back() {
    let Some(source) = oni_fixtures::raw_dir("link") else { return };
    let dir = std::env::temp_dir().join(format!("oni-core-link-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(source.join("DSC_6440.NEF.json"), dir.join("DSC_6440.NEF.json")).unwrap();
    let frame = dir.join("DSC_6440.NEF").to_string_lossy().into_owned();

    let mut store = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| true));
    store.open(&frame).unwrap();
    let start = store.settings().clone();
    let store = Arc::new(Mutex::new(store));

    let (host, events) = Host::new(Identity::generate(), "Studio".into(), []);
    let addr = host.listen("127.0.0.1:0".parse().unwrap()).await.unwrap();
    let link = Arc::new(Mutex::new(Link::new(host.clone())));
    {
        let mut link = link.lock().unwrap();
        link.frame = Some((9, 38));
        link.refresh(&store.lock().unwrap());
    }
    let walks = Arc::new(Mutex::new(Vec::new()));
    let (l, s, w) = (link.clone(), store.clone(), walks.clone());
    tokio::spawn(async move {
        drive(events, &l, &s, |out| {
            if let Outward::Walk(by) = out {
                w.lock().unwrap().push(by);
            }
        })
        .await
    });

    let code = PairingCode::parse(&host.open_pairing(vec![addr]).to_uri()).unwrap();
    let remote = Remote::pair(Remote::dial(&code.addresses).await.unwrap(), &Identity::generate(), &code, "Phone")
        .await
        .unwrap();
    remote.press(Command::Step { letter: Letter::D, by: 2 });
    remote.press(Command::Step { letter: Letter::C, by: -1 });
    remote.press(Command::StepEffect { effect: Effect::Vignette, by: 3 });
    remote.press(Command::Machine(Machine::Frontier));
    remote.press(Command::Walk { by: 1 });
    let mut display = remote.display();
    let shown = until(&mut display, |d| d.state.applied == 5).await;

    let (d, c) = (start.scanner.d as i8, start.scanner.c as i8);
    assert_eq!((shown.state.clicks.d, shown.state.clicks.c), (d + 2, c - 1));
    assert!((shown.state.vignette - 0.3).abs() < 1e-6);
    assert_eq!((shown.state.machine, shown.state.frame), (Machine::Frontier, Some((9, 38))));
    assert!(shown.state.can_undo);
    {
        let store = store.lock().unwrap();
        assert_eq!((store.settings().scanner.d, store.settings().scanner.c), (start.scanner.d + 2, start.scanner.c - 1));
        assert_eq!(store.settings().scanner_machine, oni_settings::ScannerMachine::Frontier);
        assert_eq!(shown.state.revision, store.revision(), "the screen is the store's, not the prediction");
    }
    assert_eq!(*walks.lock().unwrap(), vec![1]);

    // Twin: the phone's undo takes every press back, and the screen follows the store there.
    for _ in 0..4 {
        remote.press(Command::Undo);
    }
    let back = until(&mut display, |d| d.state.applied == 9).await;
    assert_eq!((back.state.clicks.d, back.state.clicks.c), (d, c));
    let store = store.lock().unwrap();
    assert_eq!(store.settings(), &start);
    assert!(!store.can_undo(Surface::Editor));
    let _ = std::fs::remove_dir_all(&dir);
}
