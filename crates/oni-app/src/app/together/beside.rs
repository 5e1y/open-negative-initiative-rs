//! The reset keys over frames shown together: a key also shows while a frame beside that its reset reaches
//! stands off rest by the open frame's own rule, read again once those frames, or its roll or stock, move.

use super::*;
use crate::editors::Reset;
use crate::fields::Block;
use oni_core::Field;
use oni_library::roll_membership::RollKey;
use oni_settings::DyeSet;

/// The reset keys a frame beside the open one stands off rest for: the panel's blocks and the editors', each in its
/// own order.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Beside {
    pub blocks: [bool; Block::ALL.len()],
    pub editors: [bool; crate::editors::RESETS.len()],
}

/// What an answer was read on: the frames together's generation, the open frame's roll and stock.
pub(super) type Read = (u64, Option<RollKey>, Option<DyeSet>);

impl Ctl {
    /// Which reset keys a frame beside the open one stands off rest for; none while it stands alone.
    pub(crate) fn beside(&self) -> Beside {
        let s = self.state.borrow();
        let Some(together) = s.together.as_ref() else { return Beside::default() };
        let e = &s.store;
        let dyes = s.open_dyes();
        let read = (e.together_generation(), e.roll_key(), dyes);
        if let Some((at, beside)) = &*together.beside.borrow()
            && *at == read
        {
            return *beside;
        }
        let block = |b: Block| e.together_where(b.fields()[0].knob().field(), |s| crate::store::is_modified(s, b, dyes));
        let editor = |r: Reset| match r {
            Reset::Curves => e.together_where(Field::ManualCurves, |s| !s.manual_curves.is_neutral()),
            Reset::Levels => e.together_where(Field::Levels, crate::editors::levels_modified),
            Reset::Spectrogram => e.together_where(Field::Spectrogram, |s| s.spectrogram != Default::default()),
            Reset::Saturation => e.together_where(Field::Saturation, |s| !s.saturation.is_neutral()),
            Reset::Grain => e.together_where(Field::Grain, |s| !s.grain.curve.rests()),
        };
        let beside = Beside { blocks: Block::ALL.map(block), editors: crate::editors::RESETS.map(editor) };
        *together.beside.borrow_mut() = Some((read, beside));
        beside
    }
}
