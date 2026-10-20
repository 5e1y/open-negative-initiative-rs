//! The gallery's selection arithmetic, `ListPicking` and the parts of `GalleryEngine` and
//! `GalleryView` that decide what a click, a key or ⌘A picks, on paths and in the order shown.

use oni_settings::Medal;
use std::collections::HashSet;
use std::hash::Hash;

/// ⌘: the item in or out, the rest kept.
pub fn toggled(id: &str, selection: &HashSet<String>) -> HashSet<String> {
    let mut next = selection.clone();
    if !next.remove(id) {
        next.insert(id.to_owned());
    }
    next
}

/// ⇧: the items between the anchor and the clicked position in the order shown, added to what is
/// selected. The position, not the item, decides; `None` without the anchor shown.
pub fn extended(clicked: usize, anchor: Option<&str>, order: &[String], selection: &HashSet<String>) -> Option<HashSet<String>> {
    let anchor = anchor?;
    if clicked >= order.len() {
        return None;
    }
    let from = order.iter().position(|p| p == anchor)?;
    let mut next = selection.clone();
    next.extend(order[from.min(clicked)..=from.max(clicked)].iter().cloned());
    Some(next)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    Select,
    Open,
}

/// `ClickTracker`: the first click of a pair selects at once, the second on the same item within
/// the interval opens, and a third starts a fresh pair.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClickTracker {
    last: Option<(String, f64)>,
}

impl ClickTracker {
    pub fn register(&mut self, id: &str, time: f64, interval: f64) -> Click {
        if let Some((last, at)) = &self.last
            && last == id
            && time - at <= interval
        {
            self.last = None;
            return Click::Open;
        }
        self.last = Some((id.to_owned(), time));
        Click::Select
    }
}

/// ← and → walk the displayed order, ↑ and ↓ walk rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Cells(i32),
    Rows(i32),
}

/// `GalleryView.destination`: rows group by band, so a flat jump of `columns` never crosses a
/// boundary unseen, and a short row lands on its last cell.
pub fn destination(index: usize, step: Step, blocks: &[usize], columns: usize) -> usize {
    let blocks: Vec<usize> = blocks.iter().copied().filter(|b| *b > 0).collect();
    let total: usize = blocks.iter().sum();
    if total == 0 {
        return 0;
    }
    let here = index.min(total - 1) as i64;
    let width = columns.max(1) as i64;
    match step {
        Step::Cells(delta) => (here + delta as i64).clamp(0, total as i64 - 1) as usize,
        Step::Rows(0) => here as usize,
        Step::Rows(delta) => {
            let mut block = 0;
            let mut start = 0i64;
            while block < blocks.len() - 1 && here >= start + blocks[block] as i64 {
                start += blocks[block] as i64;
                block += 1;
            }
            let column = (here - start) % width;
            let rows = |b: usize| (blocks[b] as i64 + width - 1) / width;
            let mut row = (here - start) / width + delta as i64;
            if row < 0 {
                if block == 0 {
                    return here as usize;
                }
                block -= 1;
                start -= blocks[block] as i64;
                row = rows(block) - 1;
            } else if row >= rows(block) {
                if block + 1 >= blocks.len() {
                    return here as usize;
                }
                start += blocks[block] as i64;
                block += 1;
                row = 0;
            }
            let first = start + row * width;
            let last = (first + width).min(start + blocks[block] as i64) - 1;
            (first + column).min(last) as usize
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceKey {
    Ignore,
    Swallow,
    Open,
}

/// Space opens with no modifier among ⌘ ⌥ ⌃ ⇧, and a repeat is swallowed.
pub fn space_key(repeat: bool, modified: bool) -> SpaceKey {
    if modified {
        SpaceKey::Ignore
    } else if repeat {
        SpaceKey::Swallow
    } else {
        SpaceKey::Open
    }
}

/// The cursor, else the first selected photo shown, else the first shown.
pub fn space_target(cursor: Option<&str>, selection: &HashSet<String>, shown: &[String]) -> Option<String> {
    if let Some(c) = cursor {
        return Some(c.to_owned());
    }
    shown.iter().find(|p| selection.contains(*p)).or_else(|| shown.first()).cloned()
}

/// `GalleryEngine.completedGroups`: the groups holding the selection, else the focused one if it
/// is shown holding something, else the last group.
pub fn completed_groups<G: Clone + Eq + Hash>(groups: &[(G, Vec<String>)], selection: &HashSet<String>, focus: Option<&G>) -> HashSet<G> {
    let non_empty: Vec<&(G, Vec<String>)> = groups.iter().filter(|(_, e)| !e.is_empty()).collect();
    let mut ids: HashSet<G> = non_empty.iter().filter(|(_, e)| e.iter().any(|p| selection.contains(p))).map(|(g, _)| g.clone()).collect();
    if ids.is_empty()
        && let Some(f) = focus.filter(|f| non_empty.iter().any(|(g, _)| g == *f))
    {
        ids.insert(f.clone());
    }
    if ids.is_empty()
        && let Some((last, _)) = non_empty.last()
    {
        ids.insert(last.clone());
    }
    ids
}

/// ⌘A: the selection completed with every photo of the groups it touches.
pub fn select_all<G: Clone + Eq + Hash>(groups: &[(G, Vec<String>)], selection: &HashSet<String>, focus: Option<&G>) -> HashSet<String> {
    let ids = completed_groups(groups, selection, focus);
    let mut next = selection.clone();
    next.extend(groups.iter().filter(|(g, _)| ids.contains(g)).flat_map(|(_, e)| e.iter().cloned()));
    next
}

/// `columnKeys`' ⌘A, on folder groups: the folders the selection touches, else the ringed folder,
/// else the open photo's; never the gallery's last group, which the editor does not show.
pub fn column_select_all(groups: &[(String, Vec<String>)], selection: &HashSet<String>, ring: Option<&str>, open: Option<&str>) -> HashSet<String> {
    let mut touched: HashSet<&str> = groups.iter().filter(|(_, e)| e.iter().any(|p| selection.contains(p))).map(|(f, _)| f.as_str()).collect();
    if touched.is_empty() && let Some(folder) = ring {
        touched.insert(folder);
    }
    let parent = open.map(oni_library::paths::parent);
    if touched.is_empty() && let Some(folder) = parent.as_deref() {
        touched.insert(folder);
    }
    let mut next = selection.clone();
    next.extend(groups.iter().filter(|(f, _)| touched.contains(f.as_str())).flat_map(|(_, e)| e.iter().cloned()));
    next
}

/// `GalleryView.named`: a blank or unchanged name is refused rather than stored.
pub fn named(draft: &str, was: &str) -> Option<String> {
    let wanted = draft.trim();
    (!wanted.is_empty() && wanted != was).then(|| wanted.to_owned())
}

/// `CellMenu.label`: a verb with its count once it reaches more than one photo.
pub fn menu_label(verb: &str, count: usize) -> String {
    if count > 1 { format!("{verb} ({count} photos)") } else { verb.to_owned() }
}

/// `statusPath`: the one selected photo, else the open photo while editing with nothing selected.
pub fn status_path(selection: &HashSet<String>, editing: bool, current: Option<&str>) -> Option<String> {
    if selection.len() == 1 {
        return selection.iter().next().cloned();
    }
    if selection.is_empty() && editing { current.map(str::to_owned) } else { None }
}

/// `statusLabel`: the file's name, then its pixel size once probed.
pub fn status_label(path: &str, size: Option<(u32, u32)>) -> String {
    let name = oni_library::paths::name(path);
    match size {
        Some((w, h)) => format!("{name} \u{b7} {w} \u{d7} {h}"),
        None => name,
    }
}

fn photos(n: usize) -> String {
    format!("{n} photo{}", if n == 1 { "" } else { "s" })
}

/// `statusLeading`: the ONI Trash's count or selection while it is open, else the status photo,
/// else the photos shown or the count selected.
pub fn status_leading(trash: Option<(usize, usize)>, status: Option<String>, selected: usize, shown: usize) -> String {
    if let Some((picked, trashed)) = trash {
        return if picked > 0 { format!("{picked} selected") } else { format!("{} in the ONI Trash", photos(trashed)) };
    }
    if let Some(label) = status {
        return label;
    }
    if selected == 0 { photos(shown) } else { format!("{selected} selected") }
}

/// `statusMedalTarget`: the status photo's own medal, else the selection's, `None` for a mixed
/// one; no target at all in the gallery with nothing selected.
pub fn medal_target(status: Option<&str>, selection: &HashSet<String>, label: &dyn Fn(&str) -> Medal) -> Option<(Option<Medal>, Vec<String>)> {
    if let Some(path) = status {
        return Some((Some(label(path)), vec![path.to_owned()]));
    }
    if selection.is_empty() {
        return None;
    }
    let labels: HashSet<Medal> = selection.iter().map(|p| label(p)).collect();
    let mut paths: Vec<String> = selection.iter().cloned().collect();
    paths.sort();
    Some(((labels.len() == 1).then(|| *labels.iter().next().unwrap()), paths))
}

/// A tap on the picker: on one photo the medal it already wears comes off, on several it is laid.
pub fn medal_tapped(current: Option<Medal>, count: usize, tapped: Medal) -> Medal {
    if count == 1 && current == Some(tapped) { Medal::None } else { tapped }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn order(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    /// `ListPicking.selfCheck`.
    #[test]
    fn a_clicks_command_and_shift_pick_as_shown() {
        assert_eq!(toggled("b", &set(&["a"])), set(&["a", "b"]));
        assert_eq!(toggled("a", &set(&["a", "b"])), set(&["b"]));
        let shown = order(&["p1", "p5", "p3", "p9", "p5"]);
        assert_eq!(extended(3, Some("p5"), &shown, &set(&["x"])), Some(set(&["x", "p5", "p3", "p9"])));
        assert_eq!(extended(0, Some("p3"), &shown, &set(&[])), Some(set(&["p1", "p5", "p3"])), "backwards as well");
        assert_eq!(extended(1, Some("p7"), &shown, &set(&["p1"])), None, "an anchor not shown gives nothing");
        assert_eq!(extended(4, Some("p3"), &shown, &set(&[])), Some(set(&["p3", "p9", "p5"])));
        assert_eq!(extended(6, Some("p3"), &shown, &set(&[])), None, "past the end gives nothing");
        // Twin: the same range read in sorted order differs from the order shown.
        let mut sorted = shown.clone();
        sorted.sort();
        let (a, b) = (sorted.iter().position(|p| p == "p5").unwrap(), sorted.iter().position(|p| p == "p9").unwrap());
        let by_value: HashSet<String> = sorted[a..=b].iter().cloned().collect();
        assert_ne!(Some(by_value), extended(3, Some("p5"), &shown, &set(&[])));
    }

    /// `GalleryView.selfCheck`, its click half.
    #[test]
    fn the_second_click_within_the_interval_opens_and_a_third_starts_afresh() {
        let mut clicks = ClickTracker::default();
        assert_eq!(clicks.register("a", 100.0, 0.5), Click::Select);
        assert_eq!(clicks.register("a", 100.2, 0.5), Click::Open);
        assert_eq!(clicks.register("a", 100.3, 0.5), Click::Select);
        let mut slow = ClickTracker::default();
        slow.register("a", 0.0, 0.5);
        assert_eq!(slow.register("a", 0.6, 0.5), Click::Select, "spaced past the interval, two selections");
        let mut moving = ClickTracker::default();
        moving.register("a", 0.0, 0.5);
        // Twin: a fast click on another item does not open it.
        assert_eq!(moving.register("b", 0.01, 0.5), Click::Select);
    }

    /// `GalleryView.selfCheck`, its keyboard half: a row stops at its band's end.
    #[test]
    fn arrows_walk_cells_and_rows_and_a_row_never_crosses_a_band_unseen() {
        // Two bands of 8 and 3 on six columns: rows [0-5] [6-7] | [8-10].
        let blocks = [8, 3];
        assert_eq!(destination(0, Step::Cells(-1), &blocks, 6), 0);
        assert_eq!(destination(7, Step::Cells(1), &blocks, 6), 8, "cells walk the flat order");
        assert_eq!(destination(10, Step::Cells(1), &blocks, 6), 10);
        assert_eq!(destination(4, Step::Rows(1), &blocks, 6), 7, "a short row lands on its last cell");
        assert_eq!(destination(7, Step::Rows(1), &blocks, 6), 9, "into the next band at the same column");
        assert_eq!(destination(9, Step::Rows(-1), &blocks, 6), 7);
        assert_eq!(destination(2, Step::Rows(-1), &blocks, 6), 2, "the first row stops");
        assert_eq!(destination(9, Step::Rows(1), &blocks, 6), 9, "the last row stops");
        assert_eq!(destination(3, Step::Rows(1), &[0, 8, 0, 3], 6), 7, "empty bands are skipped");
        assert_eq!(destination(5, Step::Rows(1), &[], 6), 0);
        // Twin: a flat jump of six from 4 would land on 10, in the other band.
        assert_ne!(destination(4, Step::Rows(1), &blocks, 6), 4 + 6);
    }

    #[test]
    fn space_opens_bare_swallows_a_repeat_and_targets_the_cursor_then_the_selection() {
        assert_eq!(space_key(false, false), SpaceKey::Open);
        assert_eq!(space_key(true, false), SpaceKey::Swallow);
        assert_eq!(space_key(false, true), SpaceKey::Ignore);
        let shown = order(&["a", "b", "c"]);
        assert_eq!(space_target(Some("c"), &set(&["b"]), &shown).as_deref(), Some("c"));
        assert_eq!(space_target(None, &set(&["c", "b"]), &shown).as_deref(), Some("b"), "the first selected in the order shown");
        assert_eq!(space_target(None, &set(&[]), &shown).as_deref(), Some("a"));
        assert_eq!(space_target(None, &set(&[]), &[]), None);
    }

    /// `galleryEngineSelectAllSelfCheck`.
    #[test]
    fn command_a_completes_the_groups_the_selection_touches_else_the_focus_else_the_last() {
        let a = ("A", order(&["a1", "a2"]));
        let b = ("B", order(&["b1"]));
        let c = ("C", order(&[]));
        let rolls = [a.clone(), b.clone()];
        assert_eq!(select_all(&rolls, &set(&["a1"]), None), set(&["a1", "a2"]));
        assert_eq!(select_all(&rolls, &set(&["a1"]), Some(&"B")), set(&["a1", "a2"]), "a focus elsewhere does not override");
        assert_eq!(select_all(&rolls, &set(&[]), None), set(&["b1"]), "nothing selected, nothing focused: the last group");
        assert_eq!(select_all(&rolls, &set(&["a1", "b1"]), None), set(&["a1", "a2", "b1"]));
        assert_eq!(select_all(&rolls, &set(&[]), Some(&"A")), set(&["a1", "a2"]), "the focused group");
        assert_eq!(select_all(&rolls, &set(&[]), Some(&"C")), set(&["b1"]), "a focus not shown is none");
        let with_empty = [a, b, c];
        assert_eq!(select_all(&with_empty, &set(&[]), Some(&"C")), set(&["b1"]), "a focus on an empty group is none");
        assert_eq!(completed_groups(&rolls, &set(&["b1"]), Some(&"A")), HashSet::from(["B"]));
        // Twin: the focus read before the selection would complete A.
        assert_ne!(select_all(&rolls, &set(&["b1"]), Some(&"A")), set(&["a1", "a2", "b1"]));
    }

    /// `EditorScene.columnKeys`.
    #[test]
    fn command_a_beside_a_photo_completes_its_folders_else_the_ring_else_the_open_photos() {
        let folders = [("/r/A".to_string(), order(&["/r/A/1", "/r/A/2"])), ("/r/B".to_string(), order(&["/r/B/1"]))];
        assert_eq!(column_select_all(&folders, &set(&["/r/A/1"]), None, Some("/r/B/1")), set(&["/r/A/1", "/r/A/2"]));
        assert_eq!(column_select_all(&folders, &set(&[]), Some("/r/B"), Some("/r/A/1")), set(&["/r/B/1"]), "the ring before the open photo");
        assert_eq!(column_select_all(&folders, &set(&[]), None, Some("/r/A/2")), set(&["/r/A/1", "/r/A/2"]));
        assert_eq!(column_select_all(&folders, &set(&[]), None, None), set(&[]), "nothing to complete");
        // Twin: the gallery's ⌘A completes its last group, which the editor's never falls back to.
        assert_ne!(select_all(&folders, &set(&[]), None), column_select_all(&folders, &set(&[]), None, None));
    }

    #[test]
    fn a_blank_or_unchanged_name_is_refused_and_a_menu_verb_counts_past_one() {
        assert_eq!(named("  Iceland ", "Roll 1").as_deref(), Some("Iceland"));
        assert_eq!(named("   ", "Roll 1"), None);
        assert_eq!(named("Roll 1", "Roll 1"), None);
        assert_eq!(menu_label("Paste Settings", 1), "Paste Settings");
        assert_eq!(menu_label("Paste Settings", 3), "Paste Settings (3 photos)");
    }

    #[test]
    fn the_status_bar_names_the_photo_or_counts_and_its_picker_toggles_one_photo_only() {
        assert_eq!(status_path(&set(&["/a"]), false, Some("/b")).as_deref(), Some("/a"));
        assert_eq!(status_path(&set(&[]), true, Some("/b")).as_deref(), Some("/b"));
        assert_eq!(status_path(&set(&[]), false, Some("/b")), None, "the gallery with nothing selected names none");
        assert_eq!(status_path(&set(&["/a", "/c"]), true, Some("/b")), None);
        assert_eq!(status_label("/r/DSC_1.NEF", Some((6000, 4000))), "DSC_1.NEF \u{b7} 6000 \u{d7} 4000");
        assert_eq!(status_leading(Some((0, 1)), None, 0, 9), "1 photo in the ONI Trash");
        assert_eq!(status_leading(Some((2, 5)), Some("x".into()), 0, 9), "2 selected");
        assert_eq!(status_leading(None, None, 0, 9), "9 photos");
        assert_eq!(status_leading(None, None, 3, 9), "3 selected");
        let label = |p: &str| if p == "/g" { Medal::Gold } else { Medal::None };
        assert_eq!(medal_target(None, &set(&["/g", "/n"]), &label).map(|t| t.0), Some(None), "a mixed selection shows none lit");
        assert_eq!(medal_target(None, &set(&["/g"]), &label).map(|t| t.0), Some(Some(Medal::Gold)));
        assert_eq!(medal_target(None, &set(&[]), &label), None);
        assert_eq!(medal_tapped(Some(Medal::Gold), 1, Medal::Gold), Medal::None);
        // Twin: on several photos the same tap lays the medal rather than taking it off.
        assert_eq!(medal_tapped(Some(Medal::Gold), 2, Medal::Gold), Medal::Gold);
    }
}
