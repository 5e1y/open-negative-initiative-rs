use super::Paint;

#[test]
fn a_thumbnail_waiting_or_being_drawn_again_is_stale_and_no_other() {
    let mut paint = Paint::default();
    paint.redraws.push_back(("a".into(), None));
    paint.drawing = Some("b".into());
    assert!(paint.stale("a") && paint.stale("b"));
    assert!(!paint.stale("c"), "twin: a photo no redraw waits for stands in as drawn");
}
