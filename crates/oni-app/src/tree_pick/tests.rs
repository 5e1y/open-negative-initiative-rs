use super::*;

fn rows() -> Vec<String> {
    ["F", "a", "b", "c", "d"].iter().map(|s| s.to_string()).collect()
}

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_command_click_gathers_the_row_a_plain_click_anchored_on() {
    let order = rows();
    let mut pick = TreePick::default();
    assert!(!pick.click(&"a".to_string(), &order, false, false), "a plain click picks nothing");
    assert!(pick.click(&"c".to_string(), &order, true, false));
    assert_eq!(pick.selection, set(&["a", "c"]), "the anchor joins the first ⌘ pick");
    assert!(pick.click(&"c".to_string(), &order, true, false));
    assert_eq!(pick.selection, set(&["a"]), "a second ⌘ click takes the row out");
    pick.clear();
    pick.click(&"b".to_string(), &order, true, false);
    assert_eq!(pick.selection, set(&["b", "c"]), "twin: the anchor is the last ⌘ row, never the first plain one");
    assert!(!pick.click(&"d".to_string(), &order, false, false));
    assert!(pick.selection.is_empty(), "a plain click empties the pick");
}

#[test]
fn a_shift_click_adds_the_rows_between_the_anchor_and_it() {
    let order = rows();
    let mut pick = TreePick::default();
    pick.click(&"d".to_string(), &order, false, false);
    assert!(pick.click(&"a".to_string(), &order, false, true));
    assert_eq!(pick.selection, set(&["a", "b", "c", "d"]), "backwards as well");
    assert_eq!(pick.anchor.as_deref(), Some("d"), "⇧ keeps the anchor");
    let mut lost = TreePick { selection: HashSet::new(), anchor: Some("gone".to_string()) };
    assert!(!lost.click(&"b".to_string(), &order, false, true), "twin: an anchor not shown makes it a plain click");
    assert_eq!(lost.anchor.as_deref(), Some("b"));
}

#[test]
fn a_right_click_inside_the_pick_takes_it_whole_in_the_order_shown() {
    let order = rows();
    let pick = TreePick { selection: set(&["c", "F", "a"]), anchor: Some("c".to_string()) };
    assert_eq!(pick.targets(&"a".to_string(), &order), ["F", "a", "c"]);
    assert_eq!(pick.targets(&"b".to_string(), &order), ["b"], "twin: a row outside the pick is alone");
}

#[test]
fn a_folder_picked_brings_every_photo_surveyed_under_it() {
    let members = ["/r/A/1.tif", "/r/A/B/2.tif", "/r/AB/3.tif", "/r/C/4.tif"].map(String::from);
    let picked = HashSet::from([NodeId::Folder("/r/A".into()), NodeId::Photo("/r/C/4.tif".into())]);
    assert_eq!(picked_photos(&picked, &members), set(&["/r/A/1.tif", "/r/A/B/2.tif", "/r/C/4.tif"]));
    assert!(!picked_photos(&picked, &members).contains("/r/AB/3.tif"), "twin: a sibling sharing the name's start stays out");
}
