//! The catalogue searched as the hand rests or a quick filter changes, the latest answer alone kept.

use super::*;

impl Ctl {
    pub(super) fn kit_search(self: &Rc<Self>, typed: &str) {
        let me = Rc::downgrade(self);
        let mut s = self.state.borrow_mut();
        s.org.kit.typed = typed.to_string();
        s.org.kit.settle.start(slint::TimerMode::SingleShot, SETTLE, move || {
            if let Some(me) = me.upgrade() {
                me.kit_ask();
            }
        });
    }

    /// A quick filter changed: the search asked again at once with it.
    pub(super) fn kit_filter(&self, change: impl FnOnce(&mut KitState)) {
        change(&mut self.state.borrow_mut().org.kit);
        self.kit_ask();
    }

    fn kit_ask(&self) {
        let (query, kind, brand) = {
            let s = self.state.borrow();
            let kit = &s.org.kit;
            (words::filtered(&kit.typed, &kit.brand_filter, kit.focal_filter), kit.kind_filter, kit.brand_filter.clone())
        };
        let Some(client) = self.client() else { return };
        let asked = {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            kit.asked += 1;
            if query.chars().count() < 2 {
                kit.found.clear();
                kit.searched.clear();
                kit.searching = false;
                None
            } else {
                kit.searching = true;
                Some(kit.asked)
            }
        };
        self.publish_kit();
        let Some(asked) = asked else { return };
        std::thread::spawn(move || {
            let answer = client.search(&query, kind, Some(brand.as_str()).filter(|b| !b.is_empty()), FOUND);
            with_shell(move |c| c.kit_found(asked, query, answer));
        });
    }

    fn kit_found(&self, asked: u64, query: String, answer: Result<Vec<Piece>, Failure>) {
        if self.state.borrow().org.kit.asked != asked {
            return;
        }
        self.state.borrow_mut().org.kit.searching = false;
        match answer {
            Ok(found) => {
                let mut s = self.state.borrow_mut();
                s.org.kit.found = found;
                s.org.kit.searched = query;
            }
            Err(f) => self.kit_failed(f),
        }
        self.publish_kit();
    }
}
