//! The navigator in the window (`FolderNavigator`, `RootWelcome`, `RootsCatchUp`): folders listed off
//! this thread, the favourites read behind the opened folder, the folder menu, what a launch finds.

use super::{Ctl, State, with_shell};
use crate::{Advice, App, CatchUpLine, FolderChoice, FolderLine, MenuLine, WelcomeLine};
use oni_library::favourites::{self, Area, Favourites, Surveys};
use oni_library::gallery_history::Move;
use oni_library::navigator::{self as walk, Computer, Landed, Navigation, TRASH_ROW};
use oni_library::paths::{self, holds};
use oni_library::roots::{self, Roots};
use oni_library::{archive_tree::NodeId, folders};
use serde_json::Value;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
        }
    }

    /// `rollFolders`: the opened folder's rolls and the favourites', with the hand's edits on top.
    fn rolls(&self) -> HashSet<String> {
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
    let _ = (a, b);
    false
}

impl State {
    fn present(&self, path: &str) -> bool {
        let c = &self.nav.computer;
        !c.is_on_volume(path) || self.nav.model.volumes.iter().any(|v| holds(v, path))
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
        self.nav.favourites.save(&self.support).err().map(|e| format!("The favourites cannot be saved: {e}"))
    }
}

impl Ctl {
    pub(super) fn wire_navigator(me: &Rc<Ctl>, window: &crate::MainWindow) {
        me.load_navigator();
        let n = window.global::<crate::Navigator>();
        let index = |i: i32| i.max(0) as usize;
        let p = me.clone();
        n.on_row_clicked(move |i| p.navigator_row_clicked(index(i)));
        let p = me.clone();
        n.on_star_clicked(move |i| {
            let path = p.state.borrow().nav.rows.get(index(i)).cloned();
            if let Some(path) = path {
                p.toggle_favourite(&path);
            }
        });
        let p = me.clone();
        n.on_row_menu(move |i| p.navigator_row_menu(index(i)));
        let p = me.clone();
        n.on_menu_chosen(move |i| p.navigator_menu_chosen(index(i)));
        let p = me.clone();
        n.on_rename_committed(move |i, draft| p.rename_committed(index(i), &draft));
        let p = me.clone();
        n.on_rename_cancelled(move || {
            p.state.borrow_mut().nav.model.renaming = None;
            p.publish_navigator();
        });
        let p = me.clone();
        n.on_go_up(move || {
            {
                let mut s = p.state.borrow_mut();
                let computer = s.nav.computer.clone();
                s.nav.model.go_up(&computer);
            }
            p.publish_navigator();
        });
        let p = me.clone();
        n.on_favourites_toggled(move || {
            {
                let mut s = p.state.borrow_mut();
                let on = !s.nav.model.favourites_only;
                s.nav.model.show_favourites(on);
            }
            p.publish_navigator();
        });
        let p = me.clone();
        n.on_ancestor_chosen(move |i| p.ancestor_chosen(index(i)));
        let p = me.clone();
        n.on_height_stored(move |h| {
            let _ = p.state.borrow_mut().preferences.set(EDITOR_HEIGHT, Value::from(h as f64));
        });
        let p = me.clone();
        n.on_mode_changed(move || p.follow_open_photo());
        let p = me.clone();
        n.on_welcome_added(move |i| {
            let folder = p.state.borrow().nav.welcome.get(index(i)).cloned();
            if let Some(folder) = folder {
                p.add(Path::new(&folder));
            }
        });
        let p = me.clone();
        n.on_welcome_chose(move || p.act("open"));
        let p = me.clone();
        n.on_catch_up_chose(move || p.act("open"));
        let p = me.clone();
        n.on_catch_up_added(move |i| {
            let folder = p.state.borrow().nav.catch_up_folders.get(index(i)).cloned();
            if let Some(folder) = folder {
                p.toggle_favourite(&folder);
                p.open_folder(Path::new(&folder));
            }
        });
        let p = me.clone();
        n.on_catch_up_done(move || p.catch_up_done());
        Ctl::watch_volumes(me);
        me.start_pass();
    }

    /// `FolderNavigation.init` and the launch's favourites: the remembered base, view, rows and
    /// height; `favourites.json`, seeded once from `roots.json`; a former library's folders.
    fn load_navigator(&self) {
        let (height, catching_up, failure) = {
            let mut s = self.state.borrow_mut();
            let prefs = &s.preferences;
            let base = prefs.string(BASE).map(str::to_owned);
            let favourites_only = prefs.bool(FAVOURITES_ONLY).unwrap_or(false);
            let unfolded: BTreeSet<String> = prefs
                .value(UNFOLDED)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                .unwrap_or_default();
            let height = prefs.value(EDITOR_HEIGHT).and_then(Value::as_f64).unwrap_or(EDITOR_REST);
            let remembered_opened = prefs.string(crate::preferences::OPENED_FOLDER).map(str::to_owned);
            let caught_up = prefs.bool(ROOTS_CATCH_UP_SHOWN).unwrap_or(false);
            let volumes = s.nav.computer.computer();
            s.nav.model = Navigation::new(base, favourites_only, unfolded, volumes);
            s.nav.remembered = Some(s.nav.model.remembered());
            s.org.trash_unfolded = s.nav.model.unfolded.contains(TRASH_ROW);
            let roots = Roots::load(&s.support);
            s.nav.favourites = Favourites::load(&s.support).unwrap_or_else(|| Favourites::seeded(&roots));
            let failure = s.save_favourites();
            let former = oni_library::library::load(&s.support).files;
            let opened = remembered_opened.or_else(|| roots.current().map(str::to_owned));
            if s.nav.favourites.paths.is_empty() && opened.is_none() {
                s.nav.welcome = roots::suggested(&former, &s.nav.computer.home);
            }
            s.nav.catching_up = !former.is_empty() && !caught_up;
            s.nav.former = former;
            (height, s.nav.catching_up, failure)
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        if let Some(w) = self.window() {
            let n = w.global::<crate::Navigator>();
            n.set_height(height as f32);
            n.set_catch_up_shown(catching_up);
        }
        self.publish_navigator();
    }

    /// Reads the mounted volumes again whenever the names under the volumes' folder change.
    fn watch_volumes(me: &Rc<Ctl>) {
        let (token, computer) = {
            let s = me.state.borrow();
            (s.nav.watcher.clone(), s.nav.computer.clone())
        };
        let names = |folder: &str| -> BTreeSet<String> {
            std::fs::read_dir(folder)
                .map(|l| l.filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned())).collect())
                .unwrap_or_default()
        };
        let _ = std::thread::Builder::new().name("volumes".into()).spawn(move || {
            let mut seen = names(&computer.volumes);
            loop {
                std::thread::sleep(MOUNTS);
                if Arc::strong_count(&token) <= 1 {
                    return;
                }
                let now = names(&computer.volumes);
                if now != seen {
                    seen = now;
                    let volumes = computer.computer();
                    with_shell(move |c| {
                        c.state.borrow_mut().nav.model.volumes = volumes;
                        c.publish_navigator();
                    });
                }
            }
        });
    }

    /// The rows, the header and the welcome as they stand; every row's own listing is asked so
    /// its chevron knows whether it holds a folder.
    pub(super) fn publish_navigator(&self) {
        let Some(w) = self.window() else { return };
        let published = {
            let mut s = self.state.borrow_mut();
            let favourites = s.nav.favourites.paths.clone();
            let rows = s.nav.model.rows(&favourites);
            for (path, _) in &rows {
                s.nav.model.list(path, false);
            }
            let rolls = s.nav.rolls();
            let opened = if s.org.trash_open { None } else { s.opened.clone() };
            let model = &s.nav.model;
            let lines: Vec<FolderLine> = rows
                .iter()
                .map(|(path, depth)| FolderLine {
                    title: paths::name(path).into(),
                    path: path.clone().into(),
                    depth: *depth as i32,
                    chevron: model.listings.get(path).is_none_or(|c| !c.is_empty()),
                    unfolded: model.unfolded.contains(path),
                    kind: if rolls.contains(path) {
                        1
                    } else if opened.as_deref() == Some(path.as_str()) {
                        2
                    } else {
                        0
                    },
                    picked: model.selected.as_deref() == Some(path.as_str()),
                    opened: opened.as_deref() == Some(path.as_str()),
                    favourite: favourites.contains(path),
                    present: s.present(path),
                    renaming: model.renaming.as_deref() == Some(path.as_str()),
                })
                .collect();
            let base = model.base.clone();
            let mut ancestors: Vec<Option<String>> =
                base.as_deref().map(|b| s.nav.computer.ancestors(b)).unwrap_or_default().into_iter().map(Some).collect();
            ancestors.push(None);
            let choices: Vec<FolderChoice> =
                ancestors.iter().map(|a| FolderChoice { title: walk::title(a.as_deref()).into(), current: *a == base }).collect();
            let header = (
                model.can_go_up(),
                model.favourites_only,
                if model.favourites_only { "Favourites".to_string() } else { walk::title(base.as_deref()) },
                base.as_deref().map(|b| format!("{b} — click to jump to a folder above")).unwrap_or_else(|| "The home folder and every volume".into()),
                model.favourites_only && favourites.is_empty(),
                !model.favourites_only && base.is_none(),
            );
            let welcome: Vec<WelcomeLine> = s
                .nav
                .welcome
                .iter()
                .map(|p| WelcomeLine { title: format!("Add {}", paths::name(p)).to_uppercase().into(), path: p.clone().into() })
                .collect();
            let welcome_shown = s.opened.is_none() && !s.nav.welcome.is_empty();
            let opened_pill = (s.opened.as_deref().map(paths::name).unwrap_or_default(), s.opened.clone().unwrap_or_default());
            s.nav.rows = rows.into_iter().map(|(p, _)| p).collect();
            s.remember();
            (lines, choices, header, welcome, welcome_shown, opened_pill)
        };
        self.list_wanted();
        let (lines, choices, header, welcome, welcome_shown, opened_pill) = published;
        let n = w.global::<crate::Navigator>();
        n.set_rows(ModelRc::new(VecModel::from(lines)));
        n.set_ancestors(ModelRc::new(VecModel::from(choices)));
        n.set_can_go_up(header.0);
        n.set_favourites_only(header.1);
        n.set_pill_title(header.2.into());
        n.set_pill_help(header.3.into());
        n.set_no_favourites(header.4);
        n.set_at_computer(header.5);
        n.set_welcome(ModelRc::new(VecModel::from(welcome)));
        n.set_welcome_shown(welcome_shown);
        n.set_opened_title(opened_pill.0.into());
        n.set_opened_help(opened_pill.1.into());
        self.publish_catch_up();
    }

    /// The listings asked and not under way, read on a few threads; each thread lands its share at once.
    fn list_wanted(&self) {
        let wanted = self.state.borrow_mut().nav.model.take_wanted();
        if wanted.is_empty() {
            return;
        }
        let share = wanted.len().div_ceil(LISTERS);
        for chunk in wanted.chunks(share) {
            let chunk = chunk.to_vec();
            let _ = std::thread::Builder::new().name("navigator".into()).spawn(move || {
                let listed: Vec<(String, Vec<String>)> = chunk.into_iter().map(|f| {
                    let children = walk::children(&f);
                    (f, children)
                }).collect();
                with_shell(move |c| c.listed(listed));
            });
        }
    }

    fn listed(self: &Rc<Self>, listed: Vec<(String, Vec<String>)>) {
        let opening: Vec<String> = {
            let mut s = self.state.borrow_mut();
            listed
                .into_iter()
                .filter_map(|(folder, children)| match s.nav.model.listed(&folder, children) {
                    Landed::Open(folder) => Some(folder),
                    _ => None,
                })
                .collect()
        };
        self.publish_navigator();
        for folder in opening {
            self.open_folder(Path::new(&folder));
        }
    }

    /// `NavigatorRow.onTapGesture`: the click's count read as AppKit counts it, then `click`.
    fn navigator_row_clicked(self: &Rc<Self>, i: usize) {
        let open = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return };
            let now = Instant::now();
            let count = match &s.nav.clicks {
                Some((last, at, n)) if *last == path && now.duration_since(*at) <= crate::system::double_click() => n + 1,
                _ => 1,
            };
            s.nav.clicks = Some((path.clone(), now, count));
            let roll = s.nav.rolls().contains(&path);
            s.org.trash_picked = None;
            s.nav.model.click(&path, count, roll)
        };
        self.publish_trash_rows();
        self.publish_navigator();
        if let Some(folder) = open {
            self.open_folder(Path::new(&folder));
        }
    }

    fn ancestor_chosen(&self, i: usize) {
        {
            let mut s = self.state.borrow_mut();
            let base = s.nav.model.base.clone();
            let mut ancestors: Vec<Option<String>> =
                base.as_deref().map(|b| s.nav.computer.ancestors(b)).unwrap_or_default().into_iter().map(Some).collect();
            ancestors.push(None);
            let Some(chosen) = ancestors.get(i).cloned() else { return };
            s.nav.model.go(chosen);
        }
        self.publish_navigator();
    }

    /// `NavigatorRow.contextMenu`: Open, the favourite, Rename…, the roll, New Folder Inside, Show in Finder.
    fn navigator_row_menu(&self, i: usize) -> bool {
        let Some(w) = self.window() else { return false };
        let lines = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return false };
            let present = s.present(&path);
            let favourite = s.nav.favourites.contains(&path);
            let roll = s.nav.rolls().contains(&path);
            let items = [
                (line("Open", present, false), Act::Open),
                (line(if favourite { "Remove from Favourites" } else { "Add to Favourites" }, true, false), Act::Favourite),
                (line("Rename…", present, true), Act::Rename),
                (line(if roll { "Back to a Folder" } else { "Make a Roll" }, present, false), Act::Roll(!roll)),
                (line("New Folder Inside", present, false), Act::NewFolder),
                (line("Show in Finder", true, true), Act::Reveal),
            ];
            s.nav.menu = items.iter().map(|(_, a)| *a).collect();
            s.nav.menu_path = Some(path);
            items.into_iter().map(|(l, _)| l).collect::<Vec<_>>()
        };
        w.global::<crate::Navigator>().set_menu_lines(ModelRc::new(VecModel::from(lines)));
        true
    }

    fn navigator_menu_chosen(self: &Rc<Self>, i: usize) {
        let (act, path) = {
            let s = self.state.borrow();
            let (Some(act), Some(path)) = (s.nav.menu.get(i).copied(), s.nav.menu_path.clone()) else { return };
            (act, path)
        };
        match act {
            Act::Open => {
                self.state.borrow_mut().nav.model.selected = Some(path.clone());
                self.open_folder(Path::new(&path));
            }
            Act::Favourite => self.toggle_favourite(&path),
            Act::Rename => {
                self.state.borrow_mut().nav.model.renaming = Some(path);
                self.publish_navigator();
            }
            Act::Roll(make) => self.make_roll(&path, make),
            Act::NewFolder => self.new_folder(&path),
            Act::Reveal => crate::system::reveal(Path::new(&path)),
        }
    }

    /// `toggleFavourite`: kept in `favourites.json`, the read of a folder no longer wanted stopped,
    /// a new one read in turn.
    pub(super) fn toggle_favourite(&self, path: &str) {
        let failure = {
            let mut s = self.state.borrow_mut();
            s.nav.favourites.toggle(path);
            let failure = s.save_favourites();
            let wanted = s.wanted_folders();
            s.nav.surveys.yield_keeping(&wanted);
            failure
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        self.start_pass();
        self.publish_navigator();
    }

    /// `FolderNavigator.commit`: an empty or unchanged name puts the row back as it was.
    fn rename_committed(self: &Rc<Self>, i: usize, draft: &str) {
        let path = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return };
            if s.nav.model.renaming.as_deref() != Some(path.as_str()) {
                return;
            }
            s.nav.model.renaming = None;
            path
        };
        let name = trim_spaces(draft);
        if name.is_empty() || name == paths::name(&path) {
            return self.publish_navigator();
        }
        self.rename_folder(&path, name);
    }

    /// `renameFolder`: the folder renamed on the disk; the navigator, the favourites, the opened
    /// folder, a roll's name and the index rows follow it, then the folder is read again.
    pub(super) fn rename_folder(self: &Rc<Self>, path: &str, name: &str) {
        let trimmed = trim_spaces(name);
        if trimmed.is_empty() || trimmed.contains('/') || trimmed == paths::name(path) {
            return self.publish_navigator();
        }
        let target = paths::join(&paths::parent(path), trimmed);
        // `moveItem` refuses an existing target; `rename(2)` would replace an empty folder.
        let moved = if Path::new(&target).exists() && !same_item(path, &target) {
            Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("an item named “{trimmed}” already exists")))
        } else {
            std::fs::rename(path, &target)
        };
        if let Err(e) = moved {
            self.went_wrong(&format!("The folder could not be renamed: {e}"));
            return self.publish_navigator();
        }
        let failure = {
            let mut s = self.state.borrow_mut();
            s.nav.model.follow(path, &target);
            s.nav.favourites.follow(path, &target);
            let failure = s.save_favourites();
            if let Some(opened) = s.opened.clone().filter(|o| holds(path, o)) {
                let moved = format!("{target}{}", &opened[path.len()..]);
                let _ = s.preferences.set(crate::preferences::OPENED_FOLDER, Value::String(moved.clone()));
                s.opened = Some(moved);
            }
            failure
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        if let Some(mut film) = folders::roll(&target) {
            film.name = trimmed.to_owned();
            let _ = folders::set_roll(Some(&film), &target);
        }
        self.followed(&[Move { from: path.to_owned(), to: target }]);
        self.start_pass();
        self.publish_navigator();
    }

    /// `newFolder(in:)` then `made`: "New Folder", numbered as the Finder numbers one taken; its
    /// row is picked and named at once.
    pub(super) fn new_folder(self: &Rc<Self>, folder: &str) {
        let mut name = "New Folder".to_string();
        let mut rank = 2;
        while Path::new(&paths::join(folder, &name)).exists() {
            name = format!("New Folder {rank}");
            rank += 1;
        }
        let path = paths::join(folder, &name);
        if std::fs::create_dir(&path).is_err() {
            return;
        }
        let made = paths::canonical(Path::new(&path));
        let inside = {
            let mut s = self.state.borrow_mut();
            let inside = s.opened.as_deref().is_some_and(|o| holds(o, &made));
            if inside {
                s.survey.folders.push(made.clone());
                if let Some(archive) = s.archive.as_mut() {
                    archive.expanded.insert(NodeId::Folder(folder.to_owned()));
                }
            }
            s.nav.model.made(&made);
            inside
        };
        if inside {
            self.lay_archive();
        }
        self.publish_navigator();
    }

    /// `makeRoll` and `unmakeRoll`: the folder's roll marker written or taken away.
    pub(super) fn make_roll(self: &Rc<Self>, folder: &str, make: bool) {
        if make {
            let film = oni_settings::Film::new(&paths::name(folder));
            if folders::set_roll(Some(&film), folder).is_err() {
                return self.went_wrong("The folder could not be made a roll: it cannot be written to.");
            }
        } else if folders::set_roll(None, folder).is_err() {
            return;
        }
        self.state.borrow_mut().nav.roll_edits.insert(folder.to_owned(), make);
        self.lay_archive();
        self.publish_navigator();
    }

    /// `reportFailure(as: "Something Went Wrong")`.
    fn went_wrong(&self, message: &str) {
        let Some(w) = self.window() else { return };
        let a = w.global::<Advice>();
        a.set_title("Something Went Wrong".into());
        a.set_message(message.into());
        a.set_shown(true);
    }

    /// `add(_:)`'s `folderNavigation.reveal`: a folder opened from elsewhere is picked in the column.
    pub(super) fn navigator_reveal(&self, folder: &Path) {
        {
            let mut s = self.state.borrow_mut();
            let computer = s.nav.computer.clone();
            s.nav.model.reveal(&paths::canonical(folder), &computer);
        }
        self.publish_navigator();
    }

    /// `openFolder`'s `surveys.yield`: a favourite's read gives way to the folder opened.
    pub(super) fn navigator_opened(&self, folder: &str) {
        {
            let mut s = self.state.borrow_mut();
            s.nav.surveys.yield_keeping(&[folder.to_owned()]);
            s.nav.waiting = true;
        }
        self.publish_navigator();
    }

    /// The opened folder read whole: its rolls known, then the favourites read in turn.
    pub(super) fn navigator_read(&self) {
        {
            let mut s = self.state.borrow_mut();
            let rolls: HashSet<String> = folders::rolls(&s.survey.images).into_keys().collect();
            if let Some(opened) = s.opened.clone() {
                s.nav.roll_edits.retain(|k, _| !holds(&opened, k));
            }
            s.nav.opened_rolls = rolls;
            s.nav.waiting = false;
        }
        self.start_pass();
        self.publish_navigator();
    }

    fn start_pass(&self) {
        self.state.borrow_mut().nav.tried.clear();
        self.survey_favourites();
    }

    /// `surveyFolders` for the favourites: each one no read area covers, read alone off this
    /// thread, the folders holding the opened one first.
    fn survey_favourites(&self) {
        let (root, stop) = {
            let mut s = self.state.borrow_mut();
            let wanted = s.wanted_folders();
            s.nav.surveys.keep(&wanted);
            if s.nav.reading || s.nav.waiting {
                return;
            }
            let opened = s.opened.clone();
            let mut stored: Vec<String> = s.nav.surveys.areas.keys().cloned().collect();
            stored.extend(opened.iter().cloned());
            stored.extend(s.nav.tried.iter().cloned());
            let missing = favourites::missing(&s.nav.favourites.paths, &stored);
            let next = favourites::ordered(&missing, opened.as_deref()).into_iter().find(|r| Some(r) != opened.as_ref());
            let Some(root) = next else { return };
            s.nav.tried.insert(root.clone());
            s.nav.reading = true;
            let stop = s.nav.surveys.begin(&root);
            (root, stop)
        };
        let _ = std::thread::Builder::new().name("favourites".into()).spawn(move || {
            let area = favourites::read(&root, &stop);
            with_shell(move |c| c.favourite_read(root, area));
        });
    }

    fn favourite_read(&self, root: String, area: Option<Area>) {
        {
            let mut s = self.state.borrow_mut();
            s.nav.surveys.end();
            s.nav.reading = false;
            let wanted = s.wanted_folders();
            if let Some(area) = area.filter(|_| wanted.iter().any(|w| holds(&root, w) || holds(w, &root))) {
                s.nav.roll_edits.retain(|k, _| !holds(&root, k));
                s.nav.surveys.store(area, &root);
            }
            let inside = |p: &str| wanted.iter().any(|w| holds(w, p));
            let rolls: HashSet<String> =
                s.nav.surveys.areas.values().flat_map(|a| a.rolls.keys()).filter(|k| inside(k)).cloned().collect();
            s.nav.area_rolls = rolls;
        }
        self.publish_navigator();
        self.survey_favourites();
    }

    /// `reveal(current)`: beside a photo, the navigator unfolds down to the photo's folder and
    /// scrolls it to the centre.
    pub(super) fn follow_open_photo(&self) {
        let Some(w) = self.window() else { return };
        let gallery = w.global::<App>().get_gallery();
        {
            let mut s = self.state.borrow_mut();
            let current = if gallery { None } else { s.open.as_deref().map(paths::parent) };
            if current == s.nav.current {
                return;
            }
            s.nav.current = current.clone();
            let Some(folder) = current else { return };
            let favourites = s.nav.favourites.paths.clone();
            let computer = s.nav.computer.clone();
            s.nav.model.show(&folder, &favourites, &computer);
        }
        self.publish_navigator();
        let row = {
            let s = self.state.borrow();
            s.nav.current.as_ref().and_then(|c| s.nav.rows.iter().position(|r| r == c))
        };
        if let Some(row) = row {
            let n = w.global::<crate::Navigator>();
            n.set_reveal_row(row as i32);
            n.set_reveal_serial(n.get_reveal_serial() + 1);
        }
    }

    /// The ONI Trash's fold and pick, held by the navigator as Swift's `unfolded` and `selected`.
    pub(super) fn trash_synced(&self) {
        let moved = {
            let mut s = self.state.borrow_mut();
            let unfolded = s.org.trash_unfolded;
            if unfolded != s.nav.model.unfolded.contains(TRASH_ROW) {
                if unfolded {
                    s.nav.model.unfolded.insert(TRASH_ROW.to_owned());
                } else {
                    s.nav.model.unfolded.remove(TRASH_ROW);
                }
            }
            let picked = s.org.trash_picked.clone().map(|p| format!("{TRASH_ROW}{p}"));
            let moved = picked.is_some() && picked != s.nav.model.selected;
            if moved {
                s.nav.model.selected = picked;
            }
            s.remember();
            moved
        };
        if moved {
            self.publish_navigator();
        }
    }

    /// `offerNeutralDecodeNotice`'s guard: no notice under the catch-up sheet.
    pub(super) fn catching_up(&self) -> bool {
        self.state.borrow().nav.catching_up
    }

    /// `RootsCatchUp.outside`: the former photos no favourite holds, by folder, most first.
    fn publish_catch_up(&self) {
        let Some(w) = self.window() else { return };
        let (choose, left, lines, more, all_in) = {
            let mut s = self.state.borrow_mut();
            if !s.nav.catching_up {
                return;
            }
            let roots = s.nav.favourites.paths.clone();
            let mut counts: HashMap<String, usize> = HashMap::new();
            for path in s.nav.former.iter().filter(|p| !roots.iter().any(|r| holds(r, p))) {
                *counts.entry(paths::parent(path)).or_default() += 1;
            }
            let mut outside: Vec<(String, usize)> = counts.into_iter().collect();
            outside.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            let total: usize = outside.iter().map(|(_, n)| n).sum();
            let choose = if roots.is_empty() { "Add Folder…" } else { "Add Another Folder…" }.to_uppercase();
            let left = if outside.is_empty() { String::new() } else { format!("{total} photos of your library are not in it yet:") };
            let more = if outside.len() > CATCH_UP_SHOWN { format!("and {} more folders", outside.len() - CATCH_UP_SHOWN) } else { String::new() };
            let all_in = if outside.is_empty() && !roots.is_empty() { "Every photo of your library is in." } else { "" };
            outside.truncate(CATCH_UP_SHOWN);
            s.nav.catch_up_folders = outside.iter().map(|(f, _)| f.clone()).collect();
            let lines: Vec<CatchUpLine> = outside.into_iter().map(|(f, n)| CatchUpLine { folder: f.into(), count: n.to_string().into() }).collect();
            (choose, left, lines, more, all_in.to_string())
        };
        let n = w.global::<crate::Navigator>();
        n.set_catch_up_choose(choose.into());
        n.set_catch_up_left(left.into());
        n.set_catch_up_lines(ModelRc::new(VecModel::from(lines)));
        n.set_catch_up_more(more.into());
        n.set_catch_up_all_in(all_in.into());
    }

    /// `rootsCatchUpDismissed`: never shown again, and the recalibration notice may follow.
    fn catch_up_done(&self) {
        {
            let mut s = self.state.borrow_mut();
            s.nav.catching_up = false;
            let _ = s.preferences.set(ROOTS_CATCH_UP_SHOWN, Value::Bool(true));
        }
        if let Some(w) = self.window() {
            w.global::<crate::Navigator>().set_catch_up_shown(false);
        }
        let order = self.state.borrow().order.clone();
        self.offer_neutral_decode_notice(&order);
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
