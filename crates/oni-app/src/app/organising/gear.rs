//! The bodies and lenses typed in ONI: read from the support folder once the window is wired, learned from the
//! names a hand gives a roll's body or an export's camera and lens, and offered in the gear fields after the Kit's cards.

use super::*;
use crate::Gear as GearGlobal;
use oni_library::collection::{Collection, Gear};
use oni_tldb::Kind;

impl Ctl {
    pub(super) fn wire_gear(me: &Rc<Ctl>) {
        {
            let mut s = me.state.borrow_mut();
            s.org.collection = Collection::load(&s.support);
        }
        me.seed_gear();
        if let Some(w) = me.window() {
            let p = me.clone();
            w.global::<GearGlobal>().on_served(move |kind, name, _| {
                let kind = if kind == 1 { Kind::Camera } else { Kind::Lens };
                p.state.borrow().org.kit.kept.named(kind, &name).is_some()
            });
        }
        me.publish_gear();
    }

    /// The bodies a hand gave the rolls the index holds, taken in once; an index with no photo yet waits for
    /// the next launch, so a first one before any folder is read takes nothing for good.
    fn seed_gear(&self) {
        let bodies = {
            let s = self.state.borrow();
            if s.org.collection.seeded {
                return;
            }
            s.index.as_ref().map(|index| index.camera_bodies())
        };
        let bodies = match bodies {
            Some(Ok(Some(bodies))) => bodies,
            Some(Err(e)) => return crate::timing::line(&format!("the rolls' bodies are not read: {e}")),
            _ => return,
        };
        {
            let mut s = self.state.borrow_mut();
            s.org.collection.take_in(Gear::Body, &bodies);
            s.org.collection.seeded = true;
        }
        self.keep_gear();
    }

    /// A name a hand gave a body or a lens, first in its list; kept at once, as another window may read it.
    pub(in crate::app) fn note_gear(&self, gear: Gear, name: &str) {
        let changed = self.state.borrow_mut().org.collection.note(gear, name);
        if changed {
            self.keep_gear();
        }
    }

    pub(super) fn keep_gear(&self) {
        let written = {
            let s = self.state.borrow();
            s.org.collection.save(&s.support)
        };
        if let Err(e) = written {
            crate::timing::line(&format!("the collection is not kept: {:?}", e.kind()));
        }
        self.publish_gear();
    }

    pub(in crate::app) fn publish_gear(&self) {
        let Some(w) = self.window() else { return };
        let g = w.global::<GearGlobal>();
        let (bodies, lenses) = {
            let s = self.state.borrow();
            (s.org.collection.list(Gear::Body).to_vec(), s.org.collection.list(Gear::Lens).to_vec())
        };
        let model = |names: Vec<String>| ModelRc::new(VecModel::from(names.into_iter().map(SharedString::from).collect::<Vec<_>>()));
        g.set_body_cards(ModelRc::new(VecModel::from(self.kit_cards(Kind::Camera))));
        g.set_lens_cards(ModelRc::new(VecModel::from(self.kit_cards(Kind::Lens))));
        g.set_bodies(model(self.kit_names(Kind::Camera, &bodies)));
        g.set_lenses(model(self.kit_names(Kind::Lens, &lenses)));
        g.set_revision(g.get_revision().wrapping_add(1));
    }
}
