use super::*;

fn pair() -> (PipelineSettings, PipelineSettings) {
    let mut neutral = PipelineSettings::default();
    neutral.gains.stops = [0.0, 0.0, 0.0];
    let mut edited = PipelineSettings::default();
    edited.gains.stops = [1.0, 0.0, -1.0];
    (neutral, edited)
}

fn map(items: &[(&str, &PipelineSettings)]) -> Settings {
    items.iter().map(|(p, s)| (p.to_string(), (*s).clone())).collect()
}

#[test]
fn a_gesture_undoes_and_redoes_whole_and_an_idle_one_opens_no_step() {
    let (neutral, edited) = pair();
    let mut h = History::default();
    assert!(!h.can_undo() && !h.can_redo());
    h.record(
        &map(&[("/a", &edited), ("/b", &edited)]),
        &map(&[("/a", &neutral), ("/b", &neutral)]),
        "Reset 2 photos",
        None,
    );
    assert_eq!(h.undo_label(), Some("Reset 2 photos"));
    let undone = h.undo().unwrap();
    assert_eq!((undone.len(), undone.get("/a")), (2, Some(&edited)));
    assert!(!h.can_undo() && h.can_redo());
    assert_eq!(h.redo().unwrap().get("/a"), Some(&neutral));

    let mut h2 = History::default();
    h2.record(
        &map(&[("/a", &edited), ("/b", &neutral)]),
        &map(&[("/a", &neutral), ("/b", &neutral)]),
        "Reset",
        None,
    );
    assert_eq!(h2.undo().unwrap().len(), 1, "an unchanged photo stays out of the step");
    // Twin: a gesture with no effect opens no step at all.
    let mut h3 = History::default();
    h3.record(&map(&[("/a", &neutral)]), &map(&[("/a", &neutral)]), "Reset", None);
    assert!(!h3.can_undo());

    let mut h4 = History::default();
    h4.record(&map(&[("/a", &edited)]), &map(&[("/a", &neutral)]), "un", None);
    h4.undo();
    h4.record(&map(&[("/a", &edited)]), &map(&[("/a", &neutral)]), "deux", None);
    assert!(h4.can_undo() && !h4.can_redo(), "a new gesture erases the undone branch");
}

#[test]
fn a_trash_step_comes_back_whole_and_an_empty_one_opens_none() {
    let trash = Trash { paths: BTreeSet::from(["/a".into(), "/b".into()]), marked: true };
    let mut h = History::default();
    h.record_trash(trash.clone(), "Move 2 Photos to ONI Trash");
    assert_eq!(h.undo_step().and_then(|s| s.trash), Some(trash));
    assert!(h.redo_step().and_then(|s| s.trash).is_some_and(|t| t.marked));
    // Twin: a restore that touched nothing opens no step.
    let mut empty = History::default();
    empty.record_trash(Trash { paths: BTreeSet::new(), marked: false }, "Restore");
    assert!(!empty.can_undo());
    let mut reset = History::default();
    reset.record(
        &Settings::new(),
        &Settings::new(),
        "Reset",
        Some(Trash { paths: BTreeSet::from(["/sheet.tif".into()]), marked: true }),
    );
    let mut bare = History::default();
    bare.record(&Settings::new(), &Settings::new(), "Reset", None);
    assert!(reset.can_undo() && !bare.can_undo());
}

#[test]
fn a_medal_step_keeps_what_each_photo_wore_and_one_that_changed_nothing_opens_none() {
    let worn = BTreeMap::from([("/a".to_string(), Medal::None), ("/b".to_string(), Medal::Gold)]);
    let mut h = History::default();
    h.record_medals(Medals { before: worn, after: Medal::Gold }, "Gold");
    let step = h.undo_step().and_then(|s| s.medals).expect("a medal step");
    assert_eq!(step.before, BTreeMap::from([("/a".to_string(), Medal::None)]), "a photo already gold stays out");
    assert_eq!(h.redo_paths(), BTreeSet::from(["/a".to_string()]));
    // Twin: the medal every photo already wears opens no step.
    let mut idle = History::default();
    idle.record_medals(Medals { before: BTreeMap::from([("/a".to_string(), Medal::Gold)]), after: Medal::Gold }, "Gold");
    assert!(!idle.can_undo());
}

/// Add to Project is one step: the projects it changed, each with its members before and after,
/// carried by a folder renamed since, their prints kept; it names no photo, so an undo redraws none.
#[test]
fn a_members_step_keeps_the_projects_it_changed_and_follows_a_rename() {
    let (client, other) = (Id::new(), Id::new());
    let list = |items: &[&str]| items.iter().map(|s| crate::project::Member::new(Some(s), Some("print"))).collect::<Vec<_>>();
    let roster = |id: Id, items: &[&str]| (id, crate::project::Project { id, ..crate::project::Project::new("P", oni_settings::Date::since_reference_date(8e8), list(items)) });
    let before = BTreeMap::from([roster(client, &["/r10/a"]), roster(other, &["/r8/b"])]);
    let after = BTreeMap::from([roster(client, &["/r10/a", "/r8/a"]), roster(other, &["/r8/b"])]);
    let mut h = History::default();
    h.record_members(Members { before, after }, "Add to Project");
    assert_eq!((h.undo_label(), h.undo_paths()), (Some("Add to Project"), BTreeSet::new()));
    h.follow(&[Move { from: "/r8".into(), to: "/Roll 8".into() }]);
    let step = h.undo_step().and_then(|s| s.members).expect("a members step");
    assert_eq!(step.before, BTreeMap::from([roster(client, &["/r10/a"])]), "a project left as it was stays out");
    assert_eq!(step.after[&client].members, list(&["/r10/a", "/Roll 8/a"]));
    // Twin: a gesture that changed no project's members opens no step.
    let mut idle = History::default();
    let same = BTreeMap::from([roster(client, &["/r10/a"])]);
    idle.record_members(Members { before: same.clone(), after: same }, "Add to Project");
    assert!(!idle.can_undo());
}

/// A project's name and place are no step: a reorder opens none, so ⌘Z after it takes back New
/// Project, whose undo keeps the project it took away as it went; the redo brings back the name typed.
#[test]
fn a_reorder_opens_no_step_and_a_redo_of_new_project_keeps_the_name_typed_since() {
    use crate::project::{self, Project};
    let day = oni_settings::Date::since_reference_date(8e8);
    let mut projects = vec![Project::new("Old", day, Vec::new())];
    let old = projects[0].id;
    let mut h = History::default();
    let before = project::rosters(&projects);
    let made = project::new_empty(&mut projects, crate::later(day, 60.0));
    h.record_members(Members { before, after: project::rosters(&projects) }, "New Project");
    let before = project::rosters(&projects);
    assert!(project::reorder_root(&mut projects, &std::collections::HashSet::from([old]), made));
    h.record_members(Members { before: before.clone(), after: project::rosters(&projects) }, "Move");
    assert_eq!(h.undo_label(), Some("New Project"), "the reorder opened no step");
    // Twin: compared whole, the reordered projects differ.
    assert_ne!(before, project::rosters(&projects));
    project::rename(&mut projects, made, "Client");
    let step = h.undo_step().and_then(|s| s.members).expect("New Project's step");
    let found = projects.clone();
    project::regroup(&mut projects, &step.after, &step.before);
    assert!(projects.iter().all(|p| p.id != made), "the undo takes it away");
    // Twin: the step as recorded brings it back under the name it was made with.
    let mut recorded = projects.clone();
    project::regroup(&mut recorded, &step.before, &step.after);
    assert_eq!(recorded.iter().find(|p| p.id == made).map(|p| p.name.as_str()), Some("Project 1"));
    h.walked(true, &found, &projects);
    let step = h.redo_step().and_then(|s| s.members).expect("New Project's step");
    project::regroup(&mut projects, &step.before, &step.after);
    assert_eq!(projects.iter().find(|p| p.id == made).map(|p| p.name.as_str()), Some("Client"));
}

/// New Project from a, which sat in Old, renamed then deleted by hand: ⌘Z gives a back to Old, and
/// ⌘⇧Z, whose project that undo found gone, makes none and leaves a where it stands.
#[test]
fn a_redo_never_makes_again_a_project_deleted_since() {
    use crate::project::{self, Member, Project};
    let day = oni_settings::Date::since_reference_date(8e8);
    let a = Member::new(Some("/r/a"), Some("1-a"));
    let mut projects = vec![Project::new("Old", day, vec![a.clone()])];
    let rows = [crate::index::Entry::new("/r/a", day, "1-a")];
    let mut h = History::default();
    let before = project::rosters(&projects);
    let made = project::new_from(&mut projects, &project::Reach::rows(&rows), &std::collections::HashSet::from(["/r/a".to_string()]), crate::later(day, 60.0));
    h.record_members(Members { before, after: project::rosters(&projects) }, "New Project");
    project::rename(&mut projects, made.expect("a project from a"), "Client");
    project::delete(&mut projects, made.unwrap());
    let step = h.undo_step().and_then(|s| s.members).expect("New Project's step");
    let found = projects.clone();
    project::regroup(&mut projects, &step.after, &step.before);
    let held = |projects: &[Project]| projects.iter().map(|p| (p.name.clone(), p.members.clone())).collect::<Vec<_>>();
    assert_eq!(held(&projects), [("Old".to_string(), vec![a.clone()])], "a back in Old");
    // Twin: the step as recorded makes the project again and takes a out of Old.
    let mut recorded = projects.clone();
    project::regroup(&mut recorded, &step.before, &step.after);
    assert_eq!(held(&recorded), [("Old".to_string(), vec![]), ("Project 1".to_string(), vec![a.clone()])]);
    h.walked(true, &found, &projects);
    let step = h.redo_step().and_then(|s| s.members).expect("New Project's step");
    project::regroup(&mut projects, &step.before, &step.after);
    assert_eq!(held(&projects), [("Old".to_string(), vec![a])], "no project made, a where it stands");
}

/// Two steps of one name and no photo, Add to Project then a drop onto a project: each bears its
/// own mark, so a notice naming the first never walks back the second.
#[test]
fn each_step_bears_a_mark_no_other_does() {
    let id = Id::new();
    let made = crate::project::Project { id, ..crate::project::Project::new("P", oni_settings::Date::since_reference_date(8e8), Vec::new()) };
    let list = |items: &[&str]| BTreeMap::from([(id, crate::project::Project { members: items.iter().map(|s| crate::project::Member::new(Some(s), None)).collect(), ..made.clone() })]);
    let mut h = History::default();
    h.record_members(Members { before: list(&[]), after: list(&["/a"]) }, "Add to Project");
    let told = h.undo_mark();
    h.record_members(Members { before: list(&["/a"]), after: list(&["/a", "/b"]) }, "Add to Project");
    assert_ne!(h.undo_mark(), told, "the drop's step is not the one the notice named");
    // Twin: by name and photos the two steps are one.
    let named = |h: &History| (h.undo_label().map(str::to_owned), h.undo_paths());
    let second = named(&h);
    h.undo_step();
    assert_eq!(named(&h), second);
    assert_eq!(h.undo_mark(), told, "undone, the first is on top again, its mark its own");
    h.redo_step();
    h.undo_step();
    h.undo_step();
    assert_eq!(h.undo_mark(), None);
}

#[test]
fn a_folder_renamed_after_a_new_roll_is_where_its_undo_walks_from() {
    let (neutral, edited) = pair();
    let mut h = History::default();
    h.record(&map(&[("/p/2026-10-26/A.NEF", &edited)]), &map(&[("/p/2026-10-26/A.NEF", &neutral)]), "Reset", None);
    h.record(&map(&[("/q/B.NEF", &edited)]), &map(&[("/q/B.NEF", &neutral)]), "Reset", None);
    let roll = Disk {
        folder: Some("/p/2026-10-26".into()),
        moves: vec![Move { from: "/p/A.NEF".into(), to: "/p/2026-10-26/A.NEF".into() }],
        ..Disk::default()
    };
    h.record_disk(roll, "New Roll");
    h.follow(&[Move { from: "/p/2026-10-26".into(), to: "/p/Portra".into() }]);
    let disk = h.undo_step().and_then(|s| s.disk).unwrap();
    assert_eq!(disk.moves, [Move { from: "/p/A.NEF".into(), to: "/p/Portra/A.NEF".into() }]);
    assert_eq!(disk.folder.as_deref(), Some("/p/Portra"));
    assert_eq!(h.undo_paths(), BTreeSet::from(["/q/B.NEF".to_string()]), "twin: a photo outside the folder keeps its path");
    h.undo_step();
    assert_eq!(h.undo_paths(), BTreeSet::from(["/p/Portra/A.NEF".to_string()]), "a settings step under it follows");
}

#[test]
fn the_stack_keeps_its_last_steps_up_to_its_limit() {
    let mut h = History::default();
    for i in 0..(LIMIT + 5) {
        h.record_disk(
            Disk { moves: vec![Move { from: format!("/a{i}"), to: "/b".into() }], ..Disk::default() },
            &i.to_string(),
        );
    }
    let mut undone = 0;
    while h.undo_step().is_some() {
        undone += 1;
    }
    assert_eq!(undone, LIMIT);
    // Twin: a disk gesture that moved nothing adds no step.
    let mut idle = History::default();
    idle.record_disk(Disk::default(), "Move");
    assert!(!idle.can_undo());
}

#[test]
fn the_stack_keeps_its_steps_under_a_ceiling_of_bytes() {
    let (neutral, edited) = pair();
    let gesture = |h: &mut History, n: usize| {
        let paths: Vec<String> = (0..n).map(|i| format!("/r/{i}.NEF")).collect();
        let before: Settings = paths.iter().map(|p| (p.clone(), edited.clone())).collect();
        let after: Settings = paths.iter().map(|p| (p.clone(), neutral.clone())).collect();
        h.record(&before, &after, "Reset", None);
    };
    let mut probe = History::default();
    gesture(&mut probe, 50);
    let step = probe.bytes();
    let mut h = History::bounded(step * 3);
    for _ in 0..10 {
        gesture(&mut h, 50);
    }
    let mut undone = 0;
    while h.undo_step().is_some() {
        undone += 1;
    }
    assert_eq!(undone, 3, "three steps of {step} bytes fill the ceiling");
    // The newest step stays, even past the ceiling alone.
    let mut tight = History::bounded(1);
    gesture(&mut tight, 50);
    assert!(tight.can_undo());
    // Twin: counted by steps alone, the ten gestures would all be kept.
    let mut counted = History::bounded(usize::MAX);
    for _ in 0..10 {
        gesture(&mut counted, 50);
    }
    assert_eq!(counted.bytes(), step * 10);
}

fn exposed(by: f32) -> PipelineSettings {
    let mut s = PipelineSettings::default();
    s.print.exposure += by;
    s
}

/// An edit of `path` from `from` to `to`, at `at` seconds.
fn edit(h: &mut History, path: &str, from: &PipelineSettings, to: &PipelineSettings, at: f64) {
    h.record_edit(path, map(&[(path, from)]), map(&[(path, to)]), at);
}

/// How many steps the stack holds, each undone to count it.
fn depth(h: &mut History) -> usize {
    let mut steps = 0;
    while h.undo_step().is_some() {
        steps += 1;
    }
    steps
}

#[test]
fn a_drag_within_the_window_is_one_step() {
    let mut h = History::default();
    for (i, at) in [0.0, 0.1, 0.2, 0.35].into_iter().enumerate() {
        edit(&mut h, "/a", &exposed(0.1 * i as f32), &exposed(0.1 * (i + 1) as f32), at);
    }
    assert_eq!(h.undo_label(), None, "an edit is no named gesture");
    assert_eq!(h.undo(), Some(map(&[("/a", &exposed(0.0))])), "one undo takes the whole drag back");
    // Twin: the same notches spaced past the window are four steps.
    let mut spaced = History::default();
    for (i, at) in [0.0, 0.5, 1.0, 1.5].into_iter().enumerate() {
        edit(&mut spaced, "/a", &exposed(0.1 * i as f32), &exposed(0.1 * (i + 1) as f32), at);
    }
    assert_eq!(depth(&mut spaced), 4);
}

#[test]
fn two_settings_at_once_never_coalesce() {
    let mut both = exposed(0.2);
    both.sharpen.amount = 1.0;
    let mut again = both.clone();
    again.print.exposure += 0.1;
    again.sharpen.amount = 1.2;
    let mut h = History::default();
    edit(&mut h, "/a", &PipelineSettings::default(), &both, 0.0);
    edit(&mut h, "/a", &both, &again, 0.1);
    assert_eq!(depth(&mut h), 2);
    // Twin: one setting twice in the window joins.
    let mut one = History::default();
    edit(&mut one, "/a", &exposed(0.0), &exposed(0.2), 0.0);
    edit(&mut one, "/a", &exposed(0.2), &exposed(0.3), 0.1);
    assert_eq!(depth(&mut one), 1);
}

#[test]
fn an_edit_after_an_undo_drops_the_redo_branch_and_never_joins_the_past() {
    let mut h = History::default();
    edit(&mut h, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    edit(&mut h, "/a", &exposed(0.1), &exposed(0.2), 1.0);
    h.undo();
    edit(&mut h, "/a", &exposed(0.1), &exposed(0.5), 1.1);
    assert!(!h.can_redo());
    assert_eq!(h.undo(), Some(map(&[("/a", &exposed(0.1))])));
    // Twin: without the undo the same notch within the window joins the top.
    let mut kept = History::default();
    edit(&mut kept, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    edit(&mut kept, "/a", &exposed(0.1), &exposed(0.2), 1.0);
    edit(&mut kept, "/a", &exposed(0.2), &exposed(0.5), 1.1);
    assert_eq!(depth(&mut kept), 2);
}

#[test]
fn an_edit_joins_only_its_own_frame_and_a_gesture_between_cuts_it() {
    let mut h = History::default();
    edit(&mut h, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    edit(&mut h, "/b", &exposed(0.0), &exposed(0.1), 0.1);
    assert_eq!(depth(&mut h), 2, "the same setting on another frame is a step of its own");
    let mut cut = History::default();
    edit(&mut cut, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    cut.record_medals(Medals { before: BTreeMap::from([("/b".to_string(), Medal::None)]), after: Medal::Gold }, "Medal");
    edit(&mut cut, "/a", &exposed(0.1), &exposed(0.2), 0.1);
    assert_eq!(depth(&mut cut), 3, "a gesture between two edits keeps them apart");
    // Twin: the same two edits with nothing between are one step.
    let mut joined = History::default();
    edit(&mut joined, "/a", &exposed(0.0), &exposed(0.1), 0.0);
    edit(&mut joined, "/a", &exposed(0.1), &exposed(0.2), 0.1);
    assert_eq!(depth(&mut joined), 1);
}

#[test]
fn the_frames_an_edit_wrote_keep_their_first_before_and_their_last_after() {
    let mut h = History::default();
    let with = |open: f32, other: f32| map(&[("/a", &exposed(open)), ("/b", &exposed(other))]);
    h.record_edit("/a", with(0.0, 0.0), with(0.1, 0.1), 0.0);
    h.record_edit("/a", with(0.1, 0.1), with(0.2, 0.2), 0.1);
    let step = h.undo_step().unwrap();
    assert_eq!((step.before, step.after), (with(0.0, 0.0), with(0.2, 0.2)));
    assert!(!h.can_undo(), "one step");
    // Twin: an edit that wrote other frames alone, the open one unchanged, never joins.
    let mut alone = History::default();
    alone.record_edit("/a", map(&[("/b", &exposed(0.0))]), map(&[("/b", &exposed(0.1))]), 0.0);
    alone.record_edit("/a", map(&[("/b", &exposed(0.1))]), map(&[("/b", &exposed(0.2))]), 0.1);
    assert_eq!(depth(&mut alone), 2);
}
