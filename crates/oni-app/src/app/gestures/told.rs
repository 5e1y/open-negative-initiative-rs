//! What a gesture could not do or did only in part, said one way wherever the hand is: a transient
//! notice over the gallery, the organiser and the editor alike. A success says nothing.

use super::*;
use crate::Told;
use oni_library::absence::{self, Absence};
use oni_library::{paths, refusal};

impl Ctl {
    pub(in crate::app) fn wire_told(me: &Rc<Self>, window: &crate::MainWindow) {
        let p = me.clone();
        window.global::<Told>().on_acted(move || p.told_acted());
        let p = me.clone();
        window.global::<Told>().on_closed(move || p.told_closed());
    }

    /// The notice titled with the gesture, as Swift's `reportFailure(_:as:)`; one shown gives way to it.
    pub(in crate::app) fn told(&self, title: &str, message: &str) {
        self.told_with(title, message, None);
    }

    /// The notice, with a Show in Finder key on `folder` when what could not move was left there.
    pub(in crate::app) fn told_with(&self, title: &str, message: &str, folder: Option<String>) {
        let Some(w) = self.window() else { return };
        let key = if folder.is_some() { crate::app::navigator::reveal_word() } else { "" };
        {
            let mut s = self.state.borrow_mut();
            s.org.told_folder = folder;
            s.nav.unstarred = None;
        }
        let t = w.global::<Told>();
        t.set_title(title.into());
        t.set_message(message.into());
        t.set_action(key.into());
        t.set_serial(t.get_serial().wrapping_add(1));
        t.set_shown(true);
    }

    /// The notice saying a favourite went, its key starring it again.
    pub(in crate::app) fn told_undo(&self, title: &str, message: &str, favourite: &str) {
        self.told(title, message);
        self.state.borrow_mut().nav.unstarred = Some(favourite.to_owned());
        if let Some(w) = self.window() {
            w.global::<Told>().set_action(tr("UNDO").into());
        }
    }

    fn told_acted(self: &Rc<Self>) {
        let (folder, unstarred) = {
            let mut s = self.state.borrow_mut();
            (s.org.told_folder.take(), s.nav.unstarred.take())
        };
        self.told_closed();
        if let Some(folder) = folder {
            self.reveal_folder(&folder);
        }
        if let Some(favourite) = unstarred {
            self.star(&favourite);
        }
    }

    pub(in crate::app) fn told_closed(&self) {
        {
            let mut s = self.state.borrow_mut();
            s.org.told_folder = None;
            s.nav.unstarred = None;
        }
        if let Some(w) = self.window() {
            w.global::<Told>().set_shown(false);
        }
    }

    /// Show in Finder on a folder, looked at off this thread: one gone shows the nearest folder still
    /// there, one on a disk away or refused says why.
    pub(in crate::app) fn reveal_folder(&self, folder: &str) {
        let (folder, computer) = (folder.to_owned(), self.state.borrow().nav.computer.clone());
        let _ = std::thread::Builder::new().name("revealing".into()).spawn(move || {
            let refused = match std::fs::metadata(&folder) {
                Ok(_) => return system::reveal(Path::new(&folder)),
                Err(e) => e,
            };
            let name = paths::name(&folder);
            let said = match absence::refusal(&computer, &folder, &refused) {
                Absence::Gone => {
                    system::reveal(Path::new(&absence::nearest_folder(&folder)));
                    tr!("“{name}” is no longer here: the nearest folder still there is shown.", name = name)
                }
                Absence::Unplugged { volume } => tr!("“{name}” cannot be shown: its disk “{volume}” is not connected.", name = name, volume = volume),
                Absence::Denied => tr!("“{name}” cannot be shown: ONI has no access to it.", name = name),
                Absence::Empty | Absence::Unreadable(_) => tr!("“{name}” cannot be shown: {cause}.", name = name, cause = refusal::cause(&refused)),
            };
            with_shell(move |c| c.told(crate::app::navigator::reveal_word(), &said));
        });
    }
}
