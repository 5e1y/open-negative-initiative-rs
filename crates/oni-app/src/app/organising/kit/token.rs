//! The token read from the vault at the first use that needs it, never at launch and never on the interface
//! thread, where a keychain's prompt would hold the window; until then the copy's mark says who is signed in.

use super::*;

/// The token as this run knows it.
#[derive(Default)]
pub(super) enum Token {
    #[default]
    Unread,
    /// Being read on the vault's thread; what waits on it runs once it lands.
    Reading(Vec<Waiting>),
    Known(Option<String>),
}

type Waiting = Box<dyn FnOnce(&Rc<Ctl>, Option<Client>)>;

impl KitState {
    /// Signed in as the interface shows it: the token once read, the copy's mark until then.
    pub(super) fn linked(&self) -> bool {
        match &self.token {
            Token::Known(token) => token.is_some(),
            Token::Unread | Token::Reading(_) => self.vault.is_some() && self.kept.signed_in(),
        }
    }
}

impl Ctl {
    /// The service's client, none until the token is read or when signed out.
    pub(super) fn client(&self) -> Option<Client> {
        let s = self.state.borrow();
        let kit = &s.org.kit;
        let Token::Known(Some(token)) = &kit.token else { return None };
        Some(Client { service: kit.service()?, token: token.clone() })
    }

    /// `then` handed the client once the token is known, none when signed out: the vault is read once, at
    /// the first call, and the Kit read again once it lands.
    pub(super) fn kit_with_client(self: &Rc<Self>, then: impl FnOnce(&Rc<Ctl>, Option<Client>) + 'static) {
        {
            let mut s = self.state.borrow_mut();
            let kit = &mut s.org.kit;
            if matches!(kit.token, Token::Unread) {
                match kit.vault.as_ref().filter(|_| kit.linked()) {
                    Some(worker) => {
                        crate::timing::line("tldb: the token is read from the vault at its first use");
                        worker.run(|vault| {
                            let token = vault.read();
                            with_shell(move |c| c.kit_unlocked(token));
                        });
                        kit.token = Token::Reading(Vec::new());
                    }
                    None => kit.token = Token::Known(None),
                }
            }
            if let Token::Reading(waiting) = &mut kit.token {
                return waiting.push(Box::new(then));
            }
        }
        let client = self.client();
        then(self, client);
    }

    /// The vault's answer; a sign-in or a sign-out since stands over it. With no token, nothing that waited
    /// is sent and the copy's mark says signed out.
    fn kit_unlocked(self: &Rc<Self>, token: Option<String>) {
        let waiting = {
            let mut s = self.state.borrow_mut();
            let support = s.support.clone();
            let kit = &mut s.org.kit;
            let waiting = match std::mem::take(&mut kit.token) {
                Token::Reading(waiting) => waiting,
                other => {
                    kit.token = other;
                    return;
                }
            };
            if token.is_none() {
                crate::timing::line("tldb: no token in the vault, so the Kit is signed out");
                (kit.searching, kit.sending) = (false, false);
                kit.kept.signed = Some(false);
                if let Err(e) = kit.kept.save(&support) {
                    crate::timing::line(&format!("the Kit's copy is not kept: {:?}", e.kind()));
                }
            }
            kit.token = Token::Known(token);
            waiting
        };
        self.publish_kit();
        for then in waiting {
            then(self, self.client());
        }
        self.kit_refresh();
    }

    /// A sign-in's token, kept in the vault on its thread and marked on the copy.
    pub(super) fn kit_keep_token(&self, token: String) {
        let mut s = self.state.borrow_mut();
        let support = s.support.clone();
        let kit = &mut s.org.kit;
        kit.token = Token::Known(Some(token.clone()));
        kit.kept.signed = Some(true);
        if let Err(e) = kit.kept.save(&support) {
            crate::timing::line(&format!("the Kit's copy is not kept: {:?}", e.kind()));
        }
        if let Some(worker) = &kit.vault {
            worker.run(move |vault| {
                if let Err(e) = vault.write(&token) {
                    crate::timing::line(&format!("the Kit's token is not kept: {e}"));
                }
            });
        }
    }

    /// The token forgotten and erased from the vault on its thread; `revoke`, the person's sign-out, also
    /// revokes it at the service, read from the vault first when this run never read it.
    pub(super) fn kit_forget(&self, revoke: bool) {
        let mut s = self.state.borrow_mut();
        let kit = &mut s.org.kit;
        let known = match std::mem::replace(&mut kit.token, Token::Known(None)) {
            Token::Known(token) => Some(token),
            Token::Unread | Token::Reading(_) => None,
        };
        let (Some(worker), service) = (&kit.vault, kit.service()) else { return };
        worker.run(move |vault| {
            let token = match known {
                Some(token) => token,
                None if revoke => vault.read(),
                None => None,
            };
            vault.erase();
            if let (true, Some(token), Some(service)) = (revoke, token, service) {
                std::thread::spawn(move || {
                    if let Err(f) = service.revoke(&token) {
                        crate::timing::line(&format!("the Kit's token is not revoked: {f:?}"));
                    }
                });
            }
        });
    }
}
