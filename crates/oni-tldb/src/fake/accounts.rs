//! The simulated accounts: an e-mail, a password, a name and a Kit each, the first one made at start. Kept
//! in a file when the setup names one, so a sign-in outlives the run.

use super::{Live, State, starting_kit};
use crate::piece::{Key, Piece};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// The account every start has, its password shown on the sign-in page.
pub const FIRST_EMAIL: &str = "camille@example.com";
pub const FIRST_PASSWORD: &str = "film";
pub const FIRST_NAME: &str = "Camille";

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub email: String,
    /// In plain text: the pages ask for an invented one.
    pub password: String,
    pub name: String,
    /// The link of the simulated confirmation e-mail, until it is followed.
    pub unconfirmed: Option<String>,
    pub kit: Vec<Key>,
}

pub fn first(catalogue: &[Piece]) -> Account {
    Account { email: FIRST_EMAIL.into(), password: FIRST_PASSWORD.into(), name: FIRST_NAME.into(), unconfirmed: None, kit: starting_kit(catalogue) }
}

/// What the kept file holds.
#[derive(Serialize, Deserialize)]
struct Kept {
    accounts: Vec<Account>,
    /// Each live token and its account's e-mail.
    tokens: Vec<(String, String)>,
    issued: u64,
}

/// Takes the kept file's accounts and tokens; a missing or unreadable file leaves the first account alone.
pub fn load(state: &mut State) {
    let Some(path) = &state.kept else { return };
    let Some(kept) = std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice::<Kept>(&bytes).ok()) else { return };
    if kept.accounts.is_empty() {
        return;
    }
    state.accounts = kept.accounts;
    state.issued = kept.issued;
    for (token, email) in kept.tokens {
        if let Some(account) = find(state, &email) {
            state.tokens.insert(token, Live { account, recent: VecDeque::new() });
        }
    }
}

pub fn save(state: &State) {
    let Some(path) = &state.kept else { return };
    let tokens = state.tokens.iter().map(|(token, live)| (token.clone(), state.accounts[live.account].email.clone())).collect();
    let kept = Kept { accounts: state.accounts.clone(), tokens, issued: state.issued };
    if let Ok(bytes) = serde_json::to_vec_pretty(&kept) {
        let _ = std::fs::write(path, bytes);
    }
}


/// An e-mail matches whatever its case and surrounding spaces.
pub fn find(state: &State, email: &str) -> Option<usize> {
    let email = email.trim().to_lowercase();
    state.accounts.iter().position(|a| a.email == email)
}
