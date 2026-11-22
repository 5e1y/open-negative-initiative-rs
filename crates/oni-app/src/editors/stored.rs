//! The editors' edits on the stored settings, and a block's reset at each frame's own rest.

use super::levels::Windows;
use super::{Edit, Reset};
use oni_core::Each;
use oni_settings::PipelineSettings;
use oni_settings::blocks::GrainCurve;
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::levels::{LevelsChannel, LevelsSet};
use oni_settings::spectrogram::Spectrogram;

/// The stored settings with `edit` laid on.
pub fn lay(settings: &PipelineSettings, edit: &Edit) -> PipelineSettings {
    let mut f = settings.clone();
    match edit {
        Edit::Curves(c) => f.manual_curves = c.clone(),
        Edit::Levels(l) => l.lay(&mut f),
        Edit::Spectrogram(s) => f.spectrogram = *s,
        Edit::Saturation(s) => f.saturation = s.clone(),
        Edit::Grain(c) => f.grain.curve = *c,
    }
    f
}

/// `restingLevels`: a measured roll's window when the frame stands on one, a digital photo's film's own,
/// else the bare axis.
pub fn resting_levels(settings: &PipelineSettings) -> LevelsSet {
    match (&settings.roll_reference, settings.film()) {
        (Some(reference), _) => oni_core::balance::written(reference, settings).levels,
        (None, Some(film)) => film.stock.reference().levels(),
        (None, None) => LevelsSet::neutral(settings.mode, settings.density_ceiling),
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

/// The frame's own windows at their rest.
pub fn rested_windows(settings: &PipelineSettings) -> Windows {
    Windows(rested_levels(settings))
}

/// True where the frame's own windows stand off their rest.
pub fn levels_modified(settings: &PipelineSettings) -> bool {
    rested_windows(settings) != Windows::of(settings)
}

/// The editors' reset keys, in the interface's order.
pub const RESETS: [Reset; 5] = [Reset::Curves, Reset::Levels, Reset::Spectrogram, Reset::Saturation, Reset::Grain];

/// The block's reset, read off each frame it lands on.
pub fn rest_of(block: Reset) -> Each {
    Each::new(move |s| {
        let edit = match block {
            Reset::Curves => Edit::Curves(CurveSet::default()),
            Reset::Levels => Edit::Levels(rested_windows(s)),
            Reset::Spectrogram => Edit::Spectrogram(Spectrogram::default()),
            Reset::Saturation => Edit::Saturation(SaturationCurve::default()),
            Reset::Grain => Edit::Grain(GrainCurve::REST),
        };
        lay(s, &edit)
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
        let read = |p: &str| sidecar::read(Path::new(p), false).unwrap().settings;
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
        g.release(&mut store);
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
        // The grain curve's reset reaches a frame beside whose own curve moved, the open one at rest.
        let mut bent = read(&b);
        bent.grain.curve.move_point(1, Point::new(0.3, 0.6));
        sidecar::write(&bent, Path::new(&b), Default::default()).unwrap();
        store.set_together(vec![b.clone()]);
        assert!(!read(&b).grain.curve.rests() && store.settings().grain.curve.rests(), "twin: its own curve, the open one at rest");
        g.reset(&mut store, Reset::Grain);
        store.release();
        assert!(read(&b).grain.curve.rests(), "the grain curve's reset reaches it");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
