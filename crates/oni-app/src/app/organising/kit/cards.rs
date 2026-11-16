//! The Kit's pieces as the gear fields offer them, each a card with how many photos name it, before the
//! names typed in ONI that the Kit does not hold.

use super::*;
use crate::GearCard;
use oni_library::index::GearCounts;

/// The photos that name `piece`, -1 while the index cannot tell.
pub(super) fn photos(counts: Option<&GearCounts>, piece: &Piece) -> i32 {
    let Some(counts) = counts else { return -1 };
    let n = if piece.kind == Kind::Camera { counts.body(&piece.name) } else { counts.lens(&piece.name) };
    i32::try_from(n).unwrap_or(i32::MAX)
}

impl Ctl {
    /// The photos naming each piece counted again, as the library index holds them, and every list of
    /// gear shown again with them: at launch and once an export has named gear.
    pub(in crate::app) fn kit_recount(&self) {
        let counted = self.state.borrow().index.as_ref().map(|index| index.gear_counts());
        let counts = counted.and_then(|c| c.map_err(|e| crate::timing::line(&format!("the gear's photos are not counted: {e}"))).ok());
        self.state.borrow_mut().org.kit.counts = counts;
        self.publish_gear();
        self.publish_kit();
    }

    /// The Kit's pieces of one kind as cards, the default marked.
    pub(in crate::app) fn kit_cards(&self, kind: Kind) -> Vec<GearCard> {
        let default = self.kit_default(kind);
        let s = self.state.borrow();
        let counts = &s.org.kit.counts;
        s.org.kit.kept
            .of(kind)
            .map(|p| GearCard {
                name: p.name.as_str().into(),
                detail: words::detail(p).into(),
                preferred: default.as_deref() == Some(p.name.as_str()),
                photos: photos(counts.as_ref(), p),
            })
            .collect()
    }

    /// The names typed in ONI the Kit holds no piece for, in their order.
    pub(in crate::app) fn kit_names(&self, kind: Kind, typed: &[String]) -> Vec<String> {
        let s = self.state.borrow();
        typed.iter().filter(|name| s.org.kit.kept.named(kind, name).is_none()).cloned().collect()
    }
}
