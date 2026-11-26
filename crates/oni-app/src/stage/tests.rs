//! The direction's own checks: what each screen must show, each with a twin that would fail.

use super::direction::{Cue, frame, gift_spot};
use super::unwrap;

#[test]
fn a_screen_shows_its_gift_alone_and_tied_until_it_is_opened() {
    let wrapped = frame(&Cue::at(Some(2), None));
    assert!(wrapped.gifts[2].place[3] > 0.0 && wrapped.gifts[2].lid == [0.0; 4] && wrapped.gifts[2].tie == [0.0; 4], "the huge gift, closed, no ribbon");
    assert!([0, 1, 3].iter().all(|&k| wrapped.gifts[k].place[3] == 0.0), "and no other");
    let opened = frame(&Cue::at(Some(2), Some(unwrap::length(2) + 1.0)));
    assert!(opened.gifts[2].lid[1] < 0.0, "opened, its lid down behind it");
    // Twin: the message's screen shows no gift, and the stage goes dark.
    let message = frame(&Cue::at(None, None));
    assert!(message.gifts.iter().all(|g| g.place[3] == 0.0) && message.view[3] == 0.0);
}

#[test]
fn each_gift_stands_in_the_middle_at_the_size_its_words_tell() {
    let height = |gift| {
        let [x, y, w, h] = gift_spot(&frame(&Cue::at(Some(gift), None)), 1.6).expect("a gift on its screen");
        assert!((x + w / 2.0 - 0.5).abs() < 0.08 && y >= 0.0 && y + h <= 0.95, "gift {gift} about the middle, whole: {x} {y} {w} {h}");
        h
    };
    let sizes = [height(0), height(1), height(2), height(3)];
    assert!(sizes[2] > 0.7, "the huge gift fills the picture's height: {sizes:?}");
    assert!(sizes[0] < 0.35 && sizes[1] < 0.35, "the small ones small: {sizes:?}");
    // Twin: the mystery between them and the huge one.
    assert!(sizes[3] > sizes[0].max(sizes[1]) && sizes[3] < sizes[2] * 0.75, "the mystery between: {sizes:?}");
    assert!(gift_spot(&frame(&Cue::at(None, None)), 1.6).is_none(), "no place on the message's screen");
}

#[test]
fn an_opened_gift_fades_once_its_words_come() {
    use super::direction::kept;
    assert_eq!(kept(2, None), 1.0, "wrapped, whole");
    assert_eq!(kept(2, Some(unwrap::title(2) - 0.1)), 1.0, "opening, whole");
    // Twin: once its words have come, gone.
    assert_eq!(kept(2, Some(unwrap::title(2) + 1.0)), 0.0);
    assert_eq!(frame(&Cue::at(Some(2), Some(unwrap::length(2) + 1.0))).view[3], 0.0);
}

#[test]
fn each_gift_comes_undone_in_its_own_way_and_ends_as_it_rests_opened() {
    for gift in 0..4 {
        let (start, end) = (unwrap::untied(gift, 0.0), unwrap::untied(gift, unwrap::length(gift)));
        assert_eq!(start, unwrap::Parts::tied(), "gift {gift} starts tied");
        let rest = unwrap::opened(gift);
        assert!(end.tie[..3].iter().all(|t| *t == 0.0), "gift {gift}: no ribbon at any time, {:?}", end.tie);
        assert!(end.lid.iter().zip(rest.lid).all(|(a, b)| (a - b).abs() < 0.08) || gift == 3, "gift {gift}: its lid where it rests, {:?} against {:?}", end.lid, rest.lid);
        assert!(unwrap::title(gift) < unwrap::length(gift), "gift {gift}: its words arrive before it rests");
    }
    // The mystery alone falls open; twin: the others keep their walls.
    assert_eq!(unwrap::opened(3).open, 1.0);
    assert!((0..3).all(|gift| unwrap::opened(gift).open == 0.0 && unwrap::untied(gift, unwrap::length(gift)).open == 0.0));
}
