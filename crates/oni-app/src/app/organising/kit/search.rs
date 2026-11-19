//! The catalogue searched as the hand rests or the kind changes, the latest answer alone kept; the mounts
//! the answers and the Kit hold are the mount filter's choices, which ONI applies as the search takes none.

use super::*;

impl KitState {
    /// The mount filter's choices: the answers' mounts, then the Kit's, so one is offered before any search;
    /// a mount chosen stays offered when neither holds it, so it can be let go.
    pub(super) fn offer_mounts(&mut self) {
        let mut mounts = words::mounts(&self.answers);
        for mount in words::mounts(&self.kept.items) {
            if !mounts.contains(&mount) {
                mounts.push(mount);
            }
        }
        if !self.mount_filter.is_empty() && !mounts.contains(&self.mount_filter) {
            mounts.push(self.mount_filter.clone());
        }
        self.mounts = mounts;
    }

    /// The answers the mount filter keeps.
    fn sift(&mut self) {
        let mount = self.mount_filter.as_str();
        self.found = self.answers.iter().filter(|p| mount.is_empty() || words::fits(p, mount)).cloned().collect();
    }
}

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

    /// The kind changed: the search asked again at once with it.
    pub(super) fn kit_filter(self: &Rc<Self>, change: impl FnOnce(&mut KitState)) {
        change(&mut self.state.borrow_mut().org.kit);
        self.kit_ask();
    }

    /// The mount at its place among the choices, 0 any: the answers held are sifted again, nothing asked.
    pub(super) fn kit_mount(self: &Rc<Self>, at: i32) {
        {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            kit.mount_filter = usize::try_from(at - 1).ok().and_then(|i| kit.mounts.get(i)).cloned().unwrap_or_default();
            kit.sift();
        }
        self.publish_kit();
    }

    pub(super) fn kit_ask(self: &Rc<Self>) {
        let Some(client) = self.client() else { return };
        if self.state.borrow().org.kit.held().is_some() {
            self.kit_ask_after_wait();
            return self.publish_kit();
        }
        let (query, kind, asked) = {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            kit.asked += 1;
            let query = words::query(&kit.typed);
            kit.searching = query.chars().count() >= 2;
            if !kit.searching {
                (kit.answers, kit.found) = (Vec::new(), Vec::new());
                kit.searched.clear();
                kit.offer_mounts();
            }
            (query, kit.kind_filter, kit.asked)
        };
        self.publish_kit();
        if query.chars().count() < 2 {
            return;
        }
        std::thread::spawn(move || {
            let answer = client.search(&query, kind, None, FOUND);
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
                (kit.answers, kit.searched) = (found, query);
                kit.offer_mounts();
                kit.sift();
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
