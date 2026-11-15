//! The sign-in through the system browser: the port opened, the page shown, the token kept once it returns.

use super::*;

/// What the browser shows once it returns to ONI's port: the page stays, it never closes itself.
fn landing(signed_in: bool) -> String {
    let (title, words) = if signed_in {
        (tr("Connected to The Lens DB"), tr("You can go back to ONI: your Kit is there."))
    } else {
        (tr("ONI was not authorised"), tr("You can go back to ONI and connect again whenever you want."))
    };
    const STYLE: &str = "font:16px -apple-system,Segoe UI,sans-serif;max-width:32em;margin:15vh auto;padding:0 1em";
    format!("<!doctype html><meta charset=utf-8><title>ONI</title><body style=\"{STYLE}\"><h1 style=\"font-size:1.4em\">{title}</h1><p>{words}</p>")
}

impl Ctl {
    pub(super) fn kit_connect(self: &Rc<Self>, signup: bool) {
        if self.state.borrow().org.kit.signing.is_some() {
            return;
        }
        let service = service();
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
                let support = self.state.borrow().support.clone();
                if let Err(e) = vault::write(&support, faked(), &token) {
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
