//! The reset keys over frames shown together: a key also shows while a frame beside that its reset reaches
//! stands off rest by the open frame's own rule, read again once those frames, or its roll, stock or face, move.

use super::*;
use super::super::lab::Group;
use crate::editors::Reset;
use crate::fields::Block;
use oni_core::Field;
use oni_library::roll_membership::RollKey;
use oni_settings::blocks::SeparationFamily;

/// The reset keys a frame beside the open one stands off rest for: the panel's blocks, the editors'
/// and the Scanner face's groups, each in its own order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Beside {
    pub blocks: [bool; Block::ALL.len()],
    pub editors: [bool; crate::editors::RESETS.len()],
    pub groups: [bool; Group::ALL.len()],
}

/// What an answer was read on: the frames together's generation, the open frame's roll, stock and face.
pub(super) type Read = (u64, Option<RollKey>, Option<SeparationFamily>, bool);

impl Ctl {
    /// Which reset keys a frame beside the open one stands off rest for; none while it stands alone.
    pub(crate) fn beside(&self) -> Beside {
        let s = self.state.borrow();
        let Some(together) = s.together.as_ref() else { return Beside::default() };
        let e = &s.store;
        let (dyes, scanner) = (s.open_dyes(), e.scanner_face);
        let read = (e.together_generation(), e.roll_key(), dyes, scanner);
        if let Some((at, beside)) = &*together.beside.borrow()
            && *at == read
        {
            return *beside;
        }
        let view = |s: &PipelineSettings| crate::editors::face(s, scanner);
        let block = |b: Block| e.together_where(b.fields()[0].knob().field(), |s| crate::store::is_modified(s, b, dyes));
        let editor = |r: Reset| match r {
            Reset::Curves => e.together_where(Field::ManualCurves, |s| !view(s).manual_curves.is_neutral()),
            Reset::Levels => e.together_where(Field::Levels, |s| crate::editors::rested_levels(&view(s)) != s.levels),
            Reset::Spectrogram => e.together_where(Field::Spectrogram, |s| view(s).spectrogram != Default::default()),
            Reset::Saturation => e.together_where(Field::Saturation, |s| !view(s).saturation.is_neutral()),
        };
        let group = |g: Group| e.together_where(if g == Group::Effects { Field::ScannerFinish } else { Field::Scanner }, |s| g.modified(s));
        let beside = Beside { blocks: Block::ALL.map(block), editors: crate::editors::RESETS.map(editor), groups: Group::ALL.map(group) };
        *together.beside.borrow_mut() = Some((read, beside));
        beside
    }
}
