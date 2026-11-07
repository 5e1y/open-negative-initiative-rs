//! The tree's rows under a rating filter: its one medal's photos alone, each folder counting them.

use super::tests::synthetic;
use super::*;

/// The photos' rows as laid, by name.
fn photos(a: &Archive) -> Vec<String> {
    a.rows().into_iter().filter(|r| r.kind == 2).map(|r| r.title).collect()
}

/// A folder row's count and whether it opens.
fn counted(a: &Archive, title: &str) -> Option<(String, bool)> {
    a.rows().into_iter().find(|r| r.title == title).map(|r| (r.caption, r.expandable))
}

#[test]
fn a_filter_lays_its_medals_photos_alone_and_each_folder_counts_them() {
    let (mut rows, survey) = synthetic();
    let folder = "/R/Scans";
    let roll = |name: &str| NodeId::Folder(format!("{folder}/{name}"));
    rows[4].label = Medal::Gold;
    rows[1].label = Medal::Silver;
    let gold = Archive::from_rows(folder, &rows, &survey, Some(Medal::Gold));
    assert_eq!(photos(&gold), ["b2.NEF"]);
    assert_eq!(counted(&gold, "Roll A"), Some(("0".into(), false)), "a roll with no gold counts none");
    assert_eq!(counted(&gold, "Roll B"), Some(("1".into(), true)));
    // A folder's row opens on its first photo shown, a roll with none on none.
    assert_eq!(gold.first_shown(&roll("Roll B")), Some(format!("{folder}/Roll B/b2.NEF")));
    assert_eq!(gold.first_shown(&roll("Roll A")), None);
    // Twin: every photo shown lays all six, Roll B counting three and opening on b1.
    let all = Archive::from_rows(folder, &rows, &survey, None);
    assert_eq!(photos(&all).len(), 6);
    assert_eq!(counted(&all, "Roll B"), Some(("3".into(), true)));
    assert_eq!(all.first_shown(&roll("Roll B")), Some(format!("{folder}/Roll B/b1.NEF")));

    // A medal laid under the filter brings its row, and another filter lays its own medal's.
    let mut laid = rows.clone();
    laid[0].label = Medal::Gold;
    let (more, how) = Archive::relaid(Some(gold), None, folder, &laid, &survey, Some(Medal::Gold));
    assert_eq!((how, photos(&more)), (Relaid::Regrouped, vec!["b2.NEF".to_string(), "a1.NEF".to_string()]));
    let (silver, how) = Archive::relaid(Some(more), None, folder, &laid, &survey, Some(Medal::Silver));
    assert_eq!((how, photos(&silver)), (Relaid::Regrouped, vec!["a2.NEF".to_string()]));
    // Twin: the rows kept from Gold's lay would still read its two.
    assert_ne!(photos(&silver), ["b2.NEF", "a1.NEF"]);
}
