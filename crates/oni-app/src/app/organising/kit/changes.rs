//! The Kit changed on the service: the sheet's picks sent at its Done, a removal sent once confirmed, each
//! in one call whose answer replaces the copy.

use super::*;

impl KitState {
    /// The sheet's words, answers, filters and picks forgotten, as a sheet opened again starts bare.
    pub(super) fn clear_search(&mut self) {
        self.asked += 1;
        (self.found, self.picks) = (Vec::new(), Vec::new());
        self.searching = false;
        self.searched.clear();
        self.typed.clear();
        self.kind_filter = None;
    }
}

impl Ctl {
    /// The gear sheet opened bare, on `kind` (0 both, 1 bodies, 2 lenses): a body's field opens it on the bodies.
    pub(super) fn kit_open_search(&self, kind: i32) {
        {
            let mut s = self.state.borrow_mut();
            s.org.kit.clear_search();
            s.org.kit.kind_filter = [None, Some(Kind::Camera), Some(Kind::Lens)][kind.clamp(0, 2) as usize];
            s.org.kit.picking = true;
        }
        self.publish_kit();
    }

    fn kit_close_search(&self) {
        {
            let mut s = self.state.borrow_mut();
            s.org.kit.clear_search();
            s.org.kit.picking = false;
        }
        self.publish_kit();
    }

    /// A search's answer picked for the Kit, or let go when taken again.
    pub(super) fn kit_take(&self, at: usize) {
        {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            let Some(piece) = kit.found.get(at).cloned() else { return };
            match kit.mark(&piece) {
                KitMark::Free => kit.picks.push(piece),
                KitMark::Added => kit.picks.retain(|p| p.key() != piece.key()),
                KitMark::Kept => return,
            }
        }
        self.publish_kit();
    }

    /// The sheet's Done, and its Escape: the picks sent in one change, the sheet closed once the service
    /// holds them. With none, or signed out, it just closes; while the service asks to wait, it says so.
    pub(super) fn kit_done(self: &Rc<Self>) {
        let (picks, held) = {
            let s = self.state.borrow();
            if s.org.kit.sending {
                return;
            }
            (s.org.kit.picks.iter().map(Piece::key).collect::<Vec<_>>(), s.org.kit.held())
        };
        if picks.is_empty() || self.client().is_none() {
            return self.kit_close_search();
        }
        if let Some(left) = held {
            self.state.borrow_mut().org.kit.note = self.failure_note(&Failure::Wait(left.as_secs().max(1)));
            return self.publish_kit();
        }
        self.kit_change(picks, Vec::new(), true);
    }

    /// A removal erases what the person noted of the piece on The Lens DB: asked first, then sent at once.
    /// A pick, listed after the service's pieces, goes at once: nothing of it was sent.
    pub(super) fn kit_remove(&self, kind: GearKind, at: usize) {
        let kind = if kind == GearKind::Body { Kind::Camera } else { Kind::Lens };
        let held = self.state.borrow().org.kit.kept.of(kind).count();
        if at >= held {
            {
                let mut s = self.state.borrow_mut();
                let picks = &mut s.org.kit.picks;
                let Some(i) = picks.iter().enumerate().filter(|(_, p)| p.kind == kind).nth(at - held).map(|(i, _)| i) else { return };
                picks.remove(i);
            }
            return self.publish_kit();
        }
        let Some(key) = self.state.borrow().org.kit.kept.of(kind).nth(at).map(Piece::key) else { return };
        let message = tr("The piece leaves your Kit on The Lens DB, with the condition, price, serial number and notes kept there for it.");
        self.ask(tr("Remove From Your Kit?"), message, tr("Remove"), true, Pending::Run(Box::new(move |c: &Rc<Ctl>| c.kit_change(Vec::new(), vec![key], false))));
    }

    /// One change sent; `closing` the sheet's Done, which closes it once the service answers.
    fn kit_change(&self, add: Vec<Key>, remove: Vec<Key>, closing: bool) {
        let Some(client) = self.client() else { return };
        {
            let mut s = self.state.borrow_mut();
            if s.org.kit.sending {
                return;
            }
            s.org.kit.sending = true;
        }
        self.publish_kit();
        std::thread::spawn(move || {
            let answer = client.change(&add, &remove);
            with_shell(move |c| c.kit_changed(answer, closing));
        });
    }

    /// Kept, the copy is the service's answer. Signed out, the copy stays and the picks go with the sheet;
    /// unreached, waiting or refused, the sheet stays with its picks, so Done sends them again.
    fn kit_changed(&self, answer: Result<(oni_tldb::Kit, Option<String>), Failure>, closing: bool) {
        self.state.borrow_mut().org.kit.sending = false;
        let close = match answer {
            Ok((kit, etag)) => {
                self.kit_keep(kit, etag);
                closing
            }
            Err(f) => {
                let out = f == Failure::SignedOut;
                self.kit_failed(f);
                closing && out
            }
        };
        if close {
            return self.kit_close_search();
        }
        self.publish_kit();
    }
}
