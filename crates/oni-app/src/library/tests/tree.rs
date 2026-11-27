//! The archive's tree laid again from its rows: whole on the same rows, a medal moving its row alone, a tree cut off the thread.

use super::*;

#[test]
fn a_lay_on_the_same_rows_keeps_the_tree_and_a_medal_moves_its_row_alone() {
    let (rows, survey) = synthetic();
    let folder = "/R/Scans";
    let cut = Archive::from_rows(folder, &rows, &survey, Filter::default());
    let laid = cut.rows();
    // The same rows under a survey read again, its sidecars' dates moved: the tree held stands.
    let mut again = (*survey).clone();
    again.sidecars.insert(rows[0].path.clone(), Date::since_reference_date(9e8));
    let (kept, how) = Archive::relaid(Some(cut), None, folder, &rows, &Arc::new(again), Filter::default());
    assert_eq!((how, kept.rows()), (Relaid::Kept, laid.clone()));
    // A medal moves its photo's caption and its gallery entry, nothing else.
    let mut medalled = rows.clone();
    medalled[4].label = Medal::Gold;
    let (archive, how) = Archive::relaid(Some(kept), None, folder, &medalled, &survey, Filter::default());
    assert_eq!(how, Relaid::Medals);
    let changed: Vec<usize> = archive.rows().iter().zip(&laid).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| i).collect();
    let b2 = archive.rows().iter().position(|r| r.id == NodeId::Photo(rows[4].path.clone())).unwrap();
    assert_eq!((changed, archive.rows()[b2].caption.clone()), (vec![b2], medal(Medal::Gold)));
    assert_eq!(archive.entries().iter().find(|e| e.path == rows[4].path).map(|e| e.label), Some(Medal::Gold));
    // Under a filter the medal cuts the gallery and the tree's rows again, the tree and Roll A's band kept.
    let (filtered, how) = Archive::relaid(Some(archive), None, folder, &medalled, &survey, Filter::of_medal(Medal::Gold));
    let roll_a = filtered.band("/R/Scans/Roll A").map(|g| g.entries.len());
    assert_eq!((how, filtered.entries().len(), filtered.rows().len(), roll_a), (Relaid::Regrouped, 1, laid.len() - 5, Some(3)));
    // Twin: a row that moved otherwise, its print here, cuts the tree whole.
    let mut moved = medalled.clone();
    moved[1].fingerprint = "print-new".into();
    assert_eq!(Archive::relaid(Some(filtered), None, folder, &moved, &survey, Filter::default()).1, Relaid::Cut);
    // Twin: a folder new on the disk cuts it too.
    let mut grown = (*survey).clone();
    grown.folders.push("/R/Scans/Roll C".into());
    let held = Archive::from_rows(folder, &rows, &survey, Filter::default());
    assert_eq!(Archive::relaid(Some(held), None, folder, &rows, &Arc::new(grown), Filter::default()).1, Relaid::Cut);
}

#[test]
fn a_tree_cut_off_the_thread_is_taken_only_for_the_rows_it_was_cut_from() {
    let (rows, survey) = synthetic();
    let folder = "/R/Scans";
    // A staged archive marked by its folds, so taking it shows in its rows.
    let staged = |rows: &[Entry]| {
        let mut a = Archive::from_rows(folder, rows, &survey, Filter::default());
        a.set_expanded(HashSet::new());
        a
    };
    let (taken, how) = Archive::relaid(None, Some(staged(&rows)), folder, &rows, &survey, Filter::default());
    assert_eq!((how, taken.rows().len()), (Relaid::Cut, 1), "the staged archive is taken as cut");
    // Twin: one cut from other rows is refused, the lay cutting its own from the rows held.
    let fewer = &rows[..5];
    let (cut, _) = Archive::relaid(None, Some(staged(fewer)), folder, &rows, &survey, Filter::default());
    assert_eq!((cut.rows().len(), cut.entries().len()), (10, 6), "every folder unfolded over the six photos held");
}
