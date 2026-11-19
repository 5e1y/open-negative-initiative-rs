//! The sign-in through the system browser: the port opened, the page shown, the token kept once it returns.

mod landing;

use super::*;
use landing::landing;

impl Ctl {
    pub(super) fn kit_connect(self: &Rc<Self>, signup: bool) {
        if self.state.borrow().org.kit.signing.is_some() {
            return;
        }
        let Some(service) = self.state.borrow().org.kit.service() else {
            crate::timing::line("tldb: this run asks no The Lens DB, so Connect opens nothing");
            return;
        };
        let pending = match oauth::begin(&service, signup) {
            Ok(p) => p,
            Err(e) => {
                crate::timing::line(&format!("the Kit's sign-in port is not opened: {:?}", e.kind()));
                self.state.borrow_mut().org.kit.note = tr("ONI cannot listen for The Lens DB's answer on this computer.").into();
                return self.publish_kit();
            }
        };
        self.kit_browse(&pending.url);
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut s = self.state.borrow_mut();
            s.org.kit.signing = Some(cancel.clone());
            s.org.kit.note.clear();
        }
        self.publish_kit();
        let pages = (landing(true), landing(false));
        std::thread::spawn(move || {
            let outcome = pending.finish(&service, oauth::WAIT, &cancel, &|signed| if signed { pages.0.clone() } else { pages.1.clone() });
            with_shell(move |c| c.kit_signed(outcome));
        });
    }

    pub(super) fn kit_cancel(&self) {
        if let Some(flag) = self.state.borrow_mut().org.kit.signing.take() {
            flag.store(true, Ordering::Release);
        }
        self.publish_kit();
    }

    pub(super) fn kit_signed(self: &Rc<Self>, outcome: oauth::Outcome) {
        let note = match outcome {
            oauth::Outcome::Token(token) => {
                let (support, simulated) = (self.state.borrow().support.clone(), self.state.borrow().org.kit.simulated());
                if let Err(e) = vault::write(&support, simulated, &token) {
                    crate::timing::line(&format!("the Kit's token is not kept: {e}"));
                }
                self.state.borrow_mut().org.kit.token = Some(token);
                String::new()
            }
            oauth::Outcome::Refused => tr("ONI was not authorised on The Lens DB.").into(),
            oauth::Outcome::TimedOut => tr("The sign-in waited 15 minutes and stopped: connect again when you are ready.").into(),
            oauth::Outcome::Cancelled => String::new(),
            oauth::Outcome::Failed(f) => self.failure_note(&f),
        };
        {
            let mut s = self.state.borrow_mut();
            s.org.kit.signing = None;
            s.org.kit.note = note;
        }
        self.publish_kit();
        self.kit_refresh();
    }
}
