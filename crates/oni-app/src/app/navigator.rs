//! The navigator in the window (`FolderNavigator`, `RootWelcome`, `RootsCatchUp`): folders listed off
//! this thread, the favourites kept as bookmarks, the folder menu, what a launch finds.

use super::{Ctl, Pending, State, with_shell};
use super::organising::reach;
use crate::{App, CatchUpLine, FolderChoice, FolderLine, MenuLine, WelcomeLine};
use oni_i18n::tr;
use oni_library::favourites::{self, Favourites};
use oni_library::gallery_history::Move;
use oni_library::navigator::{self as walk, Computer, Landed, Navigation, TRASH_ROW};
use oni_library::paths::{self, holds};
use oni_library::presence::{self, Away, Presence};
use oni_library::roots::{self, Roots};
use oni_library::{archive_tree::NodeId, folders};
use serde_json::Value;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod memory;
mod rows;
mod folder_menu;
pub(in crate::app) use folder_menu::reveal_word;
mod reading;
mod catch_up;
mod locate;
pub(in crate::app) use locate::{Asked, lead_words};
mod freshness;
mod keys;

/// `FolderNavigation`'s preference keys, as Swift's `UserDefaults` holds them.
pub const BASE: &str = "navigatorBase";
pub const FAVOURITES_ONLY: &str = "navigatorFavouritesOnly";
pub const UNFOLDED: &str = "navigatorUnfolded";
pub const EDITOR_HEIGHT: &str = "navigatorEditorHeight";
/// `EditorScene.rootsCatchUpShown`: the catch-up sheet was dismissed once.
pub const ROOTS_CATCH_UP_SHOWN: &str = "rootsCatchUpShown";
/// `DS.Control.navigatorEditorRest`: the rows' height beside a photograph until one is dragged.
pub const EDITOR_REST: f64 = 200.0;
/// The threads reading listings at once, as a detached task each would on a small pool.
const LISTERS: usize = 4;
/// How often the mounted volumes are read again, where Swift waits for a mount's notification.
const MOUNTS: Duration = Duration::from_secs(1);
/// `RootsCatchUp`'s folders shown before "and N more folders".
const CATCH_UP_SHOWN: usize = 6;

/// The folder menu's lines, one set wherever a folder is right-clicked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) enum FolderAct {
    Open,
    Favourite,
    Rename,
    Roll(bool),
    NewFolder,
    Reveal,
    RevealParent,
    Locate,
    Forget,
}

pub(super) struct Nav {
    pub computer: Computer,
    pub model: Navigation,
    pub favourites: Favourites,
    /// `favourites.json` as last read or written, which tells this app's changes from another's.
    pub favourites_read: Favourites,
    remembered: Option<(Option<String>, bool, Vec<String>)>,
    /// The last click: its row, when, and the count it reached, as AppKit's `clickCount` keeps it.
    clicks: Option<(String, Instant, usize)>,
    /// The rows as published, each one's path.
    rows: Vec<String>,
    menu: Vec<FolderAct>,
    menu_path: Option<String>,
    /// The open photo's folder last revealed beside it.
    current: Option<String>,
    /// `RootWelcome`'s folders, and the former library's photos `RootsCatchUp` counts.
    welcome: Vec<String>,
    former: Vec<String>,
    pub catching_up: bool,
    catch_up_folders: Vec<String>,
    opened_rolls: HashSet<String>,
    /// The listed rows whose roll marker their listing found.
    listed_rolls: HashSet<String>,
    /// Make a Roll and Back to a Folder until the folder is read again.
    roll_edits: HashMap<String, bool>,
    /// Each roll's stock: the opened folder's as its tree draws them, a listed one's from its marker.
    opened_stocks: HashMap<String, &'static str>,
    listed_stocks: HashMap<String, &'static str>,
    /// Held by the mounts' reader, which ends once the window's state is gone.
    watcher: Arc<()>,
    /// The folders the disk refused, the opened one among them, which the mounts' reader tries.
    refused: Arc<std::sync::Mutex<Vec<String>>>,
    /// The rows' folders a listing found offline or missing, looked at off this thread.
    presence: HashMap<String, Presence>,
    /// Names the volumes a favourite lives on, which `bookmarks.json` keeps beside its path.
    identify: oni_library::bookmarks::Identify,
    leads: locate::Leads,
    fresh: freshness::Fresh,
    /// The arrows walk the rows, from a click on one until a click on the grid.
    pub keys: bool,
    /// A favourite just taken away, which the notice's Undo stars again.
    pub unstarred: Option<String>,
}

impl Nav {
    pub fn new(computer: Computer, identify: oni_library::bookmarks::Identify) -> Nav {
        Nav {
            computer,
            model: Navigation::default(),
            favourites: Favourites::default(),
            favourites_read: Favourites::default(),
            remembered: None,
            clicks: None,
            rows: Vec::new(),
            menu: Vec::new(),
            menu_path: None,
            current: None,
            welcome: Vec::new(),
            former: Vec::new(),
            catching_up: false,
            catch_up_folders: Vec::new(),
            opened_rolls: HashSet::new(),
            listed_rolls: HashSet::new(),
            roll_edits: HashMap::new(),
            opened_stocks: HashMap::new(),
            listed_stocks: HashMap::new(),
            watcher: Arc::new(()),
            refused: Arc::default(),
            presence: HashMap::new(),
            identify,
            leads: locate::Leads::default(),
            fresh: Default::default(),
            keys: false,
            unstarred: None,
        }
    }

    /// A folder made a roll or a folder again by a hand, shown before the disk is listed again.
    pub(in crate::app) fn roll_edited(&mut self, folder: &str, roll: bool) {
        self.roll_edits.insert(folder.to_owned(), roll);
    }

    /// `rollFolders`: the opened folder's rolls and the listed rows', with the hand's edits on top.
    /// The roll's stock, else the negative default its tree row draws when its photos are not read.
    pub(super) fn stock(&self, folder: &str) -> &'static str {
        self.opened_stocks
            .get(folder)
            .or_else(|| self.listed_stocks.get(folder))
            .copied()
            .unwrap_or_else(|| oni_settings::film::default_stock_id(oni_settings::ConversionMode::Negative))
    }

    /// The opened folder's roll stocks; true when one changed, so the rows are laid again.
    pub(super) fn set_opened_stocks(&mut self, stocks: HashMap<String, &'static str>) -> bool {
        let changed = stocks != self.opened_stocks;
        self.opened_stocks = stocks;
        changed
    }

    pub(super) fn rolls(&self) -> HashSet<String> {
        let mut out: HashSet<String> = self.opened_rolls.union(&self.listed_rolls).cloned().collect();
        for (folder, roll) in &self.roll_edits {
            if *roll {
                out.insert(folder.clone());
            } else {
                out.remove(folder);
            }
        }
        out
    }
}

fn line(text: &str, enabled: bool, rule: bool) -> MenuLine {
    MenuLine { text: text.into(), enabled, rule, opens: false }
}

/// `.whitespaces`: spaces and tabs, never a line's end.
fn trim_spaces(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() && !matches!(c, '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'))
}

/// Whether two paths name one item, as a rename changing only a letter's case finds on macOS.
fn same_item(a: &str, b: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(x), Ok(y)) = (std::fs::metadata(a), std::fs::metadata(b)) {
            return x.dev() == y.dev() && x.ino() == y.ino();
        }
    }
    // Off Unix both paths resolve to the one spelling on disk when they name one item.
    #[cfg(not(unix))]
    if let (Ok(x), Ok(y)) = (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        return x == y;
    }
    let _ = (a, b);
    false
}

impl State {
    /// Off its volumes or on one mounted, from the path alone.
    pub(in crate::app) fn present(&self, path: &str) -> bool {
        let c = &self.nav.computer;
        !c.is_on_volume(path) || self.nav.model.volumes.iter().any(|v| holds(v, path))
    }

    /// A navigator row's state: its volume away from the path alone, else the opened folder's or
    /// what its last listing or read found; never a look at the disk.
    pub fn row_presence(&self, path: &str) -> Presence {
        if !self.present(path) || self.org.left.iter().any(|v| holds(v, path)) {
            return Presence::Offline(Away::Disconnected);
        }
        if self.opened.as_deref().is_some_and(|o| paths::same(o, path)) {
            return self.opened_presence();
        }
        self.nav.presence.get(path).copied().unwrap_or_default()
    }

    /// States looked at off this thread, a present one forgotten.
    fn noted(&mut self, found: Vec<(String, Presence)>) {
        for (path, state) in found {
            if state.is_present() {
                self.nav.presence.remove(&path);
            } else {
                self.nav.presence.insert(path, state);
            }
        }
    }

    /// Writes what `remember()` writes, when it moved.
    fn remember(&mut self) {
        let now = self.nav.model.remembered();
        if self.nav.remembered.as_ref() == Some(&now) {
            return;
        }
        let (base, favourites_only, unfolded) = now.clone();
        let _ = self.preferences.set(BASE, base.map(Value::String).unwrap_or(Value::Null));
        let _ = self.preferences.set(FAVOURITES_ONLY, Value::Bool(favourites_only));
        let _ = self.preferences.set(UNFOLDED, Value::Array(unfolded.into_iter().map(Value::String).collect()));
        self.nav.remembered = Some(now);
    }

    /// Merged with what another app wrote since, then each favourite's volume noted beside it.
    fn save_favourites(&mut self) -> Option<String> {
        let nav = &mut self.nav;
        let saved = nav.favourites.save_merged(&mut nav.favourites_read, &self.support);
        if saved.is_ok() {
            oni_library::bookmarks::record(self.support.clone(), self.nav.favourites.paths.clone(), self.nav.identify.clone());
        }
        saved.err().map(|e| tr!("The favourites cannot be saved: {error}", error = e))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_trimmed_of_spaces_and_tabs_only() {
        assert_eq!(trim_spaces(" \tRoll 12\u{3000} "), "Roll 12");
        // Twin: a line's end is not a space, so it stays and the rename refuses the name.
        assert_eq!(trim_spaces("Roll\n"), "Roll\n");
    }
}
