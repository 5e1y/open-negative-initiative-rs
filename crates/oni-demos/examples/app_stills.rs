//! `cargo run -p oni-demos --example app_stills -- FOLDER OUT`: the app stage's whole window at its
//! smallest, two pixels a point, in four states, from the first photos of FOLDER copied into a world.

use std::path::{Path, PathBuf};
use std::time::Duration;

use i_slint_backend_testing::AccessibleRole;
use oni_demos::app_stage::{AppStage, MINIMUM};
use oni_demos::world::World;
use oni_settings::sidecar;
use slint::ComponentHandle;

/// Photos copied in: enough to fill the gallery's first rows.
const PHOTOS: usize = 8;
const MOST: Duration = Duration::from_secs(120);
/// Film time for what a step set moving to land, as a hand waits before looking.
const REST: Duration = Duration::from_secs(3);

fn main() {
    if let Err(e) = run() {
        eprintln!("app_stills: {e}");
        std::process::exit(1);
    }
}

fn photos(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
        .map_err(|e| format!("the folder: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| ["nef", "rw2", "dng"].contains(&e.to_string_lossy().to_lowercase().as_str())))
        .collect();
    found.sort();
    found.truncate(PHOTOS);
    Ok(found)
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1).map(PathBuf::from);
    let (folder, out) = (args.next().ok_or("FOLDER OUT")?, args.next().ok_or("FOLDER OUT")?);
    std::fs::create_dir_all(&out).map_err(|e| format!("the output folder: {e}"))?;
    let world = World::new("stills")?;
    for (n, photo) in photos(&folder)?.iter().enumerate() {
        // Each photo keeps its own edit, its sidecar read as the app reads it.
        let settings = sidecar::read(photo, true).ok().map(|l| l.settings);
        world.add(photo, &format!("Frame {:02}", n + 1), settings.as_ref())?;
    }
    world.speak(oni_i18n::Language::English)?;
    let mut stage = AppStage::new(world.options(), MINIMUM)?;
    stage.until("the roll read", MOST, |s| s.ctl().state.borrow().read_done())?;
    stage.until("the thumbnails", MOST, |s| s.busy().is_none())?;
    let shot = |stage: &mut AppStage, name: &str| -> Result<(), String> {
        stage.advance(REST, MOST)?;
        stage.shoot()?.png(&out.join(format!("{name}.png")))
    };
    shot(&mut stage, "1-gallery")?;

    let click = |stage: &mut AppStage, at: (f32, f32), times: usize| {
        for _ in 0..times {
            stage.pointer(at, true);
            stage.pointer(at, false);
        }
    };
    let first = stage.the(AccessibleRole::ListItem, "Frame 01")?;
    click(&mut stage, first.middle(), 2);
    let opened = |s: &AppStage| s.ctl().state.borrow().renders() >= 1 && s.window().global::<oni_app::Canvas>().get_frame().size().width > 0;
    if let Err(e) = stage.until("the photo on the canvas", Duration::from_secs(20), opened) {
        stage.shoot()?.png(&out.join("failed.png"))?;
        return Err(e);
    }
    let colour = stage.the(AccessibleRole::Tab, "Colour")?;
    click(&mut stage, colour.middle(), 1);
    stage.pointer((MINIMUM.0 / 2.0, MINIMUM.1 - 2.0), false);
    shot(&mut stage, "2-editor-colour")?;

    let settings = stage.the(AccessibleRole::Button, "Settings")?;
    stage.pointer(settings.middle(), false);
    shot(&mut stage, "3-key-hovered")?;

    let dedust = stage.the(AccessibleRole::Button, "De-dust")?;
    click(&mut stage, dedust.middle(), 1);
    stage.pointer((MINIMUM.0 / 2.0, MINIMUM.1 - 2.0), false);
    stage.until("De-dust's panel", MOST, |s| s.window().global::<oni_app::PanelState>().get_tab() == oni_app::PanelTab::Dedust)?;
    shot(&mut stage, "4-dedust")?;
    Ok(())
}
