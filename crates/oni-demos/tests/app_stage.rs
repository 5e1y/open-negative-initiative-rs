//! The app stage films the real window: the canvas shows the photo through the texture it shares
//! with Slint, a bar key lights under the pointer, and a pointer parked changes nothing.

use std::collections::HashSet;
use std::time::Duration;

use i_slint_backend_testing::AccessibleRole;
use oni_demos::app_stage::{AppStage, MINIMUM, Placed, crop};
use oni_demos::photo::Rgba;
use oni_demos::world::World;
use slint::ComponentHandle;

/// Real time the app may take for a step on a GPU; a software adapter is not filmed on.
const MOST: Duration = Duration::from_secs(60);

fn distinct(shot: &Rgba) -> usize {
    shot.pixels.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect::<HashSet<_>>().len()
}

fn inset(p: Placed, by: f32) -> Placed {
    Placed { x: p.x + by, y: p.y + by, width: p.width - 2.0 * by, height: p.height - 2.0 * by }
}

/// Where the photo lies in the window: the canvas card's inner area, offset by the frame's place in it.
fn frame(stage: &AppStage) -> Result<(Placed, Placed), String> {
    let card = *stage.kind("CanvasCard").first().ok_or("no canvas card shown")?;
    let c = stage.window().global::<oni_app::Canvas>();
    let (padding_x, padding_y) = ((card.width - c.get_width()) / 2.0, (card.height - c.get_height()) / 2.0);
    let inner = Placed { x: card.x + padding_x, y: card.y + padding_y, width: c.get_width(), height: c.get_height() };
    let photo = Placed { x: inner.x + c.get_frame_x(), y: inner.y + c.get_frame_y(), width: c.get_frame_width(), height: c.get_frame_height() };
    Ok((inner, photo))
}

#[test]
fn the_stage_shows_the_canvas_lights_a_key_under_the_pointer_and_a_parked_pointer_changes_nothing() {
    if let Err(e) = oni_app::render::Device::open() {
        return eprintln!("SKIPPED the app stage: {e}");
    }
    let world = World::new("check").unwrap();
    let source = world.root.join("source.tif");
    oni_fixtures::tiff::graded_tiff(&source, 360, 240, 0.8);
    world.add(&source, "Frame 1", None).unwrap();
    world.speak(oni_i18n::Language::English).unwrap();
    let mut options = world.options();
    // A bare frame opens on the editor, not on Prepare the Roll.
    options.require_roll = false;
    let mut stage = AppStage::new(options, MINIMUM).unwrap();
    stage.until("the roll read", MOST, |s| s.ctl().state.borrow().read_done()).unwrap();
    stage.idle(MOST).unwrap();

    // A double click on the tile, as a hand gives it, opens the photo on the canvas.
    let tile = stage.the(AccessibleRole::ListItem, "Frame 1").unwrap();
    for down in [true, false, true, false] {
        stage.pointer(tile.middle(), down);
    }
    let opened = |s: &AppStage| s.ctl().state.borrow().renders() >= 1 && s.window().global::<oni_app::Canvas>().get_frame().size().width > 0;
    stage.until("the photo on the canvas", MOST, opened).unwrap();
    stage.pointer((MINIMUM.0 / 2.0, MINIMUM.1 - 4.0), false);
    // What the opening set moving, the load bar's fade among them, lands before the first frame.
    stage.advance(Duration::from_secs(3), MOST).unwrap();
    let before = stage.frame(MOST).unwrap();
    assert_eq!((before.width, before.height), ((MINIMUM.0 * 2.0) as u32, (MINIMUM.1 * 2.0) as u32), "two pixels a point");

    let (inner, photo) = frame(&stage).unwrap();
    let shown = crop(&before, inset(photo, 4.0));
    assert!(distinct(&shown) > 200, "the canvas holds the photo's ramp, {} colours", distinct(&shown));
    // Twin: the ground above the photo, where the texture is not, is one flat colour.
    let ground = Placed { x: inner.x + 4.0, y: inner.y + 2.0, width: inner.width - 8.0, height: (photo.y - inner.y - 4.0).max(1.0) };
    assert!(photo.y - inner.y > 8.0, "the photo leaves ground above it: {photo:?} in {inner:?}");
    assert!(distinct(&crop(&before, ground)) <= 4, "the ground is flat, {} colours", distinct(&crop(&before, ground)));

    // Twin first: the pointer parked while the clock runs leaves the window as it was.
    let parked = { stage.advance(Duration::from_millis(400), MOST).unwrap(); stage.shoot().unwrap() };
    assert!(parked == before, "a parked pointer changes no pixel");
    let key = stage.the(AccessibleRole::Button, "Settings").unwrap();
    stage.pointer(key.middle(), false);
    let lit = { stage.advance(Duration::from_millis(400), MOST).unwrap(); stage.shoot().unwrap() };
    assert!(crop(&lit, key) != crop(&before, key), "the key under the pointer lights");

    if let Some(shots) = std::env::var_os("ONI_SHOTS") {
        let shots = std::path::PathBuf::from(shots);
        before.png(&shots.join("app-stage-editor.png")).unwrap();
        lit.png(&shots.join("app-stage-key-lit.png")).unwrap();
    }
}
