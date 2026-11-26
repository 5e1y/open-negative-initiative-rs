//! The direction's own checks: what each act must show, each with a twin that would fail.

use super::direction::{Act, Cue, OVERTURE, frame};
use super::unwrap;

#[test]
fn a_gift_stays_tied_until_its_own_act_unties_it() {
    let hall = frame(&Cue::at(Act::Hall, 3.0));
    assert!(hall.gifts.iter().all(|g| g.tie == [1.0, 1.0, 1.0, 0.0] && g.lid == [0.0; 4]), "every gift tied in the hall");
    let end = frame(&Cue::at(Act::Unwrap(2), unwrap::length(2)));
    assert_eq!(&end.gifts[2].tie[..3], &[0.0, 0.0, 0.5], "the chosen gift's bow gone, its bands fallen open");
    assert!(end.gifts[2].tie[3] >= 1.0 && end.gifts[2].lid[1] < 0.0, "its light out, its lid down behind it");
    // Twin: the others keep their ribbon, in the dark.
    assert!(end.gifts[0].tie[0] == 1.0 && end.gifts[0].pose[3] == 0.0);
}

#[test]
fn the_overture_lights_every_gift_where_it_stands_by_its_end() {
    let start = frame(&Cue::at(Act::Overture, 0.0));
    assert!(start.light[..3] == [0.0; 3] && start.gifts.iter().all(|g| g.pose[1] == 0.0), "the gifts on the floor, no light on yet");
    let end = frame(&Cue::at(Act::Overture, OVERTURE));
    assert!(end.light[0] == 1.0 && end.gifts.iter().all(|g| g.pose[1] == 0.0 && g.pose[3] == 1.0), "every gift lit where it stood");
    // Twin: the huge one is lit alone first, the others still dark.
    let alone = frame(&Cue::at(Act::Overture, 4.8));
    assert!(alone.light[0] > 0.9 && alone.gifts[2].pose[3] == 1.0 && alone.gifts[0].pose[3] == 0.0);
    // The same moment on the welcome's clock, as the hall's camera drifts on it.
    let hall = frame(&Cue { clock: OVERTURE, ..Cue::at(Act::Hall, 0.0) });
    assert!(end.eye.iter().zip(hall.eye).all(|(a, b)| (a - b).abs() < 1e-3), "the overture ends on the hall's stand");
}

#[test]
fn the_plate_shows_only_on_the_huge_gifts_page() {
    assert_eq!(frame(&Cue::at(Act::Page(1), 1.0)).plate[3], 0.0);
    let mut cue = Cue::at(Act::Page(2), 1.0);
    cue.step_t = 3.0;
    assert!(frame(&cue).plate[3] > 0.0, "twin: the huge gift's page raises it");
}

#[test]
fn each_gift_comes_undone_in_its_own_way_and_ends_as_it_rests_opened() {
    for gift in 0..4 {
        let (start, end) = (unwrap::untied(gift, 0.0), unwrap::untied(gift, unwrap::length(gift)));
        assert_eq!(start, unwrap::Parts::tied(), "gift {gift} starts tied");
        let rest = unwrap::opened(gift);
        assert!(end.tie[..2].iter().all(|t| t.abs() < 1e-3) && (end.tie[2] - 0.5).abs() < 1e-3, "gift {gift}: its bow gone, its bands fallen open, {:?}", end.tie);
        assert!(end.lid.iter().zip(rest.lid).all(|(a, b)| (a - b).abs() < 0.08) || gift == 3, "gift {gift}: its lid where it rests, {:?} against {:?}", end.lid, rest.lid);
        assert!(unwrap::title(gift) < unwrap::length(gift), "gift {gift}: its title arrives before its page");
    }
    // The mystery alone falls open; twin: the others keep their walls.
    assert_eq!(unwrap::opened(3).open, 1.0);
    assert!((0..3).all(|gift| unwrap::opened(gift).open == 0.0 && unwrap::untied(gift, unwrap::length(gift)).open == 0.0));
}

#[test]
fn the_film_ends_on_its_print_then_shows_each_stock_once_and_the_three_together() {
    use super::film::{FINALE, PRINTED, SECOND, THIRD, plate};
    let exposed = plate(1, 2.7);
    assert!(exposed.strip == 1.0 && exposed.slide == 0.0 && exposed.sheet == 0.0 && exposed.latent == 1.0 && exposed.developed == 0.0, "the strip stopped, the photo gone into it, nothing developed");
    let scanned = plate(3, 10.0);
    assert!(scanned.head > 1.5 && scanned.inverted == 0.0 && scanned.lamp == 1.0 && scanned.developed == 1.0, "scanned whole, still a negative");
    let printed = plate(4, PRINTED);
    assert!(printed.inverted == 1.0 && printed.strip == 0.0 && printed.sheet == 1.0 && printed.forward == 1.0 && printed.stock == 0.0, "inverted whole, the strip gone, the first stock's print");
    assert!((plate(4, SECOND + 1.0).stock - 1.0).abs() < 1e-3 && (plate(4, THIRD + 1.0).stock - 2.0).abs() < 1e-3 && plate(4, THIRD + 1.0).fan == 0.0, "the second stock, then the third");
    // Twin: past the last turn it stops on the first again, the three fanned out, and turns no more.
    let end = plate(4, FINALE + 60.0);
    assert!(end.stock == 3.0 && end.fan == 1.0);
}
