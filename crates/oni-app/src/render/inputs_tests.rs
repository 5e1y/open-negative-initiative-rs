//! The plots' readings: the input histograms read the frame the canvas shows, its custom mask and dust
//! patches laid, the curves' on its face; the picture is counted for a plot on screen alone.
use super::*;

fn tiff(path: &std::path::Path, w: u32, h: u32, pixels: &[f32]) {
    let out = std::fs::File::create(path).unwrap();
    tiff::encoder::TiffEncoder::new(out).unwrap().write_image::<tiff::encoder::colortype::RGBA32Float>(w, h, pixels).unwrap();
}

/// A mask or a patch the frame names moves both readings; one whose file is missing reads the bare
/// frame, and the same file arriving later moves the reading kept for the same settings.
#[test]
fn the_input_histograms_lay_the_frames_mask_and_patches() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the input histograms' mask and patches are not exercised");
        return;
    };
    let root = std::env::temp_dir().join(format!("oni-inputs-laid-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (support, caches) = (root.join("support"), root.join("caches"));
    let masks = crate::flat_fields::folder(&support);
    std::fs::create_dir_all(&masks).unwrap();
    std::fs::create_dir_all(caches.join("Corrections")).unwrap();
    // A panel falling off to the right, so a flat frame divided by it spreads over many bins.
    let (mw, mh) = (8u32, 4u32);
    let mask: Vec<f32> = (0..mw * mh).flat_map(|i| { let v = 1.0 - 0.6 * (i % mw) as f32 / mw as f32; [v, v, v, 1.0] }).collect();
    tiff(&masks.join("panel.tiff"), mw, mh, &mask);
    let mut engine = Engine::on(&device);
    engine.support = Some(support);
    engine.caches = Some(caches.clone());
    // 64 × 48: a patch the size of the frame is cut to its stroke's tile, laid where it was made.
    let (w, h) = (64usize, 48usize);
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels: vec![[0.3, 0.3, 0.3, 1.0]; w * h] }, full_width: w as f64, undeclared: None }).unwrap();
    let rest = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    let bare = engine.inputs(&negative, &rest, false).expect("readings");

    let mut masked = rest.clone();
    masked.flat_field = oni_settings::frame::FlatField { mask: FlatFieldMask::Custom, custom: Some("panel".into()), ..Default::default() };
    let divided = engine.inputs(&negative, &masked, false).expect("readings");
    assert_ne!(divided.levels, bare.levels, "the mask reaches the levels' input");
    assert_ne!(divided.curves, bare.curves, "the mask reaches the curves' input");
    // Twin: a mask whose file is missing lays nothing, so both read the bare frame.
    masked.flat_field.custom = Some("gone".into());
    let gone = engine.inputs(&negative, &masked, false).expect("readings");
    assert_eq!((&gone.levels, &gone.curves), (&bare.levels, &bare.curves));
    // The same id loaded afterwards moves the reading the same settings kept.
    std::fs::copy(masks.join("panel.tiff"), masks.join("gone.tiff")).unwrap();
    let found = engine.inputs(&negative, &masked, false).expect("readings");
    assert_eq!(found.levels, divided.levels, "the mask read later is laid");

    let id = Id::parse("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B").unwrap();
    let mut dusted = rest.clone();
    dusted.corrections = vec![oni_settings::frame::Correction { id, stroke: vec![oni_settings::cg::Point::new(0.5, 0.5)], radius: 6.0 }];
    // Twin: a patch not generated yet lays nothing.
    let missing = engine.inputs(&negative, &dusted, false).expect("readings");
    assert_eq!((&missing.levels, &missing.curves), (&bare.levels, &bare.curves));
    let bright: Vec<f32> = (0..w * h).flat_map(|_| [0.9f32, 0.9, 0.9, 1.0]).collect();
    tiff(&crate::patches::file(&caches, id), w as u32, h as u32, &bright);
    let healed = engine.inputs(&negative, &dusted, false).expect("readings");
    assert_ne!(healed.levels, bare.levels, "the patch reaches the levels' input");
    assert_ne!(healed.curves, bare.curves, "the patch reaches the curves' input");
    let _ = std::fs::remove_dir_all(&root);
}

/// On the Scanner face the curves' plot reads what enters its curves, the look and the keypad laid;
/// the levels' stays Pro's windows, as the balance reads them, and no reading moves a render.
#[test]
fn the_scanner_curves_read_its_look_while_the_levels_stay_pro_s_and_no_render_moves() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the Scanner face's input histograms are not exercised");
        return;
    };
    let mut engine = Engine::on(&device);
    let (w, h) = (300usize, 200usize);
    let pixels = (0..w * h).map(|i| { let v = 0.02 + 0.6 * (i % w) as f32 / w as f32; [v, v * 0.7, v * 0.5, 1.0] }).collect();
    let negative = engine.prepare(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: w as f64, undeclared: None }).unwrap();
    let pro = PipelineSettings::rest(oni_settings::ConversionMode::Negative, true);
    let scanner = PipelineSettings { face: oni_settings::Face::Scanner, ..pro.clone() };
    let shown = |s: &PipelineSettings, e: &mut Engine| e.render(&negative, s, None).unwrap().pixels;
    let (pro_shown, scanner_shown) = (shown(&pro, &mut engine), shown(&scanner, &mut engine));
    let (on_pro, on_scanner) = (engine.inputs(&negative, &pro, false).expect("readings"), engine.inputs(&negative, &scanner, false).expect("readings"));
    assert_ne!(on_scanner.curves, on_pro.curves, "the Scanner's look moves what enters its curves");
    // Twin: the levels read Pro's windows on both faces.
    assert_eq!(on_scanner.levels, on_pro.levels);
    // The keypad lies in front of the curves; the face's own curves follow what they read.
    let mut keyed = scanner.clone();
    keyed.scanner = oni_settings::scanner::ScannerClicks::new(4, 0, 0, 0, 0);
    assert_ne!(engine.inputs(&negative, &keyed, false).expect("readings").curves, on_scanner.curves, "a density step reaches the curves' input");
    let mut curved = scanner.clone();
    curved.scanner_finish.manual_curves.linked.points = vec![oni_settings::cg::Point::new(0.0, 0.0), oni_settings::cg::Point::new(0.3, 0.8), oni_settings::cg::Point::new(1.0, 1.0)];
    assert_eq!(engine.inputs(&negative, &curved, false).expect("readings").curves, on_scanner.curves, "the curves' input is read in front of them");
    // The readings, through either face, leave both renders as they were.
    assert_eq!((shown(&pro, &mut engine), shown(&scanner, &mut engine)), (pro_shown, scanner_shown.clone()));
    // Twin: the same comparison sees a render that moved.
    assert_ne!(shown(&curved, &mut engine), scanner_shown);
}

/// A render counts its whole picture only for a plot on screen; one counted for nothing is counted
/// once a plot shows, the same settings or not.
#[test]
fn a_render_counts_its_picture_only_for_a_plot_on_screen() {
    let Ok(device) = Device::open() else {
        eprintln!("no adapter: the count is not exercised");
        return;
    };
    let engine = Engine::shared(&device);
    let linear = Linear { width: 8, height: 8, pixels: vec![[0.3, 0.2, 0.1, 1.0]; 64] };
    let negative = Arc::new(engine.prepare(Decoded { linear, full_width: 8.0, undeclared: None }).unwrap());
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = Worker::new(Box::new(engine), move |d| match d {
        Delivery::Render(_) => tx.send("render").unwrap(),
        Delivery::Counted { .. } => tx.send("counted").unwrap(),
        Delivery::Inputs { .. } => {}
    });
    let job = |revision: u64, counted: bool| {
        let (kind, asked) = (crate::timing::Kind::Notch, Instant::now());
        Job { revision, path: PathBuf::new(), negative: negative.clone(), settings: PipelineSettings::default(), kind, asked, view: None, reference: None, held: false, counted }
    };
    let landed = || std::iter::from_fn(|| rx.recv_timeout(Duration::from_secs(10)).ok()).take_while(|d| *d != "render").count();
    let drained = || std::iter::from_fn(|| rx.recv_timeout(Duration::from_millis(500)).ok()).collect::<Vec<_>>();
    worker.submit(job(1, false));
    assert_eq!((landed(), drained()), (0, vec![]), "rendered, counted for nothing on screen");
    // Twin: the same picture asked with a plot on screen is counted.
    worker.submit(job(2, true));
    assert_eq!((landed(), drained()), (0, vec!["counted"]));
}
