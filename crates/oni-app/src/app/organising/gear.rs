//! The person's collection of bodies and lenses: read from the support folder once the window is wired,
//! offered in an export's EXIF and a roll's body, shown where nothing is picked in the archive, and learned
//! from the names a hand gives a roll's body or an export's camera and lens.

use super::*;
use crate::{Gear as GearGlobal, GearKind};
use oni_library::collection::{Collection, Gear};

fn gear(kind: GearKind) -> Gear {
    match kind {
        GearKind::Body => Gear::Body,
        GearKind::Lens => Gear::Lens,
    }
}

impl Ctl {
    pub(super) fn wire_gear(me: &Rc<Ctl>, window: &crate::MainWindow) {
        {
            let mut s = me.state.borrow_mut();
            s.org.collection = Collection::load(&s.support);
        }
        let g = window.global::<GearGlobal>();
        let p = me.clone();
        g.on_add(move |kind, name| p.note_gear(gear(kind), &name));
        let p = me.clone();
        g.on_forget(move |kind, name| p.forget_gear(gear(kind), &name));
        me.publish_gear();
    }

    /// A name a hand gave a body or a lens, first in its list; kept at once, as another window may read it.
    pub(in crate::app) fn note_gear(&self, gear: Gear, name: &str) {
        let changed = self.state.borrow_mut().org.collection.note(gear, name);
        if changed {
            self.keep_gear();
        }
    }

    pub(in crate::app) fn forget_gear(&self, gear: Gear, name: &str) {
        let changed = self.state.borrow_mut().org.collection.forget(gear, name);
        if changed {
            self.keep_gear();
        }
    }

    fn keep_gear(&self) {
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
        let s = self.state.borrow();
        let model = |names: &[String]| ModelRc::new(VecModel::from(names.iter().map(|n| SharedString::from(n.as_str())).collect::<Vec<_>>()));
        g.set_bodies(model(s.org.collection.list(Gear::Body)));
        g.set_lenses(model(s.org.collection.list(Gear::Lens)));
    }
}
