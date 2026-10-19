use super::*;

/// `RollPreviews.selfCheck`'s bytes: a colour pixel's complement, a grey film's luma complement.
#[test]
fn a_negative_reads_as_its_complement_and_a_grey_film_on_its_luma() {
    let mut colour = vec![13, 201, 67, 255];
    inverted(&mut colour, false);
    assert_eq!(colour, [242, 54, 188, 255]);
    let mut grey = vec![13, 201, 67, 255];
    inverted(&mut grey, true);
    assert_eq!(grey, [104, 104, 104, 255]);
    // Twin: under a half alpha the complement is taken under the alpha, never under 255.
    let mut half = vec![13, 201, 67, 128];
    inverted(&mut half, false);
    assert_eq!(half, [115, 0, 61, 128]);
    assert_ne!(half[..3], [242, 54, 188]);
}

#[test]
fn a_tile_shows_the_frame_at_rest_on_the_rolls_film_and_flat_field() {
    let mut base = PipelineSettings::rest(ConversionMode::Negative, true);
    base.levels.red.black = 0.3;
    let flat = FlatField { mask: oni_settings::frame::FlatFieldMask::ValoiEasy35V1, ..FlatField::default() };
    let s = rendered(&base, &Look { film: ConversionMode::Positive, flat: Some(flat.clone()) });
    assert_eq!((s.mode, s.flat_field.clone()), (ConversionMode::Positive, flat));
    // Twin: no flat-field on the look leaves the frame's own.
    let own = rendered(&base, &Look { film: ConversionMode::Negative, flat: None });
    assert_eq!(own.flat_field, base.flat_field);
}

#[test]
fn the_card_comes_first_and_a_new_look_waits_for_the_grids_pause() {
    let mut q = Queue::default();
    let s = PipelineSettings::rest(ConversionMode::Negative, true);
    let source = Arc::new(Source { linear: Linear { width: 1, height: 1, pixels: vec![[0.5; 4]] }, full_width: 1.0 });
    q.settings.insert("a".into(), s.clone());
    q.sources.insert("a".into(), source);
    q.look = Some(Look { film: ConversionMode::Negative, flat: None });
    q.shown = Some("a".into());
    q.card_now = true;
    q.decode.push_back("b".into());
    let now = Instant::now();
    assert!(matches!(q.next(now), Ok(Work::Card(p, ..)) if p == "a"));
    assert!(matches!(q.next(now), Ok(Work::Decode(p)) if p == "b"));
    q.retile_at = Some(now + GRID_PAUSE);
    // Twin: before the pause no tile is rendered again; after it, every source is.
    assert!(matches!(q.next(now), Err(Some(_))));
    assert!(matches!(q.next(now + GRID_PAUSE), Ok(Work::Tile(p, ..)) if p == "a"));
}
