//! The welcome's film on its stage: a page with a film plays it in a loop on the rig's clock, the cover
//! plays none, Reduce Motion stands on the last frame, and the sheet closing drops the player.

use oni_app::app::{Films, Options, Shell};
use oni_app::tutorial::{Frames, synthetic};
use oni_app::welcome::Film;
use oni_app::{Help, Motion};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The places of the bar in the stand-in films: ten frames, each one moved.
const PLACES: [u16; 10] = [0, 6, 12, 18, 24, 30, 36, 42, 48, 54];

/// Moves the rig's clock by `span` a frame's fraction at a time, the decoder given real time to keep up.
fn watch(rig: &common::Rig, span: Duration) {
    let step = Duration::from_millis(10);
    for _ in 0..span.as_millis() / step.as_millis() {
        std::thread::sleep(Duration::from_millis(1));
        rig.pump();
        rig.advance(step);
    }
}

/// The film's first and last pictures, as the decoder lays them.
fn ends(clip: &'static [u8]) -> (Vec<u8>, Vec<u8>) {
    let mut frames = Frames::new(clip).expect("a GIF");
    let first = frames.next().expect("a first frame").0.to_vec();
    while frames.next().is_some() {}
    (first, frames.picture().to_vec())
}

pub(crate) fn the_welcome_plays_its_pages_film_and_drops_it_as_it_closes() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the welcome's film is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-welcome-clip-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let interface: &'static [u8] = Box::leak(synthetic(64, 48, &PLACES).into_boxed_slice());
    let dedust: &'static [u8] = Box::leak(synthetic(80, 60, &PLACES).into_boxed_slice());
    let films: Films = Box::new(move |film| Some(if film == Film::Interface { interface } else { dedust }));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), films, ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let help = shell.window.global::<Help>();
    let size = || {
        let s = help.get_welcome_clip().size();
        (s.width, s.height)
    };
    let pixels = || help.get_welcome_clip().to_rgba8().map(|b| b.as_bytes().to_vec());
    let revision = || help.get_welcome_clip_revision();

    // The cover: no film, and the clock moving asks no frame.
    shell.welcome_if_due();
    assert!(help.get_welcome_shown());
    let opened = revision();
    watch(&rig, Duration::from_millis(500));
    assert_eq!((size(), revision()), ((0, 0), opened), "the cover plays no film");

    // Page 1 plays its film at its size, past its last frame: it loops while the page shows.
    help.invoke_welcome_turned(1);
    rig.until("the interface film's first frame", Duration::from_secs(5), || size() == (64, 48));
    let first = revision();
    watch(&rig, Duration::from_millis(1500));
    let shown = revision() - first;
    assert!(shown > PLACES.len() as i32, "the film plays on the rig's clock and starts again: {shown} frames shown");

    // Twin: back on the cover the film goes and the revision stands.
    help.invoke_welcome_turned(0);
    rig.pump();
    let standing = revision();
    watch(&rig, Duration::from_millis(500));
    assert_eq!((size(), revision()), ((0, 0), standing), "on the cover the revision stands");

    // Page 2 plays its own film.
    help.invoke_welcome_turned(2);
    rig.until("the De-dust film's first frame", Duration::from_secs(5), || size() == (80, 60));

    // Reduce Motion: the film's last frame, still.
    shell.window.global::<Motion>().set_reduced(true);
    help.invoke_welcome_turned(1);
    rig.until("the interface film's still", Duration::from_secs(5), || size() == (64, 48));
    let still = revision();
    watch(&rig, Duration::from_millis(1000));
    assert_eq!(revision(), still, "under Reduce Motion the film stands still");
    let (opening, last) = ends(interface);
    assert_eq!(pixels(), Some(last), "on its last frame");
    assert_ne!(pixels(), Some(opening), "twin: not on its first");
    shell.window.global::<Motion>().set_reduced(false);

    // Closing the sheet drops the player: no picture, no frame asked after.
    help.invoke_welcome_turned(2);
    rig.until("the De-dust film again", Duration::from_secs(5), || size() == (80, 60));
    help.invoke_welcome_closed();
    rig.pump();
    let closed = revision();
    watch(&rig, Duration::from_millis(500));
    assert_eq!((size(), revision()), ((0, 0), closed), "the sheet closed holds no film and asks no frame");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
