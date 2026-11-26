//! The direction's own checks: what each screen must show, each with a twin that would fail.

use super::direction::{Cue, frame, gift_spot};
use super::unwrap;

#[test]
fn a_screen_shows_its_gift_alone_and_tied_until_it_is_opened() {
    let wrapped = frame(&Cue::at(Some(2), None));
    assert!(wrapped.gifts[2].place[3] > 0.0 && wrapped.gifts[2].tie[..3] == [1.0, 1.0, 1.0], "the huge gift, tied");
    assert!([0, 1, 3].iter().all(|&k| wrapped.gifts[k].place[3] == 0.0), "and no other");
    let opened = frame(&Cue::at(Some(2), Some(unwrap::length(2) + 1.0)));
    assert!(opened.gifts[2].tie[..2] == [0.0, 0.0] && opened.gifts[2].lid[1] < 0.0, "opened, its bow gone, its lid down behind it");
    // Twin: the message's screen shows no gift, and the stage goes dark.
    let message = frame(&Cue::at(None, None));
    assert!(message.gifts.iter().all(|g| g.place[3] == 0.0) && message.view[3] == 0.0);
}

#[test]
fn the_gift_stands_in_the_middle_of_the_picture() {
    for gift in 0..4 {
        let shot = frame(&Cue::at(Some(gift), None));
        let [x, y, w, h] = gift_spot(&shot, 1.6).expect("a gift on its screen");
        let middle = (x + w / 2.0, y + h / 2.0);
        assert!((middle.0 - 0.5).abs() < 0.08 && (middle.1 - 0.5).abs() < 0.2, "gift {gift} about the middle, {middle:?}");
        assert!(h > 0.2 && h < 0.75, "gift {gift} neither lost nor cut, {h}");
    }
    assert!(gift_spot(&frame(&Cue::at(None, None)), 1.6).is_none(), "twin: no place on the message's screen");
}

#[test]
fn each_gift_comes_undone_in_its_own_way_and_ends_as_it_rests_opened() {
    for gift in 0..4 {
        let (start, end) = (unwrap::untied(gift, 0.0), unwrap::untied(gift, unwrap::length(gift)));
        assert_eq!(start, unwrap::Parts::tied(), "gift {gift} starts tied");
        let rest = unwrap::opened(gift);
        assert!(end.tie[..2].iter().all(|t| t.abs() < 1e-3) && (end.tie[2] - 0.5).abs() < 1e-3, "gift {gift}: its bow gone, its bands fallen open, {:?}", end.tie);
        assert!(end.lid.iter().zip(rest.lid).all(|(a, b)| (a - b).abs() < 0.08) || gift == 3, "gift {gift}: its lid where it rests, {:?} against {:?}", end.lid, rest.lid);
        assert!(unwrap::title(gift) < unwrap::length(gift), "gift {gift}: its words arrive before it rests");
    }
    // The mystery alone falls open; twin: the others keep their walls.
    assert_eq!(unwrap::opened(3).open, 1.0);
    assert!((0..3).all(|gift| unwrap::opened(gift).open == 0.0 && unwrap::untied(gift, unwrap::length(gift)).open == 0.0));
}
