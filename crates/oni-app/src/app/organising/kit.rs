//! The person's Kit on The Lens DB, in the archive's column where nothing is picked: the sign-in through
//! the system browser, the catalogue searched in a sheet whose Done sends what was added in one call, a
//! removal sent once confirmed, and the copy kept in the support folder, which ONI works from offline and
//! which fills an export's EXIF.

mod cards;
mod changes;
mod defaults;
mod facts;
mod publish;
mod search;
mod sign_in;
mod vault;
mod wait;
mod words;

pub use facts::complete;

use super::*;
use crate::app::{Pending, TldbSite, with_shell};
use crate::{GearKind, Kit as KitGlobal, KitLink, KitMark, KitPiece, KitSearch};
use oni_i18n::{tr, trn};
use oni_tldb::{Client, Failure, Key, Kept, Kind, Piece, Read, Service, oauth};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The search waits for the hand to rest this long, so a word typed asks the service once.
const SETTLE: Duration = Duration::from_millis(400);
/// A search's answers asked: the service's most.
const FOUND: u32 = 50;

#[derive(Default)]
pub struct KitState {
    /// Opens a page in the person's browser; a check keeps the address instead.
    browse: Option<Box<dyn Fn(&str)>>,
    site: TldbSite,
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
    /// What the search's sheet added, sent to the Kit at its Done.
    picks: Vec<Piece>,
    sending: bool,
    /// The search's sheet over the window.
    picking: bool,
    /// The search's filter: a kind, or none for both.
    kind_filter: Option<Kind>,
    /// How many photos name each piece, counted when they may have changed, never on each redraw.
    counts: Option<oni_library::index::GearCounts>,
    note: String,
    settle: slint::Timer,
    /// A 429's Retry-After: no call before this instant, the latest search asked again once it passes.
    not_before: Option<Instant>,
    after_wait: slint::Timer,
}

impl KitState {
    pub fn new(browse: Box<dyn Fn(&str)>, site: TldbSite) -> KitState {
        KitState { browse: Some(browse), site, ..KitState::default() }
    }

    /// The service the Kit asks, none when this run asks none.
    fn service(&self) -> Option<Service> {
        let site = match &self.site {
            TldbSite::None => return None,
            TldbSite::Real => oni_tldb::SITE,
            TldbSite::Simulated(site) => site.as_str(),
        };
        Some(Service::new(site, crate::updates::version()))
    }

    /// A simulated service's token lies in the support folder, never in a keychain.
    fn simulated(&self) -> bool {
        matches!(self.site, TldbSite::Simulated(_))
    }

    /// Where a piece of the catalogue stands against the Kit and the sheet's picks.
    fn mark(&self, p: &Piece) -> KitMark {
        let key = p.key();
        if self.kept.items.iter().any(|h| h.key() == key) {
            KitMark::Kept
        } else if self.picks.iter().any(|h| h.key() == key) {
            KitMark::Added
        } else {
            KitMark::Free
        }
    }

    /// The Kit of one kind as the column shows it, `default` the name its default card holds, `photos` how
    /// many photos name a piece.
    fn shown(&self, kind: Kind, default: Option<&str>, photos: &dyn Fn(&Piece) -> i32) -> Vec<KitPiece> {
        self.kept.of(kind).map(|p| piece(p, KitMark::Kept, words::detail(p), default == Some(p.name.as_str()), photos(p))).collect()
    }
}

fn piece(p: &Piece, mark: KitMark, detail: String, by_default: bool, photos: i32) -> KitPiece {
    KitPiece { name: p.name.as_str().into(), detail: detail.into(), url: p.url.as_str().into(), mark, by_default, photos }
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

impl Ctl {
    pub(super) fn wire_kit(me: &Rc<Ctl>, window: &crate::MainWindow) {
        {
            let mut s = me.state.borrow_mut();
            let support = s.support.clone();
            let kit = &mut s.org.kit;
            kit.kept = Kept::load(&support);
            kit.token = if kit.site == TldbSite::None { None } else { vault::read(&support, kit.simulated()) };
        }
        let k = window.global::<KitGlobal>();
        let p = me.clone();
        k.on_connect(move |signup| p.kit_connect(signup));
        let p = me.clone();
        k.on_cancel(move || p.kit_cancel());
        let p = me.clone();
        k.on_disconnect(move || p.kit_disconnect());
        let p = me.clone();
        k.on_search_gear(move || p.kit_open_search());
        let p = me.clone();
        k.on_search(move |t| p.kit_search(&t));
        let p = me.clone();
        k.on_take(move |i| p.kit_take(i.max(0) as usize));
        let p = me.clone();
        k.on_remove(move |kind, i| p.kit_remove(kind, i.max(0) as usize));
        let p = me.clone();
        k.on_prefer(move |kind, i| p.kit_prefer(kind, i.max(0) as usize));
        let p = me.clone();
        k.on_filter_kind(move |i| p.kit_filter(|kit| kit.kind_filter = [None, Some(Kind::Camera), Some(Kind::Lens)][i.clamp(0, 2) as usize]));
        let p = me.clone();
        k.on_done(move || p.kit_done());
        let p = me.clone();
        k.on_entered(move || p.kit_recount());
        let p = me.clone();
        k.on_open(move |url| p.kit_open_page(|service| service.page(&url)));
        let p = me.clone();
        k.on_open_site(move || p.kit_open_page(|service| service.page(&service.site)));
        let p = me.clone();
        k.on_suggest(move || p.kit_open_page(Service::submit_page));
        me.kit_recount();
        me.kit_refresh();
    }

    fn kit_browse(&self, url: &str) {
        if let Some(browse) = &self.state.borrow().org.kit.browse {
            browse(url);
        }
    }

    /// A page of the service, carrying `utm_source=oni`; none in a run that asks no service.
    fn kit_open_page(&self, page: impl FnOnce(&Service) -> String) {
        let Some(service) = self.state.borrow().org.kit.service() else { return };
        self.kit_browse(&page(&service));
    }

    fn client(&self) -> Option<Client> {
        let s = self.state.borrow();
        let kit = &s.org.kit;
        Some(Client { service: kit.service()?, token: kit.token.clone()? })
    }

    /// The Kit read again, at launch and once signed in; an unchanged one costs the service no body.
    fn kit_refresh(&self) {
        let Some(client) = self.client() else { return };
        if self.state.borrow().org.kit.held().is_some() {
            return;
        }
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

    /// The Kit as the service answered it replaces the copy whole, merged records included.
    fn kit_keep(&self, kit: oni_tldb::Kit, etag: Option<String>) {
        let written = {
            let mut s = self.state.borrow_mut();
            s.org.kit.kept = Kept { account: kit.account.display_name, items: kit.items, etag };
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

    /// A token the service no longer knows signs ONI out, the copy kept as the contract asks; a wait holds
    /// every call until it is over.
    fn kit_failed(&self, failure: Failure) {
        let note = self.failure_note(&failure);
        let mut s = self.state.borrow_mut();
        let support = s.support.clone();
        let kit = &mut s.org.kit;
        match failure {
            Failure::SignedOut => {
                kit.token = None;
                vault::erase(&support, kit.simulated());
            }
            Failure::Wait(seconds) => kit.not_before = Some(Instant::now() + Duration::from_secs(seconds)),
            Failure::Down(_) | Failure::Refused(_) => {}
        }
        kit.note = note;
    }

    /// Signed out at the person's word: the token revoked, the copy and the sheet's picks dropped.
    fn kit_disconnect(&self) {
        let (token, service) = {
            let mut s = self.state.borrow_mut();
            let support = s.support.clone();
            let kit = &mut s.org.kit;
            if kit.site != TldbSite::None {
                vault::erase(&support, kit.simulated());
            }
            let token = kit.token.take();
            kit.kept = Kept::default();
            kit.clear_search();
            kit.picking = false;
            kit.note.clear();
            let _ = std::fs::remove_file(Kept::file_path(&support));
            (token, kit.service())
        };
        if let (Some(token), Some(service)) = (token, service) {
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
}
