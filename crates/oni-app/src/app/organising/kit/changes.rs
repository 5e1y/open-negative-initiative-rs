//! The changes held for Save: pieces found added, pieces of the Kit marked to leave, all sent in one call.

use super::*;

impl Ctl {
    /// The search's sheet opened or closed; a closed one forgets its words.
    pub(super) fn kit_pick(&self, open: bool) {
        {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            kit.picking = open;
            if !open {
                kit.asked += 1;
                (kit.found, kit.searching) = (Vec::new(), false);
                kit.searched.clear();
            }
        }
        self.publish_kit();
    }

    /// A search's answer taken into the Kit until Save, or let go when taken again.
    pub(super) fn kit_take(&self, at: usize) {
        {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            let Some(piece) = kit.found.get(at).cloned() else { return };
            match kit.mark(&piece) {
                KitMark::Free => kit.adding.push(piece),
                KitMark::Added => kit.adding.retain(|p| p.key() != piece.key()),
                KitMark::Kept | KitMark::Leaving => return,
            }
        }
        self.publish_kit();
    }

    /// A piece of the Kit marked to leave at Save, kept again when asked again; an added one let go.
    pub(super) fn kit_remove(&self, kind: GearKind, at: usize) {
        {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            let kind = if kind == GearKind::Body { Kind::Camera } else { Kind::Lens };
            let Some(key) = kit.listed(kind).get(at).map(|p| p.key()) else { return };
            if kit.adding.iter().any(|p| p.key() == key) {
                kit.adding.retain(|p| p.key() != key);
            } else if let Some(i) = kit.removing.iter().position(|k| *k == key) {
                kit.removing.remove(i);
            } else {
                kit.removing.push(key);
            }
        }
        self.publish_kit();
    }

    /// The Kit's column left with changes not saved: asked once, so none is forgotten.
    pub(super) fn kit_left(&self) {
        {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            if kit.adding.is_empty() && kit.removing.is_empty() || kit.sending {
                return;
            }
            kit.asking = true;
        }
        self.publish_kit();
    }

    /// 0 discards, 1 saves, any other keeps the changes for when the Kit is shown again.
    pub(super) fn kit_answered(self: &Rc<Self>, answer: i32) {
        self.state.borrow_mut().org.kit.asking = false;
        match answer {
            0 => self.kit_discard(),
            1 => self.kit_save(),
            _ => self.publish_kit(),
        }
    }

    /// Changes not saved, which quitting would lose.
    pub(in crate::app) fn kit_unsaved(&self) -> usize {
        let s = self.state.borrow();
        if s.org.kit.token.is_none() { 0 } else { s.org.kit.adding.len() + s.org.kit.removing.len() }
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
    pub(super) fn kit_save(self: &Rc<Self>) {
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
            // The changes wait, so the same Save sends them again: the service takes a repeat as one.
            Err(f) => self.kit_failed(f),
        }
        self.publish_kit();
    }
}
