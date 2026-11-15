//! The person's Kit on The Lens DB, in the archive's column where nothing is picked: the sign-in through
//! the system browser, the catalogue searched, the changes sent in one call at Validate, and the copy kept
//! in the support folder, which ONI works from offline and which fills an export's EXIF.

mod changes;
mod facts;
mod sign_in;
mod vault;
mod words;

pub use facts::complete;

use super::*;
use crate::{GearKind, Kit as KitGlobal, KitFound, KitLink, KitPiece};
use oni_i18n::{tr, trn};
use oni_tldb::{Client, Failure, Key, Kept, Kind, Piece, Read, Service, oauth};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::app::{Pending, with_shell};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// The search waits for the hand to rest this long, so a word typed asks the service once.
const SETTLE: Duration = Duration::from_millis(400);
/// A search's answers shown; the service gives 50 at most.
const FOUND: u32 = 20;
/// The entries one change carries at most, by the service's contract.
const BATCH: usize = 200;

#[derive(Default)]
pub struct KitState {
    /// Opens a page in the person's browser; a check keeps the address instead.
    browse: Option<Box<dyn Fn(&str)>>,
    pub kept: Kept,
    token: Option<String>,
    /// While a sign-in waits for the browser's return: the flag that cancels it.
    signing: Option<Arc<AtomicBool>>,
    found: Vec<Piece>,
    searched: String,
    typed: String,
    /// The latest search's number, so a slower earlier answer never replaces it.
    asked: u64,
    searching: bool,
    adding: Vec<Piece>,
    removing: Vec<Key>,
    sending: bool,
    note: String,
    settle: slint::Timer,
}

impl KitState {
    pub fn new(browse: Box<dyn Fn(&str)>) -> KitState {
        KitState { browse: Some(browse), ..KitState::default() }
    }
}

/// `ONI_TLDB=fake` swaps the service for the simulated one, started once in this process; a check hands the
/// address of its own. Either keeps its worthless token in the support folder, never in a keychain.
fn faked() -> bool {
    std::env::var_os("ONI_TLDB").is_some()
}

fn service() -> Service {
    static FAKE: OnceLock<oni_tldb::fake::Fake> = OnceLock::new();
    let site = match std::env::var("ONI_TLDB") {
        Ok(v) if v == "fake" => FAKE.get_or_init(oni_tldb::fake::start).site.clone(),
        Ok(v) if v.starts_with("http://127.0.0.1:") => v,
        _ => oni_tldb::SITE.to_string(),
    };
    Service::new(&site, crate::updates::version())
}

fn shown(piece: &Piece, removing: bool) -> KitPiece {
    KitPiece { name: piece.name.as_str().into(), detail: words::detail(piece).into(), url: piece.url.as_str().into(), removing }
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

impl Ctl {
    pub(super) fn wire_kit(me: &Rc<Ctl>, window: &crate::MainWindow) {
        {
            let mut s = me.state.borrow_mut();
            let support = s.support.clone();
            s.org.kit.kept = Kept::load(&support);
            s.org.kit.token = vault::read(&support, faked());
        }
        let k = window.global::<KitGlobal>();
        let p = me.clone();
        k.on_connect(move |signup| p.kit_connect(signup));
        let p = me.clone();
        k.on_cancel(move || p.kit_cancel());
        let p = me.clone();
        k.on_disconnect(move || p.kit_disconnect());
        let p = me.clone();
        k.on_search(move |t| p.kit_search(&t));
        let p = me.clone();
        k.on_add(move |i| p.kit_add(i.max(0) as usize));
        let p = me.clone();
        k.on_unadd(move |i| p.kit_unadd(i.max(0) as usize));
        let p = me.clone();
        k.on_remove(move |kind, i| p.kit_remove(kind, i.max(0) as usize));
        let p = me.clone();
        k.on_validate(move || p.kit_validate());
        let p = me.clone();
        k.on_discard(move || p.kit_discard());
        let p = me.clone();
        k.on_open(move |url| p.kit_browse(&service().page(&url)));
        let p = me.clone();
        k.on_open_site(move || p.kit_browse(&service().page(&service().site)));
        let p = me.clone();
        k.on_suggest(move || p.kit_browse(&service().submit_page()));
        me.publish_gear();
        me.publish_kit();
        me.kit_refresh();
    }

    fn kit_browse(&self, url: &str) {
        if let Some(browse) = &self.state.borrow().org.kit.browse {
            browse(url);
        }
    }

    fn client(&self) -> Option<Client> {
        self.state.borrow().org.kit.token.clone().map(|token| Client { service: service(), token })
    }

    pub(in crate::app) fn publish_kit(&self) {
        let Some(w) = self.window() else { return };
        let k = w.global::<KitGlobal>();
        let s = self.state.borrow();
        let kit = &s.org.kit;
        k.set_link(if kit.signing.is_some() {
            KitLink::Waiting
        } else if kit.token.is_some() {
            KitLink::In
        } else {
            KitLink::Out
        });
        k.set_account(kit.kept.account.clone().unwrap_or_default().into());
        let of = |kind| model(kit.kept.of(kind).map(|p| shown(p, kit.removing.contains(&p.key()))).collect());
        k.set_bodies(of(Kind::Camera));
        k.set_lenses(of(Kind::Lens));
        let among = |list: &[Piece], p: &Piece| list.iter().any(|h| h.key() == p.key());
        k.set_found(model(
            kit.found.iter().map(|p| KitFound { name: p.name.as_str().into(), detail: words::found(p).into(), held: among(&kit.kept.items, p), waiting: among(&kit.adding, p) }).collect(),
        ));
        k.set_searched(kit.searched.as_str().into());
        k.set_searching(kit.searching);
        k.set_adding(model(kit.adding.iter().map(|p| KitPiece { detail: words::found(p).into(), ..shown(p, false) }).collect()));
        k.set_removals(kit.removing.len() as i32);
        k.set_sending(kit.sending);
        k.set_note(kit.note.as_str().into());
    }

    /// The Kit read again, at launch and once signed in; an unchanged one costs the service no body.
    fn kit_refresh(&self) {
        let Some(client) = self.client() else { return };
        let etag = self.state.borrow().org.kit.kept.etag.clone();
        std::thread::spawn(move || {
            let read = client.kit(etag.as_deref());
            with_shell(move |c| c.kit_read(read));
        });
    }

    fn kit_read(&self, read: Result<Read, Failure>) {
        match read {
            Ok(Read::Fresh { kit, etag }) => self.kit_keep(kit, etag),
            Ok(Read::Same) => self.state.borrow_mut().org.kit.note.clear(),
            Err(f) => self.kit_failed(f),
        }
        self.publish_kit();
    }

    fn kit_keep(&self, kit: oni_tldb::Kit, etag: Option<String>) {
        let written = {
            let mut s = self.state.borrow_mut();
            s.org.kit.kept = Kept { account: Some(kit.account.display_name), items: kit.items, etag };
            s.org.kit.note = if kit.not_found.is_empty() {
                String::new()
            } else {
                trn("A piece is no longer in the catalogue and was left out.", "{n} pieces are no longer in the catalogue and were left out.", kit.not_found.len() as u64)
            };
            s.org.kit.kept.save(&s.support)
        };
        if let Err(e) = written {
            crate::timing::line(&format!("the Kit's copy is not kept: {:?}", e.kind()));
        }
        self.publish_gear();
    }

    fn failure_note(&self, failure: &Failure) -> String {
        match failure {
            Failure::SignedOut => tr("The Lens DB signed ONI out: the Kit last read is still offered in the EXIF.").into(),
            Failure::Wait(s) => tr!("The Lens DB asks ONI to wait {s} s before asking again.", s = s),
            Failure::Down(e) => {
                crate::timing::line(&format!("The Lens DB is not reached: {e}"));
                tr("The Lens DB cannot be reached: the Kit shown is the one last read.").into()
            }
            Failure::Refused(e) => {
                crate::timing::line(&format!("The Lens DB refused a request: {e}"));
                tr("The Lens DB refused ONI's request.").into()
            }
        }
    }

    /// A token the service no longer knows signs ONI out; the copy stays, as the contract asks.
    fn kit_failed(&self, failure: Failure) {
        let note = self.failure_note(&failure);
        let mut s = self.state.borrow_mut();
        if failure == Failure::SignedOut {
            s.org.kit.token = None;
            vault::erase(&s.support, faked());
        }
        s.org.kit.note = note;
    }

    fn kit_search(self: &Rc<Self>, typed: &str) {
        let me = Rc::downgrade(self);
        let mut s = self.state.borrow_mut();
        s.org.kit.typed = typed.to_string();
        s.org.kit.settle.start(slint::TimerMode::SingleShot, SETTLE, move || {
            if let Some(me) = me.upgrade() {
                me.kit_ask();
            }
        });
    }

    fn kit_ask(&self) {
        let query = words::query(&self.state.borrow().org.kit.typed);
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
            let answer = client.search(&query, None, FOUND);
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

    /// Signed out at the person's word: the token revoked, the copy and the waiting changes dropped.
    fn kit_disconnect(&self) {
        let token = {
            let mut s = self.state.borrow_mut();
            let support = s.support.clone();
            vault::erase(&support, faked());
            let kit = &mut s.org.kit;
            let token = kit.token.take();
            kit.kept = Kept::default();
            (kit.found, kit.adding, kit.removing) = (Vec::new(), Vec::new(), Vec::new());
            kit.searched.clear();
            kit.note.clear();
            let _ = std::fs::remove_file(Kept::file_path(&support));
            token
        };
        if let Some(token) = token {
            let service = service();
            std::thread::spawn(move || {
                if let Err(f) = service.revoke(&token) {
                    crate::timing::line(&format!("the Kit's token is not revoked: {f:?}"));
                }
            });
        }
        self.publish_kit();
        self.publish_gear();
    }

    pub(in crate::app) fn kit_facts(&self, m: &mut oni_settings::ExifMetadata) {
        complete(&self.state.borrow().org.kit.kept, m);
    }

    /// The names an export's EXIF offers: the Kit's pieces first, then the names typed in ONI.
    pub(in crate::app) fn kit_names(&self, kind: Kind, typed: &[String]) -> Vec<String> {
        let s = self.state.borrow();
        let mut names: Vec<String> = s.org.kit.kept.of(kind).map(|p| p.name.clone()).collect();
        for name in typed {
            if !names.iter().any(|n| n.to_lowercase() == name.to_lowercase()) {
                names.push(name.clone());
            }
        }
        names
    }
}
