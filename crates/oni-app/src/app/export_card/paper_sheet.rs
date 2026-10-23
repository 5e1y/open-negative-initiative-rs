//! `PaperProfilesSheet`: the makers and labs that publish ICC profiles, in a sidebar, and what the
//! chosen one offers in a card. A direct source lists and installs; any other opens its page.

use super::super::{Ctl, with_shell};
use crate::{Exporting, PaperRow};
use oni_colour::{InstallRecord, PaperSource, Reach, Space};
use oni_i18n::{tr, trn};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use url::Url;

pub type Fetch = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;
pub type Download = Arc<dyn Fn(&str, &Path) -> Result<PathBuf, String> + Send + Sync>;

/// What the sheet asks of the world: a page, a file, the browser, the Trash. A check gives its own.
#[derive(Clone)]
pub struct Web {
    pub fetch: Fetch,
    pub download: Download,
    pub open: Rc<dyn Fn(&str)>,
    pub discard: Rc<dyn Fn(&Path) -> std::io::Result<()>>,
}

impl Web {
    pub fn system() -> Web {
        Web {
            fetch: Arc::new(crate::web::text),
            download: Arc::new(crate::web::download),
            open: Rc::new(crate::system::open_url),
            discard: Rc::new(crate::system::trash),
        }
    }
}

pub struct Sheet {
    pub shown: bool,
    /// Never out of `PaperSource::all`: the sidebar always names what the card shows.
    pub source: usize,
    pub offered: Vec<Url>,
    pub installed: HashSet<Url>,
    pub filter: String,
    pub reading: bool,
    pub installing: Option<Url>,
    pub message: Option<String>,
    /// Each read carries a generation; a later choice of source drops an earlier page.
    reads: Arc<AtomicU64>,
    pub web: Web,
}

impl Default for Sheet {
    fn default() -> Sheet {
        Sheet {
            shown: false,
            source: 0,
            offered: Vec::new(),
            installed: HashSet::new(),
            filter: String::new(),
            reading: false,
            installing: None,
            message: None,
            reads: Arc::default(),
            web: Web::system(),
        }
    }
}

impl Sheet {
    /// The offered links the filter's words all appear in, as titled.
    pub fn shown_links(&self) -> Vec<Url> {
        let filter = self.filter.to_lowercase();
        let words: Vec<&str> = filter.split_whitespace().collect();
        self.offered
            .iter()
            .filter(|u| {
                let title = PaperSource::title(u).to_lowercase();
                words.iter().all(|w| title.contains(w))
            })
            .cloned()
            .collect()
    }
}

fn direct(s: &PaperSource) -> bool {
    !matches!(s.reach, Reach::Browser)
}

/// `PaperSource.install`'s scratch folder: the support folder's own, one per download.
fn scratch(support: &Path) -> PathBuf {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    support.join("Scratch").join(format!("paper-download-{:08x}", stamp as u32))
}

impl Ctl {
    fn record(&self) -> InstallRecord {
        let c = self.export.borrow();
        c.prefs.value(InstallRecord::KEY).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
    }

    fn keep_record(&self, record: &InstallRecord) {
        if let Ok(value) = serde_json::to_value(record) {
            self.export.borrow_mut().save(InstallRecord::KEY, value);
        }
    }

    pub fn open_papers(self: &Rc<Self>) {
        {
            let mut c = self.export.borrow_mut();
            c.papers.shown = true;
            c.papers.source = 0;
        }
        self.read_paper_page();
    }

    fn close_papers(self: &Rc<Self>) {
        self.export.borrow_mut().papers.shown = false;
        self.publish_papers();
    }

    fn pick_paper_source(self: &Rc<Self>, at: usize) {
        if at >= PaperSource::all().len() || at == self.export.borrow().papers.source {
            return;
        }
        self.export.borrow_mut().papers.source = at;
        self.read_paper_page();
    }

    /// `read()`: the chosen source's page read off this thread, its installed links marked on return.
    fn read_paper_page(self: &Rc<Self>) {
        let (source, fetch, generation, reads) = {
            let mut c = self.export.borrow_mut();
            let p = &mut c.papers;
            (p.offered, p.filter, p.message, p.reading) = (Vec::new(), String::new(), None, true);
            let generation = p.reads.fetch_add(1, Ordering::Relaxed) + 1;
            (PaperSource::all()[p.source].clone(), p.web.fetch.clone(), generation, p.reads.clone())
        };
        self.publish_papers();
        std::thread::spawn(move || {
            let read = source.offered(|page| fetch(page));
            with_shell(move |c| {
                if reads.load(Ordering::Relaxed) != generation {
                    return;
                }
                let record = c.record();
                {
                    let mut card = c.export.borrow_mut();
                    let folder = card.profiles.clone();
                    let p = &mut card.papers;
                    p.reading = false;
                    match read {
                        Ok(offered) => {
                            p.installed = offered.iter().filter(|u| !record.installed_files(u, &folder).is_empty()).cloned().collect();
                            p.offered = offered;
                        }
                        Err(e) => p.message = Some(tr!("The page could not be read: {error}", error = e)),
                    }
                }
                c.publish_papers();
            });
        });
    }

    fn paper_link(&self, row: usize) -> Option<Url> {
        let c = self.export.borrow();
        if c.papers.installing.is_some() {
            return None;
        }
        c.papers.shown_links().get(row).cloned()
    }

    /// `install(url)`: downloaded off this thread, adopted into the profiles, the first one chosen.
    fn install_paper(self: &Rc<Self>, row: usize) {
        let Some(link) = self.paper_link(row) else { return };
        let (download, folder) = {
            let mut c = self.export.borrow_mut();
            c.papers.installing = Some(link.clone());
            (c.papers.web.download.clone(), scratch(&c.support))
        };
        self.say(&tr!("Downloading {source}", source = PaperSource::title(&link)));
        self.publish_papers();
        std::thread::spawn(move || {
            let file = download(link.as_str(), &folder);
            with_shell(move |c| {
                c.say("");
                let (profiles, support) = {
                    let card = c.export.borrow();
                    (card.profiles.clone(), card.support.clone())
                };
                let message = match file {
                    Ok(file) => {
                        let mut record = c.record();
                        let (installed, refused) = record.adopt(&file, &link, &profiles, &support.join("Scratch"));
                        c.keep_record(&record);
                        if let Some(first) = installed.first() {
                            c.export.borrow_mut().papers.installed.insert(link.clone());
                            let message = trn(
                                "Installed {n} profile — the export now uses “{name}”.",
                                "Installed {n} profiles — the export now uses “{name}”.",
                                installed.len() as u64,
                            )
                            .replace("{name}", &first.name);
                            c.choose_profile(first.clone());
                            c.list_profiles();
                            message
                        } else {
                            if refused > 0 {
                                tr("Nothing to install: the file held no RGB printer profile (CMYK profiles are not supported).").into()
                            } else {
                                tr("Nothing to install: the file held no RGB printer profile.").into()
                            }
                        }
                    }
                    Err(e) => tr!("The download failed: {error}", error = e),
                };
                let _ = std::fs::remove_dir_all(&folder);
                {
                    let mut card = c.export.borrow_mut();
                    card.papers.installing = None;
                    card.papers.message = Some(message);
                }
                c.publish_papers();
            });
        });
    }

    /// `uninstall(url)`: to the Trash; the working space leaves a profile that was the one in use.
    fn uninstall_paper(self: &Rc<Self>, row: usize) {
        let Some(link) = self.paper_link(row) else { return };
        let (folder, discard) = {
            let c = self.export.borrow();
            (c.profiles.clone(), c.papers.web.discard.clone())
        };
        let mut record = self.record();
        let removed = record.uninstall(&link, &folder, |f| discard(f));
        self.keep_record(&record);
        {
            let mut c = self.export.borrow_mut();
            c.papers.installed.remove(&link);
            c.papers.message = Some(if removed.is_empty() {
                tr("Nothing was left to remove.").into()
            } else {
                tr!("Moved {source} to the Trash.", source = PaperSource::title(&link))
            });
        }
        let in_use = matches!(self.export.borrow().settings.space(), Space::Profile(p) if removed.contains(&p.path));
        {
            let mut c = self.export.borrow_mut();
            if removed.contains(&c.last_profile) {
                c.last_profile.clear();
                c.save(super::LAST_PROFILE, serde_json::Value::from(""));
            }
        }
        if in_use {
            self.set_space(oni_export::Settings::default().space().clone());
        }
        self.list_profiles();
        self.publish_papers();
    }

    pub(super) fn publish_papers(&self) {
        let Some(w) = self.window() else { return };
        let c = self.export.borrow();
        let p = &c.papers;
        let e = w.global::<Exporting>();
        let all = PaperSource::all();
        let source = &all[p.source];
        let names = |d: bool| {
            let list: Vec<SharedString> = all.iter().filter(|s| direct(s) == d).map(|s| s.name.into()).collect();
            ModelRc::new(VecModel::from(list))
        };
        e.set_papers_shown(p.shown);
        e.set_paper_direct(names(true));
        e.set_paper_browsed(names(false));
        e.set_paper_source(p.source as i32);
        e.set_paper_direct_count(all.iter().filter(|s| direct(s)).count() as i32);
        e.set_paper_name(source.name.into());
        e.set_paper_note(source.note.into());
        e.set_paper_browser(!direct(source));
        e.set_paper_reading(p.reading);
        e.set_paper_any(!p.offered.is_empty());
        e.set_paper_filter(p.filter.clone().into());
        e.set_paper_busy(p.installing.is_some());
        e.set_paper_message(p.message.clone().unwrap_or_default().into());
        let rows: Vec<PaperRow> = p
            .shown_links()
            .iter()
            .map(|u| PaperRow { title: PaperSource::title(u).into(), installed: p.installed.contains(u), installing: p.installing.as_ref() == Some(u) })
            .collect();
        e.set_paper_rows(ModelRc::new(VecModel::from(rows)));
    }

    pub(super) fn wire_papers(me: &Rc<Ctl>, e: &Exporting) {
        let p = me.clone();
        e.on_get_papers(move || p.open_papers());
        let p = me.clone();
        e.on_papers_done(move || p.close_papers());
        let p = me.clone();
        e.on_paper_picked(move |at| p.pick_paper_source(at.max(0) as usize));
        let p = me.clone();
        e.on_paper_open_page(move || {
            let (open, page) = {
                let c = p.export.borrow();
                (c.papers.web.open.clone(), PaperSource::all()[c.papers.source].page)
            };
            open(page);
        });
        let p = me.clone();
        e.on_paper_filter_typed(move |text| {
            p.export.borrow_mut().papers.filter = text.to_string();
            p.publish_papers();
        });
        let p = me.clone();
        e.on_paper_install(move |row| p.install_paper(row.max(0) as usize));
        let p = me.clone();
        e.on_paper_uninstall(move |row| p.uninstall_paper(row.max(0) as usize));
    }
}

/// A check reads pages, downloads and discards through its own hands.
pub fn web_with(ctl: &Ctl, web: Web) {
    ctl.export.borrow_mut().papers.web = web;
}

/// The sheet's state, read back by the checks.
pub fn sheet(ctl: &Ctl) -> std::cell::Ref<'_, Sheet> {
    std::cell::Ref::map(ctl.export.borrow(), |c| &c.papers)
}
