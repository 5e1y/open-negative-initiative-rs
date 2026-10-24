//! The navigator in the window (`FolderNavigator`, `RootWelcome`, `RootsCatchUp`): folders listed off
//! this thread, the favourites read behind the opened folder, the folder menu, what a launch finds.

use super::{Ctl, State, with_shell};
use crate::{Advice, App, CatchUpLine, FolderChoice, FolderLine, MenuLine, WelcomeLine};
use oni_i18n::tr;
use oni_library::favourites::{self, Area, Favourites, Surveys};
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
mod reading;
mod catch_up;

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

#[derive(Clone, Copy)]
enum Act {
    Open,
    Favourite,
    Rename,
    Roll(bool),
    NewFolder,
    Reveal,
}

pub(super) struct Nav {
    pub computer: Computer,
    pub model: Navigation,
    pub favourites: Favourites,
    remembered: Option<(Option<String>, bool, Vec<String>)>,
    /// The last click: its row, when, and the count it reached, as AppKit's `clickCount` keeps it.
    clicks: Option<(String, Instant, usize)>,
    /// The rows as published, each one's path.
    rows: Vec<String>,
    menu: Vec<Act>,
    menu_path: Option<String>,
    /// The open photo's folder last revealed beside it.
    current: Option<String>,
    /// `RootWelcome`'s folders, and the former library's photos `RootsCatchUp` counts.
    welcome: Vec<String>,
    former: Vec<String>,
    pub catching_up: bool,
    catch_up_folders: Vec<String>,
    surveys: Surveys,
    /// The favourites this pass has read or tried, so one the disk refuses is not read again in it.
    tried: HashSet<String>,
    reading: bool,
    /// Held off while the opened folder's own read runs, which goes first.
    waiting: bool,
    opened_rolls: HashSet<String>,
    area_rolls: HashSet<String>,
    /// Make a Roll and Back to a Folder until the folder is read again.
    roll_edits: HashMap<String, bool>,
    /// Held by the mounts' reader, which ends once the window's state is gone.
    watcher: Arc<()>,
    /// The folders the disk refused, the opened one among them, which the mounts' reader tries.
    refused: Arc<std::sync::Mutex<Vec<String>>>,
    /// The rows' folders a listing or a favourite's read found offline or missing, looked at off this thread.
    presence: HashMap<String, Presence>,
}

impl Nav {
    pub fn new(computer: Computer) -> Nav {
        Nav {
            computer,
            model: Navigation::default(),
            favourites: Favourites::default(),
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
            surveys: Surveys::default(),
            tried: HashSet::new(),
            reading: false,
            waiting: false,
            opened_rolls: HashSet::new(),
            area_rolls: HashSet::new(),
            roll_edits: HashMap::new(),
            watcher: Arc::new(()),
            refused: Arc::default(),
            presence: HashMap::new(),
        }
    }

    /// `rollFolders`: the opened folder's rolls and the favourites', with the hand's edits on top.
    pub(super) fn rolls(&self) -> HashSet<String> {
        let mut out: HashSet<String> = self.opened_rolls.union(&self.area_rolls).cloned().collect();
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

    fn wanted_folders(&self) -> Vec<String> {
        let mut wanted: Vec<String> = self.opened.iter().cloned().collect();
        wanted.extend(self.nav.favourites.paths.iter().cloned());
        wanted
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

    fn save_favourites(&mut self) -> Option<String> {
        self.nav.favourites.save(&self.support).err().map(|e| tr!("The favourites cannot be saved: {error}", error = e))
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
