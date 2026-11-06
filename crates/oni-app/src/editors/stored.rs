//! The editors' edits on the stored settings: on the face edited, and a block's reset at each frame's
//! own rest.

use super::{Edit, Reset};
use oni_core::Each;
use oni_settings::PipelineSettings;
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::levels::{LevelsChannel, LevelsSet};
use oni_settings::spectrogram::Spectrogram;

/// The settings as the open face edits them: the Scanner face reads its own finishing.
pub fn face(settings: &PipelineSettings, scanner: bool) -> PipelineSettings {
    if scanner { settings.as_scanner() } else { settings.clone() }
}

/// The stored settings with `edit` laid on the face, the other face's finishing kept.
pub fn lay(settings: &PipelineSettings, scanner: bool, edit: &Edit) -> PipelineSettings {
    let mut f = face(settings, scanner);
    match edit {
        Edit::Curves(c) => f.manual_curves = c.clone(),
        Edit::Levels(l) => f.levels = *l,
        Edit::Spectrogram(s) => f.spectrogram = *s,
        Edit::Saturation(s) => f.saturation = s.clone(),
    }
    if scanner { settings.storing_scanner_edit(&f) } else { f }
}

/// `restingLevels`: a measured roll's window when the frame stands on one, else the bare axis.
pub fn resting_levels(settings: &PipelineSettings) -> LevelsSet {
    match &settings.roll_reference {
        Some(reference) => oni_core::balance::written(reference, settings).levels,
        None => LevelsSet::neutral(settings.mode, settings.density_ceiling),
    }
}

/// The Per-Channel Levels block's three windows at their rest, the linked and luma ones kept.
pub fn rested_levels(settings: &PipelineSettings) -> LevelsSet {
    let rest = resting_levels(settings);
    let mut out = settings.levels;
    for c in LevelsChannel::PER_CHANNEL {
        out.set(c, rest.get(c));
    }
    out
}

/// The editors' reset keys, in the interface's order.
pub const RESETS: [Reset; 4] = [Reset::Curves, Reset::Levels, Reset::Spectrogram, Reset::Saturation];

/// The block's reset on the face edited, read off each frame it lands on.
pub fn rest_of(block: Reset, scanner: bool) -> Each {
    Each::new(move |s| {
        let edit = match block {
            Reset::Curves => Edit::Curves(CurveSet::default()),
            Reset::Levels => Edit::Levels(rested_levels(&face(s, scanner))),
            Reset::Spectrogram => Edit::Spectrogram(Spectrogram::default()),
            Reset::Saturation => Edit::Saturation(SaturationCurve::default()),
        };
        lay(s, scanner, &edit)
    })
}

#[cfg(test)]
mod tests {
    use super::super::geometry::Size;
    use super::super::{Gestures, Plot, Reset, levels};
    use oni_core::{Archive, EditorStore};
    use oni_settings::cg::Point;
    use oni_settings::{ConversionMode, PipelineSettings};
    use oni_settings::levels::LevelsChannel;
    use oni_settings::sidecar;
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn slide(white: f32) -> PipelineSettings {
        let mut s = PipelineSettings::rest(ConversionMode::Positive, false);
        s.levels.red.white = white;
        s
    }

    #[test]
    fn a_handle_s_drag_moves_each_slide_by_its_distance_and_a_reset_reaches_a_frame_the_open_one_left_at_rest() {
        let dir = std::env::temp_dir().join(format!("oni-app-editors-together-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let [a, b] = ["a.tif", "b.tif"].map(|n| dir.join(n).to_string_lossy().into_owned());
        let mut theirs = slide(2.2);
        theirs.manual_curves.red.add(Point::new(0.5, 0.7));
        for (p, s) in [(&a, slide(2.4)), (&b, theirs)] {
            std::fs::write(p, b"").unwrap();
            sidecar::write(&s, Path::new(p), Default::default()).unwrap();
        }
        let read = |p: &str| sidecar::read(Path::new(p), false, true).unwrap().settings;
        let mut store = EditorStore::new(Archive::new(vec![a.clone(), b.clone()], |_: &Path| false));
        store.open(&a).unwrap();
        store.set_together(vec![b.clone()]);
        let mut g = Gestures::default();
        let track = Size::new(308.0, 34.0);
        g.sizes[Plot::Levels as usize] = track;
        let (red, mode) = (LevelsChannel::Red, store.settings().mode);
        let white = levels::knob_x(levels::Grip::White, &store.settings().levels.get(red), red, mode, track.w);
        let t0 = Instant::now();
        g.press(&mut store, Plot::Levels, (white, 17.0), t0);
        g.drag(&mut store, Plot::Levels, (white - 30.0, 17.0), false, t0 + Duration::from_millis(40));
        g.release(&mut store, Plot::Levels);
        store.release();
        let moved = store.settings().levels.red.white - 2.4;
        assert!(moved < -0.01, "{moved}");
        assert!((read(&b).levels.red.white - (2.2 + moved)).abs() < 1e-5, "its own Auto levels, moved: {}", read(&b).levels.red.white);
        // Twin: the window laid as the open slide's value would sit where the open one does.
        assert!((read(&b).levels.red.white - store.settings().levels.red.white).abs() > 0.1);
        assert!(store.settings().manual_curves.is_neutral() && !read(&b).manual_curves.is_neutral(), "twin: its own curve, the open one at rest");
        g.reset(&mut store, Reset::Curves);
        store.release();
        assert!(read(&b).manual_curves.is_neutral(), "the reset reaches it all the same");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
