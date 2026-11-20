//! The Kit as the interface shows it: the link, the column's pieces, the sheet's answers and filters, the
//! picks Done sends, and why what is shown may be behind the service.

use super::*;
use crate::KitPair;

impl Ctl {
    pub(in crate::app) fn publish_kit(&self) {
        let Some(w) = self.window() else { return };
        let k = w.global::<KitGlobal>();
        let (body, lens) = (self.kit_default(Kind::Camera), self.kit_default(Kind::Lens));
        let s = self.state.borrow();
        let counts = &s.org.kit.counts;
        let kit = &s.org.kit;
        k.set_link(if kit.signing.is_some() {
            KitLink::Waiting
        } else if kit.token.is_some() {
            KitLink::In
        } else {
            KitLink::Out
        });
        k.set_account(kit.kept.account.clone().unwrap_or_default().into());
        let photos = |p: &Piece| cards::photos(counts.as_ref(), p);
        k.set_bodies(model(kit.shown(Kind::Camera, body.as_deref(), &photos)));
        k.set_lenses(model(kit.shown(Kind::Lens, lens.as_deref(), &photos)));
        let found: Vec<KitPiece> = kit.found.iter().map(|p| piece(p, kit.mark(p), words::found(p), false, -1)).collect();
        let pairs = found.chunks(2).enumerate().map(|(i, two)| KitPair {
            left: two[0].clone(),
            right: two.get(1).cloned().unwrap_or_default(),
            has_right: two.len() == 2,
            at: 2 * i as i32,
        });
        w.global::<KitSearch>().set_pairs(model(pairs.collect()));
        k.set_found(model(found));
        k.set_searched(kit.searched.as_str().into());
        k.set_searching(kit.searching);
        k.set_picking(kit.picking);
        k.set_picked(kit.picks.len() as i32);
        k.set_kind_filter(match kit.kind_filter {
            None => 0,
            Some(Kind::Camera) => 1,
            Some(Kind::Lens) => 2,
        });
        k.set_sending(kit.sending);
        k.set_note(kit.note.as_str().into());
    }
}
