//! The shared engine's uploads: through the surface's gate, never behind a render.
use super::*;
use std::sync::mpsc;
use std::time::Duration;

/// An upload from another thread lands while a render holds the engine, and waits while the
/// window's surface is reconfigured; the preview it makes is the one the engine alone makes.
#[test]
fn an_upload_lands_beside_a_held_engine_and_waits_on_the_gate() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the uploads are not exercised");
        return;
    };
    let shared = Engine::shared(&device);
    let (w, h) = (300usize, 200usize);
    let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| [(i % w) as f32 / w as f32, 0.3, 0.5, 1.0]).collect();
    let decoded = || Decoded { linear: Linear { width: w, height: h, pixels: pixels.clone() }, full_width: w as f64, undeclared: None };
    let started = |f: Box<dyn FnOnce() + Send>| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            f();
            let _ = tx.send(());
        });
        rx
    };

    let held = shared.lock().unwrap();
    let other = shared.clone();
    let laid = started(Box::new(move || {
        other.full(Decoded { linear: Linear { width: 4, height: 4, pixels: vec![[0.5; 4]; 16] }, full_width: 4.0, undeclared: None }).unwrap();
    }));
    assert!(laid.recv_timeout(Duration::from_secs(5)).is_ok(), "the upload lands while a render holds the engine");
    // Twin: what needs the engine itself waits for it.
    let other = shared.clone();
    let locked = started(Box::new(move || drop(other.lock().unwrap())));
    assert!(locked.recv_timeout(Duration::from_millis(300)).is_err(), "a lock waits on the held engine");
    drop(held);
    assert!(locked.recv_timeout(Duration::from_secs(5)).is_ok());

    shared.hold();
    let other = shared.clone();
    let gated = started(Box::new(move || drop(other.full(Decoded { linear: Linear { width: 4, height: 4, pixels: vec![[0.5; 4]; 16] }, full_width: 4.0, undeclared: None }).unwrap())));
    assert!(gated.recv_timeout(Duration::from_millis(300)).is_err(), "no upload while the surface is reconfigured");
    shared.release();
    assert!(gated.recv_timeout(Duration::from_secs(5)).is_ok(), "the upload lands once the gate opens");

    let apart = shared.prepare(decoded()).unwrap();
    let mut engine = shared.lock().unwrap();
    let alone = engine.prepare(decoded()).unwrap();
    let read = |e: &mut Engine, n: &Negative| {
        let image = oni_graph::Image::new(&n.preview);
        e.parts().0.render(&image, image.extent(), None).unwrap()
    };
    assert_eq!(read(&mut engine, &apart), read(&mut engine, &alone), "the preview made beside the engine is the engine's own");
}
