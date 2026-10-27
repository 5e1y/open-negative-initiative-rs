//! A measurement lays the custom mask and the dust patches its frame names, as `Pipeline.framed`
//! lays them in front of every reading; a file that is gone lays nothing.

use oni_measure::key::{Decoded, Measurer};
use oni_measure::truncate::Stage;
use oni_pipeline::flat_field::Mask;
use oni_pipeline::resident::Files;
use oni_settings::cg::Point;
use oni_settings::frame::{Correction, FlatFieldMask};
use oni_settings::id::Id;
use oni_settings::PipelineSettings;
use std::sync::Arc;

const SIZE: (u32, u32) = (300, 200);

/// A mask darker on its left, and a bright patch over the whole frame, when they are there.
struct Fake {
    present: bool,
}

impl Files for Fake {
    fn mask(&self, id: &str) -> Option<Mask> {
        let (w, h) = (30u32, 20u32);
        let pixels = (0..w * h).map(|i| { let v = 0.4 + 0.6 * (i % w) as f32 / (w - 1) as f32; [v, v, v, 1.0] }).collect();
        (self.present && id == "panel").then(|| Mask { width: w, height: h, pixels, peaks: [1.0; 3], per_channel: false })
    }

    fn patch(&self, _: Id) -> Option<(u32, u32, Vec<[f32; 4]>)> {
        self.present.then(|| (SIZE.0, SIZE.1, vec![[0.9, 0.9, 0.9, 1.0]; (SIZE.0 * SIZE.1) as usize]))
    }
}

fn measurer(files: Option<Fake>) -> Option<Measurer> {
    let graph = oni_graph::Graph::new().ok()?;
    let m = Measurer::new(
        graph,
        Box::new(|_, _| {
            let pixels = (0..SIZE.0 * SIZE.1).map(|i| [0.2 + (i % 7) as f32 * 0.02, 0.3, 0.25, 1.0]).collect();
            Ok(Decoded { width: SIZE.0, height: SIZE.1, pixels, full_width: f64::from(SIZE.0) })
        }),
        Box::new(|_, _| String::new()),
    );
    Some(match files {
        Some(f) => m.laying(Arc::new(f)),
        None => m,
    })
}

fn plane(m: &Measurer, s: &PipelineSettings) -> Vec<[f32; 4]> {
    m.with("frame", s, |r, copy| r.plane(copy, s, Stage::Levels)).expect("a reading").pixels
}

fn differs(a: &[[f32; 4]], b: &[[f32; 4]]) -> f32 {
    a.iter().zip(b).map(|(p, q)| (0..3).map(|c| (p[c] - q[c]).abs()).fold(0.0, f32::max)).fold(0.0, f32::max)
}

#[test]
fn a_reading_lays_the_frames_own_mask_and_patches_and_nothing_once_their_files_are_gone() {
    let (Some(laying), Some(gone), Some(blind)) = (measurer(Some(Fake { present: true })), measurer(Some(Fake { present: false })), measurer(None)) else {
        return eprintln!("no GPU: the measure's inputs are not exercised");
    };
    let bare = PipelineSettings::default();
    let mut masked = bare.clone();
    masked.flat_field.mask = FlatFieldMask::Custom;
    masked.flat_field.custom = Some("panel".into());
    let mut dusted = bare.clone();
    dusted.corrections = vec![Correction { radius: 20.0, ..Correction::new(vec![Point::new(0.5, 0.5)]) }];
    let rest = plane(&laying, &bare);
    assert!(differs(&plane(&laying, &masked), &rest) > 0.05, "the mask reaches the reading");
    assert!(differs(&plane(&laying, &dusted), &rest) > 0.05, "the patch reaches the reading");
    // Twins: a mask or patch whose file is gone, or a measurer told of no files, lays nothing.
    for m in [&gone, &blind] {
        assert_eq!(differs(&plane(m, &masked), &rest), 0.0);
        assert_eq!(differs(&plane(m, &dusted), &rest), 0.0);
    }
    // Another id than the frame's lays no mask either.
    masked.flat_field.custom = Some("another".into());
    assert_eq!(differs(&plane(&laying, &masked), &rest), 0.0);
}
