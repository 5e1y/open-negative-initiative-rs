//! The catalogue searched as the hand rests or a filter changes, the latest answer alone kept; the brands
//! the answers hold become the brand filter's choices, as the service lists none.

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

    /// A filter changed: the search asked again at once with it.
    pub(super) fn kit_filter(self: &Rc<Self>, change: impl FnOnce(&mut KitState)) {
        change(&mut self.state.borrow_mut().org.kit);
        self.kit_ask();
    }

    pub(super) fn kit_ask(self: &Rc<Self>) {
        let Some(client) = self.client() else { return };
        if self.state.borrow().org.kit.held().is_some() {
            self.kit_ask_after_wait();
            return self.publish_kit();
        }
        let (query, kind, brand, asked) = {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            kit.asked += 1;
            let query = words::query(&kit.typed);
            kit.searching = query.chars().count() >= 2;
            if !kit.searching {
                kit.found.clear();
                kit.searched.clear();
            }
            (query, kit.kind_filter, kit.brand_filter.clone(), kit.asked)
        };
        self.publish_kit();
        if query.chars().count() < 2 {
            return;
        }
        std::thread::spawn(move || {
            let answer = client.search(&query, kind, Some(brand.as_str()).filter(|b| !b.is_empty()), FOUND);
            with_shell(move |c| c.kit_found(asked, query, answer));
        });
    }

    fn kit_found(self: &Rc<Self>, asked: u64, query: String, answer: Result<Vec<Piece>, Failure>) {
        if self.state.borrow().org.kit.asked != asked {
            return;
        }
        self.state.borrow_mut().org.kit.searching = false;
        match answer {
            Ok(found) => {
                let mut s = self.state.borrow_mut();
                let kit = &mut s.org.kit;
                if kit.brand_filter.is_empty() {
                    kit.brands = words::brands(&found);
                }
                (kit.found, kit.searched) = (found, query);
                kit.note.clear();
            }
            Err(f) => {
                let wait = matches!(f, Failure::Wait(_));
                self.kit_failed(f);
                if wait {
                    self.kit_ask_after_wait();
                }
            }
        }
        self.publish_kit();
    }
}
