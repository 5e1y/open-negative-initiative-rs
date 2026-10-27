//! `Updates` in the window: the feed read on a schedule off this thread, the bar's UPDATE key while
//! an update waits, and a check asked for ending on Sparkle's dialogs; a verified archive installs itself.

use super::{Ctl, Pending, with_shell};
use crate::updates::{self as feed, Item};
use crate::{Bar, Confirm};
use oni_i18n::tr;
use slint::{ComponentHandle, Timer, TimerMode};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// A body read whole from the web; a check hands its own and never reaches the network.
pub type Fetch = Arc<dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync>;
/// Where a downloaded archive is shown: the Finder, or a check's own record.
pub type Reveal = Arc<dyn Fn(&Path) + Send + Sync>;

pub struct Updater {
    pub fetch: Fetch,
    /// The folder a verified archive is kept in.
    pub downloads: PathBuf,
    pub reveal: Reveal,
    /// The build compared with the feed and the system its items must run on.
    pub build: String,
    pub system: Option<String>,
    /// `SUPublicEDKey`, which every archive kept must be signed for.
    pub public_key: String,
    /// `SUEnableAutomaticChecks`: on for a published build; a development build looks only when asked.
    pub automatic: bool,
    /// The Swift app's last launched version on this Mac, read only when ours has none.
    pub swift_prior: Arc<dyn Fn() -> Option<String> + Send + Sync>,
}

impl Updater {
    /// The app's own: the web, `~/Downloads`, the Finder, this build on this system.
    pub fn system() -> Updater {
        Updater {
            fetch: Arc::new(crate::web::bytes),
            downloads: downloads(),
            reveal: Arc::new(|p: &Path| crate::system::reveal(p)),
            build: feed::build().to_string(),
            system: feed::system(),
            public_key: feed::PUBLIC_KEY.to_string(),
            automatic: feed::stamped(),
            swift_prior: Arc::new(feed::prior::swift),
        }
    }
}

fn downloads() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default();
    home.join("Downloads")
}

/// A name in `folder` no file holds yet: the archive never writes over one already there.
fn free_name(folder: &Path, name: &str) -> PathBuf {
    let (stem, extension) = name.rsplit_once('.').map_or((name, String::new()), |(s, e)| (s, format!(".{e}")));
    (1..).map(|n| if n == 1 { folder.join(name) } else { folder.join(format!("{stem} {n}{extension}")) }).find(|p| !p.exists()).unwrap_or_else(|| folder.join(name))
}

#[derive(Default)]
pub struct Updates {
    pub(crate) updater: Option<Updater>,
    /// `available`: the item found, which the bar's key names.
    pub available: Option<Item>,
    pub checking: bool,
    /// A check from the key or the menu, which ends on a dialog.
    asked: bool,
    pub downloading: bool,
    timer: Option<Timer>,
}

impl Updates {
    pub fn new(updater: Option<Updater>) -> Updates {
        Updates { updater, ..Default::default() }
    }
}

impl Ctl {
    /// `Updates.start()`: idempotent, so the first scheduled check is armed once.
    pub(super) fn start_updates(self: &Rc<Self>) {
        let automatic = self.state.borrow().updates.updater.as_ref().is_some_and(|u| u.automatic);
        if automatic && self.state.borrow().updates.timer.is_none() {
            self.report_install();
            self.schedule_update_check();
        }
    }

    /// `reportInstall`: an installation or an update reported once per version, remembered only
    /// once the recorder answers, so an offline launch tries again at the next.
    fn report_install(self: &Rc<Self>) {
        let version = feed::version().to_string();
        let (fetch, swift, ours, checked) = {
            let s = self.state.borrow();
            let Some(updater) = s.updates.updater.as_ref() else { return };
            let ours = s.preferences.string(feed::LAST_LAUNCHED_KEY).map(str::to_string);
            if ours.as_deref() == Some(version.as_str()) {
                return;
            }
            (updater.fetch.clone(), updater.swift_prior.clone(), ours, s.preferences.string(feed::LAST_CHECK_KEY).is_some())
        };
        std::thread::spawn(move || {
            let swift = if ours.is_none() { swift() } else { None };
            let Some(from) = feed::prior::from(&version, ours, swift, checked) else {
                return with_shell(move |c| c.remember(feed::LAST_LAUNCHED_KEY, version.into()));
            };
            let url = feed::installed_url(&version, from.as_deref());
            if fetch(&url).is_ok() {
                with_shell(move |c| c.remember(feed::LAST_LAUNCHED_KEY, version.into()));
            }
        });
    }

    /// `scheduleNextUpdateCheck`: the interval since the last check, a clock gone back counting none.
    fn schedule_update_check(self: &Rc<Self>) {
        let last = self.state.borrow().preferences.string(feed::LAST_CHECK_KEY).and_then(oni_settings::Date::parse_iso8601);
        let since = last.map_or(f64::INFINITY, |d| (oni_settings::Date::now().unix_seconds() - d.unix_seconds()).max(0.0));
        let delay = Duration::from_secs_f64((feed::CHECK_INTERVAL.as_secs_f64() - since).max(0.0));
        let timer = Timer::default();
        let me = Rc::downgrade(self);
        timer.start(TimerMode::SingleShot, delay, move || {
            if let Some(me) = me.upgrade() {
                me.check_for_updates(false);
            }
        });
        self.state.borrow_mut().updates.timer = Some(timer);
    }

    /// `checkNow()`: the feed read off this thread; one check at a time, a later ask joining it.
    pub(super) fn check_for_updates(self: &Rc<Self>, asked: bool) {
        let (fetch, build, system) = {
            let mut s = self.state.borrow_mut();
            let u = &mut s.updates;
            u.asked |= asked;
            if u.checking {
                return;
            }
            let Some(updater) = u.updater.as_ref() else {
                drop(s);
                return self.update_checked(Err("no updater".into()));
            };
            u.checking = true;
            (updater.fetch.clone(), updater.build.clone(), updater.system.clone())
        };
        self.remember(feed::LAST_CHECK_KEY, oni_settings::Date::now().iso8601().into());
        self.publish_update_key();
        std::thread::spawn(move || {
            let found = fetch(feed::FEED).and_then(|bytes| feed::parse(&String::from_utf8_lossy(&bytes))).map(|items| feed::newest(&items, &build, system.as_deref()));
            with_shell(move |c| c.update_checked(found));
        });
    }

    /// `didFindValidUpdate` and `updaterDidNotFindUpdate`; a failure leaves what was found standing.
    fn update_checked(self: &Rc<Self>, found: Result<Option<Item>, String>) {
        let asked = {
            let mut s = self.state.borrow_mut();
            let u = &mut s.updates;
            u.checking = false;
            if let Ok(item) = &found {
                u.available = item.clone();
            }
            std::mem::take(&mut u.asked)
        };
        if self.state.borrow().updates.updater.as_ref().is_some_and(|u| u.automatic) {
            self.schedule_update_check();
        }
        self.publish_update_key();
        if !asked {
            return;
        }
        match found {
            Ok(Some(item)) => {
                let (title, message) = feed::found(&item);
                self.state.borrow_mut().pending = Some(Pending::DownloadUpdate);
                if let Some(w) = self.window() {
                    let c = w.global::<Confirm>();
                    c.set_title(title.into());
                    c.set_message(message.into());
                    c.set_key(tr("DOWNLOAD").into());
                    c.set_shown(true);
                    w.invoke_refocus();
                }
            }
            Ok(None) => {
                let (title, message) = feed::up_to_date();
                self.advise(&title, &message);
            }
            Err(_) => self.advise(tr(feed::ERROR_TITLE), tr(feed::CHECK_FAILED)),
        }
        self.publish_bar();
    }

    /// The archive fetched, kept only once its length and its EdDSA signature hold, then shown.
    pub(super) fn download_update(self: &Rc<Self>) {
        let (enclosure, fetch, folder, key) = {
            let mut s = self.state.borrow_mut();
            let u = &mut s.updates;
            let (Some(item), Some(updater)) = (u.available.as_ref(), u.updater.as_ref()) else { return };
            let Some(enclosure) = item.enclosure.clone() else { return };
            if u.downloading {
                return;
            }
            u.downloading = true;
            (enclosure, updater.fetch.clone(), updater.downloads.clone(), updater.public_key.clone())
        };
        self.publish_update_key();
        std::thread::spawn(move || {
            let failed = || tr(feed::DOWNLOAD_FAILED).to_string();
            let kept = fetch(&enclosure.url).map_err(|_| failed()).and_then(|bytes| {
                feed::checked(&enclosure, &bytes, &key).map_err(|e| tr(e).to_string())?;
                std::fs::create_dir_all(&folder).map_err(|_| failed())?;
                let path = free_name(&folder, &feed::file_name(&enclosure.url));
                std::fs::write(&path, &bytes).map_err(|_| failed())?;
                Ok(path)
            });
            with_shell(move |c| c.update_downloaded(kept));
        });
    }

    fn update_downloaded(self: &Rc<Self>, kept: Result<PathBuf, String>) {
        let reveal = {
            let mut s = self.state.borrow_mut();
            s.updates.downloading = false;
            s.updates.updater.as_ref().map(|u| u.reveal.clone())
        };
        match kept {
            Ok(path) => {
                // The update lays itself in place and the app reopens on it.
                match crate::update_install::prepare(&path) {
                    Ok(()) => {
                        self.state.borrow_mut().quitting = true;
                        let _ = slint::quit_event_loop();
                        return;
                    }
                    Err(why) => crate::timing::line(&format!("update left to install by hand: {why}")),
                }
                if let Some(reveal) = reveal {
                    reveal(&path);
                }
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.say(&tr!("{name} is in Downloads, its signature checked: open it to install.", name = name));
            }
            Err(message) => self.advise(tr(feed::ERROR_TITLE), &message),
        }
        self.publish_update_key();
    }

    /// The export slot's third face, or none.
    pub(super) fn publish_update_key(&self) {
        let Some(w) = self.window() else { return };
        let s = self.state.borrow();
        let u = &s.updates;
        let bar = w.global::<Bar>();
        let display = u.available.as_ref().map(|i| i.display.clone());
        bar.set_update_title(display.as_deref().map(feed::key_title).unwrap_or_default().into());
        bar.set_update_help(display.as_deref().map(feed::key_help).unwrap_or_default().into());
        bar.set_updating(u.downloading || (u.checking && u.asked));
    }
}
