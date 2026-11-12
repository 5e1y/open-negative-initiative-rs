//! What the Prepare the Roll check reads and does beside its flow: a frame's sidecar, a draft as an
//! earlier build kept it, a picture read as its scan, a click.

use crate::common;
use oni_settings::prepare::Draft;
use oni_settings::sidecar::{self, DraftBase, RollDraft};
use slint::LogicalPosition;
use slint::platform::{PointerEventButton, WindowEvent};
use std::path::{Path, PathBuf};

pub fn loaded(path: &Path) -> sidecar::Loaded {
    sidecar::read(path, false, true).unwrap()
}

/// `n`'s place in `draft` as a frame's sidecar of format 21 keeps it, its base by path.
pub fn stored(draft: &Draft, n: &str, at: &dyn Fn(&str) -> PathBuf) -> RollDraft {
    let m = draft.frames[n];
    let base = draft.base.as_ref().map(|b| DraftBase { path: at(&b.file).to_string_lossy().into_owned(), x: b.x, y: b.y, radius: b.radius });
    let (step, capture, film, window, flat_field) = (draft.step.clone(), draft.capture.clone(), draft.film, draft.window, draft.flat_field.clone());
    RollDraft { id: draft.id, step, film, capture, pass_order: draft.pass_order, pass_offset: draft.pass_offset, flat_field, base, window, ticked: m.ticked, flagged: m.flagged }
}

/// Whether a picture of a graded scan reads as the scan does: darker at its top left than at its
/// bottom right, as the gradient was written; an inverted picture reads the other way.
pub fn as_scanned(rgba: &[u8], width: usize, height: usize) -> bool {
    let luma = |fx: f32, fy: f32| {
        let at = ((fy * height as f32) as usize * width + (fx * width as f32) as usize) * 4;
        rgba[at..at + 3].iter().map(|v| f32::from(*v)).sum::<f32>()
    };
    luma(0.1, 0.1) + 6.0 < luma(0.9, 0.9)
}

pub fn click(rig: &common::Rig, x: f32, y: f32) {
    let at = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
}
