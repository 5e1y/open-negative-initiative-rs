//! The changes held for Validate: pieces found added, pieces of the Kit marked to leave, all sent in one call.

use super::*;

impl Ctl {
    pub(super) fn kit_add(&self, at: usize) {
        {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            let Some(piece) = kit.found.get(at).cloned() else { return };
            if kit.kept.items.iter().chain(&kit.adding).any(|h| h.key() == piece.key()) {
                return;
            }
            kit.adding.push(piece);
        }
        self.publish_kit();
    }

    pub(super) fn kit_unadd(&self, at: usize) {
        {
            let mut s = self.state.borrow_mut();
            if at < s.org.kit.adding.len() {
                s.org.kit.adding.remove(at);
            }
        }
        self.publish_kit();
    }

    /// A piece's removal held for Validate, or let go when pressed again.
    pub(super) fn kit_remove(&self, kind: GearKind, at: usize) {
        {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            let kind = if kind == GearKind::Body { Kind::Camera } else { Kind::Lens };
            let Some(key) = kit.kept.of(kind).nth(at).map(Piece::key) else { return };
            if let Some(i) = kit.removing.iter().position(|k| *k == key) {
                kit.removing.remove(i);
            } else {
                kit.removing.push(key);
            }
        }
        self.publish_kit();
    }

    pub(super) fn kit_discard(&self) {
        {
            let mut s = self.state.borrow_mut();
            s.org.kit.adding.clear();
            s.org.kit.removing.clear();
        }
        self.publish_kit();
    }

    /// A removal erases what the person noted of the piece on The Lens DB: asked first.
    pub(super) fn kit_validate(self: &Rc<Self>) {
        let removals = self.state.borrow().org.kit.removing.len();
        if removals == 0 {
            return self.kit_send();
        }
        let message = trn(
            "The piece leaves your Kit on The Lens DB, with the condition, price, serial number and notes kept there for it.",
            "{n} pieces leave your Kit on The Lens DB, with the condition, price, serial number and notes kept there for them.",
            removals as u64,
        );
        self.ask(tr("Remove From Your Kit?"), &message, tr("Remove"), true, Pending::Run(Box::new(|c: &Rc<Ctl>| c.kit_send())));
    }

    pub(super) fn kit_send(&self) {
        let Some(client) = self.client() else { return };
        let (add, remove) = {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            if kit.sending || (kit.adding.is_empty() && kit.removing.is_empty()) {
                return;
            }
            kit.sending = true;
            (kit.adding.iter().map(Piece::key).collect::<Vec<_>>(), kit.removing.clone())
        };
        self.publish_kit();
        std::thread::spawn(move || {
            let mut last = None;
            let entries: Vec<(Key, bool)> = add.into_iter().map(|k| (k, true)).chain(remove.into_iter().map(|k| (k, false))).collect();
            for part in entries.chunks(BATCH) {
                let (adds, removes): (Vec<_>, Vec<_>) = part.iter().partition(|(_, adding)| *adding);
                let keys = |v: Vec<&(Key, bool)>| v.into_iter().map(|(k, _)| *k).collect::<Vec<_>>();
                last = Some(client.change(&keys(adds), &keys(removes)));
                if matches!(last, Some(Err(_))) {
                    break;
                }
            }
            if let Some(answer) = last {
                with_shell(move |c| c.kit_sent(answer));
            }
        });
    }

    pub(super) fn kit_sent(&self, answer: Result<(oni_tldb::Kit, Option<String>), Failure>) {
        self.state.borrow_mut().org.kit.sending = false;
        match answer {
            Ok((kit, etag)) => {
                {
                    let mut s = self.state.borrow_mut();
                    s.org.kit.adding.clear();
                    s.org.kit.removing.clear();
                }
                self.kit_keep(kit, etag);
            }
            // The changes wait, so the same Validate sends them again: the service takes a repeat as one.
            Err(f) => self.kit_failed(f),
        }
        self.publish_kit();
    }
}
