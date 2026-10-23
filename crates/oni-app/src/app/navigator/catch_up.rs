//! RootWelcome and RootsCatchUp: a former library's folders offered once.

use super::*;

impl Ctl {
    /// `offerNeutralDecodeNotice`'s guard: no notice under the catch-up sheet.
    pub(in crate::app) fn catching_up(&self) -> bool {
        self.state.borrow().nav.catching_up
    }

    /// `RootsCatchUp.outside`: the former photos no favourite holds, by folder, most first.
    pub(super) fn publish_catch_up(&self) {
        let Some(w) = self.window() else { return };
        let (choose, left, lines, more, all_in) = {
            let mut s = self.state.borrow_mut();
            if !s.nav.catching_up {
                return;
            }
            let roots = s.nav.favourites.paths.clone();
            let mut counts: HashMap<String, usize> = HashMap::new();
            for path in s.nav.former.iter().filter(|p| !roots.iter().any(|r| holds(r, p))) {
                *counts.entry(paths::parent(path)).or_default() += 1;
            }
            let mut outside: Vec<(String, usize)> = counts.into_iter().collect();
            outside.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let total: usize = outside.iter().map(|(_, n)| n).sum();
            let choose = if roots.is_empty() { tr("Add Folder…") } else { tr("Add Another Folder…") }.to_uppercase();
            let left = if outside.is_empty() { String::new() } else { tr!("{total} photos of your library are not in it yet:", total = total) };
            let more = if outside.len() > CATCH_UP_SHOWN { tr!("and {count} more folders", count = outside.len() - CATCH_UP_SHOWN) } else { String::new() };
            let all_in = if outside.is_empty() && !roots.is_empty() { tr("Every photo of your library is in.") } else { "" };
            outside.truncate(CATCH_UP_SHOWN);
            s.nav.catch_up_folders = outside.iter().map(|(f, _)| f.clone()).collect();
            let lines: Vec<CatchUpLine> = outside.into_iter().map(|(f, n)| CatchUpLine { folder: f.into(), count: n.to_string().into() }).collect();
            (choose, left, lines, more, all_in.to_string())
        };
        let n = w.global::<crate::Navigator>();
        n.set_catch_up_choose(choose.into());
        n.set_catch_up_left(left.into());
        n.set_catch_up_lines(ModelRc::new(VecModel::from(lines)));
        n.set_catch_up_more(more.into());
        n.set_catch_up_all_in(all_in.into());
    }

    /// `rootsCatchUpDismissed`: never shown again, and the recalibration notice may follow.
    pub(super) fn catch_up_done(&self) {
        {
            let mut s = self.state.borrow_mut();
            s.nav.catching_up = false;
            let _ = s.preferences.set(ROOTS_CATCH_UP_SHOWN, Value::Bool(true));
        }
        if let Some(w) = self.window() {
            w.global::<crate::Navigator>().set_catch_up_shown(false);
        }
        let order = self.state.borrow().order.clone();
        self.offer_neutral_decode_notice(&order);
    }
}
