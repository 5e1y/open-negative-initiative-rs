//! Locate… and Remove from Library…, on a click only: a folder gone follows the folder picked, or
//! leaves the cache, no file touched; a place an identity suggests is offered, never followed alone.

use super::*;
use oni_library::bookmarks::{Bookmarks, Lead};
use oni_i18n::trn;
use oni_library::relink::{self, Recognised};

/// A question the sheet holds until a key answers it.
pub(in crate::app) enum Asked {
    /// Follow `from` to `to` though none of its photos was recognised there.
    Follow { from: String, to: String, recognised: Recognised },
    /// Forget the folder gone, its photos' rows and thumbnails, and the favourites under it.
    Forget(String),
}

/// The banner's lead: what it was looked for with, and what was found.
#[derive(Default)]
pub(super) struct Leads {
    looked: Option<(String, Presence, Vec<String>)>,
    lead: Option<Lead>,
    /// Bumped by each look and each check, so an answer landing after a newer ask is dropped.
    looks: u64,
    checks: u64,
}

impl State {
    /// Where the opened folder out of reach seems to be now, once looked for off this thread.
    pub(in crate::app) fn lead(&self) -> Option<&Lead> {
        let opened = self.opened.as_deref()?;
        let looked = self.nav.leads.looked.as_ref()?;
        (paths::same(&looked.0, opened) && !self.opened_presence().is_present()).then_some(self.nav.leads.lead.as_ref()).flatten()
    }
}

/// The banner's words on a lead: the volume renamed, or the folder found under another name or place.
pub(in crate::app) fn lead_words(lead: &Lead) -> String {
    let (from, to) = (paths::name(&lead.from), paths::name(&lead.to));
    if lead.volume {
        tr!("“{from}” seems to have become “{to}”.", from = from.trim_end_matches(['/', '\\']), to = to.trim_end_matches(['/', '\\']))
    } else if from != to {
        tr!("It seems to have become “{place}”.", place = to)
    } else {
        tr!("It seems to be in “{place}” now.", place = paths::name(&paths::parent(&lead.to)))
    }
}

impl Ctl {
    /// `Locate…`: the folder picker, then the photos known under `gone` looked for there.
    pub(in crate::app) fn locate(self: &Rc<Self>, gone: &str) {
        // Taken out first: a modal panel runs a nested event loop, which may reach the state.
        let picker = std::mem::replace(&mut self.state.borrow_mut().picker, Box::new(|| None));
        let chosen = picker();
        self.state.borrow_mut().picker = picker;
        match chosen {
            Some(chosen) => self.check_place(gone, &paths::canonical(&chosen), false),
            None if !crate::system::HAS_PICKER => self.told(tr("Locate"), tr("This system has no folder picker.")),
            None => {}
        }
    }

    /// The photos the index holds under `from` looked for at their places under `to` off this
    /// thread; the folder follows once one is recognised, else the sheet asks first.
    fn check_place(self: &Rc<Self>, from: &str, to: &str, volume: bool) {
        if paths::holds(from, to) {
            return self.told(tr("Locate"), &tr!("“{name}” cannot stand in for itself.", name = paths::name(from)));
        }
        let (turn, rows) = {
            let mut s = self.state.borrow_mut();
            s.nav.leads.checks += 1;
            let rows: Vec<(String, String)> = s.index.as_ref().map(|i| i.under(from)).unwrap_or_default().into_iter().map(|e| (e.path, e.fingerprint)).collect();
            (s.nav.leads.checks, rows)
        };
        let (from, to) = (from.to_owned(), to.to_owned());
        let _ = std::thread::Builder::new().name("locating".into()).spawn(move || {
            // Back meanwhile, it stays: a volume's old name may be a leftover folder, its photos not.
            let back = rows.iter().any(|(p, _)| Path::new(p).exists()) || (!volume && Path::new(&from).exists());
            let recognised = relink::recognised(&rows, &from, &to);
            with_shell(move |c| c.place_checked(turn, from, to, if back { None } else { Some(recognised) }));
        });
    }

    fn place_checked(self: &Rc<Self>, turn: u64, from: String, to: String, recognised: Option<Recognised>) {
        if self.state.borrow().nav.leads.checks != turn {
            return;
        }
        let (gone, place) = (paths::name(&from), paths::name(&to));
        let Some(recognised) = recognised else {
            return self.told(tr("Locate"), &tr!("“{name}” is there again: it stays where it is.", name = gone));
        };
        if recognised.found == 0 {
            let message = if recognised.total == 0 {
                tr!("ONI holds no photo of “{gone}” to look for in “{place}”. Follow it there all the same? No file is touched.", gone = gone, place = place)
            } else {
                trn(
                    "The photo ONI knew in “{gone}” is not in “{place}”. Follow it there all the same? No file is touched.",
                    "None of the {n} photos ONI knew in “{gone}” is in “{place}”. Follow it there all the same? No file is touched.",
                    recognised.total as u64,
                )
                .replace("{gone}", &gone)
                .replace("{place}", &place)
            };
            let asked = Asked::Follow { from, to, recognised };
            return self.ask(tr("No Photo Recognised"), &message, tr("Follow Anyway"), false, super::super::Pending::Locate(asked));
        }
        let said = trn("{n} photo of {total} recognised in “{place}”: “{gone}” follows it.", "{n} photos of {total} recognised in “{place}”: “{gone}” follows it.", recognised.found as u64)
            .replace("{total}", &recognised.total.to_string())
            .replace("{place}", &place)
            .replace("{gone}", &gone);
        self.follow_located(&from, &to, &recognised);
        self.told(tr("Located"), &said);
    }

    /// What the sheet held, once a key went ahead.
    pub(in crate::app) fn locate_answered(self: &Rc<Self>, asked: Asked) {
        match asked {
            Asked::Follow { from, to, recognised } => self.follow_located(&from, &to, &recognised),
            Asked::Forget(folder) => self.forget_confirmed(folder),
        }
    }

    /// A row whose place under `to` holds another file loses its row, so that file is stamped
    /// afresh and never wears another's print; then every record follows the folder.
    fn follow_located(self: &Rc<Self>, from: &str, to: &str, recognised: &Recognised) {
        if let Some(index) = &self.state.borrow().index {
            for path in &recognised.other {
                let _ = index.remove(path);
            }
        }
        self.follow_folder(from, to);
    }

    /// A folder renamed in ONI or located: the navigator, the favourites, the opened folder, then
    /// the index rows, the picks, the folds and the projects' paths follow it, the folder read again.
    pub(in crate::app) fn follow_folder(self: &Rc<Self>, old: &str, new: &str) {
        let failure = {
            let mut s = self.state.borrow_mut();
            s.nav.model.follow(old, new);
            s.nav.favourites.follow(old, new);
            let failure = s.save_favourites();
            if let Some(opened) = s.opened.clone().filter(|o| holds(old, o)) {
                let moved = paths::moved(&opened, old, new);
                let _ = s.preferences.set(crate::preferences::OPENED_FOLDER, Value::String(moved.clone()));
                s.opened = Some(moved);
            }
            // What was known of the old places says nothing of the new ones.
            s.nav.presence.retain(|p, _| !holds(old, p));
            s.nav.leads.looked = None;
            s.nav.leads.lead = None;
            failure
        };
        if let Some(message) = failure {
            self.told(tr("Something Went Wrong"), &message);
        }
        self.followed(&[Move { from: old.to_owned(), to: new.to_owned() }]);
        self.publish_navigator();
    }

    /// `Remove from Library…`: a folder gone from a disk that answers, once the sheet confirms;
    /// never one whose disk is away.
    pub(in crate::app) fn ask_forget(self: &Rc<Self>, folder: &str) {
        let (count, gone) = {
            let s = self.state.borrow();
            (s.index.as_ref().map_or(0, |i| i.under(folder).len()), s.folder_presence(folder) == Presence::Missing)
        };
        if !gone {
            return;
        }
        let name = paths::name(folder);
        let message = if count == 0 {
            tr!("ONI holds no photo under “{folder}”: only the folder is forgotten. No file is touched.", folder = folder)
        } else {
            trn(
                "ONI forgets the photo it knows in “{folder}”. No file is touched: if the folder comes back, its photo returns with its settings.",
                "ONI forgets the {n} photos it knows in “{folder}”. No file is touched: if the folder comes back, its photos return with their settings.",
                count as u64,
            )
            .replace("{folder}", folder)
        };
        let title = tr!("Remove “{name}” from the Library?", name = name);
        self.ask(&title, &message, tr("Remove from Library"), true, super::super::Pending::Locate(Asked::Forget(folder.to_owned())));
    }

    /// The folder looked at again off this thread first: only one still gone is forgotten.
    fn forget_confirmed(self: &Rc<Self>, folder: String) {
        let computer = self.state.borrow().nav.computer.clone();
        let _ = std::thread::Builder::new().name("forgetting".into()).spawn(move || {
            let state = presence::of(&folder, &computer);
            with_shell(move |c| c.forget(&folder, state));
        });
    }

    /// The rows under `folder` out of the index, their thumbnails once no other row claims them,
    /// the favourites and folds under it and its kept shape; never a sidecar nor an original.
    pub(in crate::app) fn forget(self: &Rc<Self>, folder: &str, state: Presence) {
        let name = paths::name(folder);
        if state != Presence::Missing {
            let why = if state.is_present() {
                tr!("“{name}” is there again: nothing was forgotten.", name = name)
            } else {
                tr!("“{name}” is out of reach, not gone: ONI keeps it until its disk is back.", name = name)
            };
            return self.told(tr("Remove from Library"), &why);
        }
        let outcome = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let forgotten = match &s.index {
                Some(index) => index.forget(folder).map_err(|e| tr!("The library index could not be changed: {error}", error = e)),
                None => Err(tr("The library index cannot be opened").to_string()),
            };
            match forgotten {
                Err(e) => Err(e),
                Ok(gone) => {
                    let claims = |f: &String| s.index.as_ref().is_some_and(|i| i.claims(f));
                    let mut prints: Vec<String> = gone.iter().map(|e| e.fingerprint.clone()).filter(|f| !claims(f)).collect();
                    prints.sort();
                    prints.dedup();
                    for e in &gone {
                        s.thumbs.remove(&e.path);
                    }
                    let failure = if s.nav.favourites.forget(folder) > 0 { s.save_favourites() } else { None };
                    s.nav.model.unfolded.retain(|p| !holds(folder, p));
                    s.remember();
                    s.nav.presence.retain(|p, _| !holds(folder, p));
                    s.nav.leads.looked = None;
                    s.nav.leads.lead = None;
                    let kept = s.index_path.parent().map(|caches| oni_library::kept::file(&caches.join("Surveys"), folder));
                    let opened = s.opened.as_deref().is_some_and(|o| holds(o, folder) || holds(folder, o));
                    Ok((prints, kept, s.support.clone(), failure, opened))
                }
            }
        };
        let (prints, kept, support, failure, opened) = match outcome {
            Ok(done) => done,
            Err(e) => return self.told(tr("Remove from Library"), &e),
        };
        if let Some(message) = failure {
            self.told(tr("Something Went Wrong"), &message);
        }
        // The caches' own files alone, in the support and caches folders, off this thread.
        let _ = std::thread::Builder::new().name("forgetting".into()).spawn(move || {
            for print in &prints {
                crate::thumbnails::forget(&support, print);
            }
            if let Some(kept) = kept {
                let _ = std::fs::remove_file(kept);
            }
        });
        self.read_library();
        if opened {
            self.read_again();
        } else {
            self.lay_grid();
        }
        self.publish_navigator();
    }

    /// The opened folder out of reach looked for by the identities `bookmarks.json` keeps, off
    /// this thread, each time it, its state or the mounted volumes change.
    pub(in crate::app) fn look_for_lead(&self) {
        let (job, dropped) = {
            let mut s = self.state.borrow_mut();
            let presence = s.opened_presence();
            let key = s.opened.clone().map(|o| (o, presence, s.nav.model.volumes.clone()));
            if s.nav.leads.looked == key {
                return;
            }
            s.nav.leads.looked = key.clone();
            let dropped = s.nav.leads.lead.take().is_some();
            s.nav.leads.looks += 1;
            let job = key.filter(|(_, p, _)| matches!(p, Presence::Missing | Presence::Offline(Away::Disconnected))).map(|(opened, _, mounted)| {
                let favourites = s.nav.favourites.paths.clone();
                (s.nav.leads.looks, opened, mounted, favourites, s.nav.computer.clone(), s.nav.identify.clone(), s.support.clone())
            });
            (job, dropped)
        };
        if dropped {
            self.publish_banner();
        }
        let Some((turn, opened, mounted, favourites, computer, identify, support)) = job else { return };
        let _ = std::thread::Builder::new().name("leads".into()).spawn(move || {
            // The favourites as they stand: a place still noted for one forgotten leads nowhere.
            let mut bookmarks = Bookmarks::load(&support);
            bookmarks.places.retain(|p, _| favourites.contains(p));
            let lead = bookmarks.lead(&opened, &computer, &mounted, &identify);
            with_shell(move |c| c.lead_found(turn, lead));
        });
    }

    fn lead_found(&self, turn: u64, lead: Option<Lead>) {
        {
            let mut s = self.state.borrow_mut();
            if s.nav.leads.looks != turn || s.nav.leads.lead == lead {
                return;
            }
            s.nav.leads.lead = lead;
        }
        self.publish_banner();
    }

    /// The banner's Follow: the lead checked by its photos as a folder picked by hand would be.
    pub(in crate::app) fn follow_lead(self: &Rc<Self>) {
        let lead = self.state.borrow().lead().cloned();
        if let Some(lead) = lead {
            self.check_place(&lead.from, &lead.to, lead.volume);
        }
    }
}
