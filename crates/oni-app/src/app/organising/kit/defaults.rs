//! The body and the lens the Kit names by default: picked on their card, laid under every export's line where
//! the roll and the frame name none, and kept by a roll whose frames are written with its body.

use super::*;
use oni_library::collection::Gear;
use oni_settings::Film;

fn gear(kind: Kind) -> Gear {
    if kind == Kind::Camera { Gear::Body } else { Gear::Lens }
}

impl Ctl {
    /// A piece of the Kit made the default of its kind, or none when it was; a piece not saved yet is none.
    pub(super) fn kit_prefer(&self, kind: GearKind, at: usize) {
        let kind = if kind == GearKind::Body { Kind::Camera } else { Kind::Lens };
        {
            let mut s = self.state.borrow_mut();
            let Some(name) = s.org.kit.listed(kind).get(at).filter(|p| s.org.kit.mark(p) == KitMark::Kept).map(|p| p.name.clone()) else { return };
            s.org.collection.toggle_default(gear(kind), &name);
        }
        self.keep_gear();
        self.publish_kit();
    }

    /// The default named by its card while the Kit holds it, so none fills a line unseen.
    pub(super) fn kit_default(&self, kind: Kind) -> Option<String> {
        let s = self.state.borrow();
        let name = s.org.collection.default_of(gear(kind))?;
        s.org.kit.kept.named(kind, name).map(|p| p.name.clone())
    }

    pub(in crate::app) fn kit_defaults(&self) -> crate::export::Defaults {
        crate::export::Defaults { body: self.kit_default(Kind::Camera), lens: self.kit_default(Kind::Lens) }
    }

    /// Each roll whose lines show the default body, its own body empty, takes it; the rolls that did, by
    /// their film. While a disk gesture runs none does, and the frames keep the body themselves.
    pub(in crate::app) fn roll_default_bodies(self: &Rc<Self>, defaulted: Vec<(String, Film, String)>) -> Vec<oni_settings::Id> {
        let mut taken = Vec::new();
        if self.state.borrow().org.disk_busy.is_some() {
            return taken;
        }
        let mut done = Vec::new();
        for (path, film, body) in defaulted {
            let roll = self.state.borrow().roll_at(&path).filter(|r| r.film.id == film.id && r.film.camera_body.as_deref().is_none_or(str::is_empty));
            let Some(roll) = roll.filter(|r| !done.contains(&r.key())) else { continue };
            done.push(roll.key());
            let marker = roll.marker.clone();
            self.update_roll_of(roll, tr("Set Camera Body"), |f| f.camera_body = Some(body.clone()));
            if self.state.borrow().roll_at(&path).is_some_and(|r| r.film.camera_body.as_deref() == Some(body.as_str()) && r.marker == marker) {
                taken.push(film.id);
            }
        }
        taken
    }
}
