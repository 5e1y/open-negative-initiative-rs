use super::*;
use std::cell::Cell;

fn written(support: &Path, id: &str, pixels: &[f32], width: u32, height: u32) {
    std::fs::create_dir_all(folder(support)).unwrap();
    let out = std::fs::File::create(file(support, id)).unwrap();
    let mut encoder = tiff::encoder::TiffEncoder::new(out).unwrap();
    encoder.write_image::<tiff::encoder::colortype::RGBA32Float>(width, height, pixels).unwrap();
}

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-flat-fields-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A panel falling off from 1 on the right to 0.25 on the left, as a decode would hand it.
fn ramp() -> Picture {
    let (w, h) = (64, 32);
    let pixels = (0..w * h).map(|i| { let v = 0.25 + 0.75 * (i % w) as f32 / (w - 1) as f32; [v, v, v, 1.0] }).collect();
    (w, h, pixels)
}

#[test]
fn a_mask_reads_with_its_peaks_and_its_passes_flag() {
    let support = scratch("read");
    let pixels: Vec<f32> = (0..4 * 3).flat_map(|i| [0.2 + i as f32 * 0.05, 0.9 - i as f32 * 0.01, 0.4, 1.0]).collect();
    written(&support, "abc", &pixels, 4, 3);
    std::fs::write(folder(&support).join("masks.json"), r#"[{"id":"abc","name":"Panel","perChannel":true},{"id":"def","name":"Other"}]"#).unwrap();
    let mask = read(&support, "abc").expect("the mask reads");
    assert_eq!((mask.width, mask.height, mask.pixels.len()), (4, 3, 12));
    assert!((mask.peaks[0] - 0.75).abs() < 1e-6 && (mask.peaks[1] - 0.9).abs() < 1e-6, "{:?}", mask.peaks);
    assert!(mask.per_channel);
    assert_eq!(entries(&support).len(), 2);
    // Twin: a mask the list does not mark as passes divides on its green, and a gone file reads none.
    written(&support, "def", &pixels, 4, 3);
    assert!(!read(&support, "def").unwrap().per_channel);
    assert!(read(&support, "gone").is_none());
    let _ = std::fs::remove_dir_all(&support);
}

#[test]
fn the_id_is_the_first_eight_bytes_of_the_contents_sha_256() {
    // SHA-256("abc") = ba7816bf8f01cfea 414140de…, the FIPS 180-2 example.
    assert_eq!(id_of(b"abc", None), "ba7816bf8f01cfea");
    let passes = RgbPasses { red: "R.NEF".into(), green: "G.NEF".into(), blue: "B.NEF".into(), registration: None };
    // Twin: the same bytes leading a triplet take another id, so the two masks never share one.
    assert_ne!(id_of(b"abc", Some(&passes)), id_of(b"abc", None));
    assert_eq!(id_of(b"abc", Some(&passes)).len(), 16);
}

/// `FlatField.selfCheck`'s loaded mask: it joins the list under its file's name, loading it
/// again adds nothing, and the TIFF holds the decode's falloff.
#[test]
fn a_loaded_picture_joins_the_list_under_its_files_name_once() {
    let support = scratch("load");
    let source = support.join("Scans/panel.tiff");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, b"the panel's own bytes").unwrap();
    let decodes = Cell::new(0);
    let decode = |_: &Path| {
        decodes.set(decodes.get() + 1);
        Some(ramp())
    };
    let entry = load(&support, &source, decode).expect("the panel loads");
    assert_eq!((entry.name.as_str(), entry.per_channel), ("panel", None));
    assert_eq!(entry.id, id_of(b"the panel's own bytes", None));
    assert_eq!(entries(&support), [entry.clone()]);
    let text = std::fs::read_to_string(folder(&support).join("masks.json")).unwrap();
    assert!(!text.contains("perChannel"), "an unset flag is left out, as JSONEncoder does: {text}");
    let again = load(&support, &source, |_| -> Option<Picture> { panic!("a known file is not decoded again") }).unwrap();
    assert_eq!((again, entries(&support).len(), decodes.get()), (entry.clone(), 1, 1));
    let (w, h, pixels) = read_tiff(&file(&support, &entry.id)).expect("the half-float TIFF reads");
    assert_eq!((w, h), (64, 32));
    let (dim, bright) = (pixels[16 * 64 + 2][1], pixels[16 * 64 + 61][1]);
    assert!((dim - 0.2738).abs() < 1e-3 && (bright - 0.9762).abs() < 1e-3, "{dim} {bright}");
    // Twin: a file no decoder takes, or no file at all, is undecodable and leaves the list alone.
    let other = support.join("Scans/noise.tiff");
    std::fs::write(&other, b"other bytes").unwrap();
    assert_eq!(load(&support, &other, |_| None), Err(Failure::Undecodable));
    assert_eq!(load(&support, &support.join("gone.tiff"), |_| Some(ramp())), Err(Failure::Undecodable));
    assert_eq!(entries(&support).len(), 1);
    let _ = std::fs::remove_dir_all(&support);
}

/// `RGBPasses.selfCheck`'s triplet: the lead taken as a mask is marked made of passes, under an
/// id of its own; the same file alone is a white-light mask.
#[test]
fn a_triplets_lead_is_a_mask_of_passes_under_an_id_of_its_own() {
    let support = scratch("passes");
    let lead = support.join("Scans/B.tif");
    std::fs::create_dir_all(lead.parent().unwrap()).unwrap();
    std::fs::write(&lead, b"lead bytes").unwrap();
    let alone = load(&support, &lead, |_| Some(ramp())).unwrap();
    let mut settings = oni_settings::PipelineSettings::default();
    settings.rgb_passes = Some(RgbPasses { red: "R.tif".into(), green: "G.tif".into(), blue: "B.tif".into(), registration: None });
    sidecar::write(&settings, &lead, Default::default()).expect("the lead's sidecar");
    assert_eq!(passes_named_by(&lead).map(|p| p.red), Some("R.tif".into()));
    let triplet = load(&support, &lead, |_| Some(ramp())).unwrap();
    assert_eq!((triplet.per_channel, alone.per_channel), (Some(true), None));
    assert_ne!(triplet.id, alone.id);
    assert!(read(&support, &triplet.id).unwrap().per_channel && !read(&support, &alone.id).unwrap().per_channel);
    let _ = std::fs::remove_dir_all(&support);
}

#[test]
fn the_mask_source_is_known_by_its_name_then_its_content() {
    let support = scratch("source");
    let source = support.join("Rig.NEF");
    std::fs::write(&source, b"rig bytes").unwrap();
    let entry = load(&support, &source, |_| Some(ramp())).unwrap();
    assert!(is_mask_source(&support, &source));
    // Twins: the same name over other bytes, and the same bytes under another name, are not it.
    let renamed = support.join("Other.NEF");
    std::fs::write(&renamed, b"rig bytes").unwrap();
    assert!(!is_mask_source(&support, &renamed));
    std::fs::write(&source, b"rewritten").unwrap();
    assert!(!is_mask_source(&support, &source));
    // A rename keeps the id; forgetting drops the entry and its file.
    rename(&support, &entry.id, "Easel");
    assert_eq!(entries(&support)[0], Entry { name: "Easel".into(), ..entry.clone() });
    forget(&support, &HashSet::from([entry.id.clone()]));
    assert!(entries(&support).is_empty() && read(&support, &entry.id).is_none());
    let _ = std::fs::remove_dir_all(&support);
}

/// `FlatField.selfCheck`'s remembered sliders.
#[test]
fn a_fresh_frame_opens_on_the_colour_last_dialled_and_a_dialled_one_keeps_its_own() {
    let support = scratch("remember");
    let mut preferences = Preferences::load(&support);
    assert_eq!(with_remembered(&FlatField::default(), remembered(&preferences)), FlatField::default());
    let dialled = FlatField { amount: 0.37, temperature: 0.61, tint: -0.23, ..FlatField::default() };
    remember(&mut preferences, &dialled).unwrap();
    let kept = remembered(&Preferences::load(&support));
    let fresh = FlatField { mask: oni_settings::frame::FlatFieldMask::ValoiEasy35V1, ..FlatField::default() };
    let opened = with_remembered(&fresh, kept);
    assert_eq!((opened.amount, opened.temperature, opened.tint, opened.mask), (1.0, 0.61, -0.23, fresh.mask));
    // Twin: a frame already dialled keeps its own sliders.
    let own = FlatField { amount: 0.5, ..FlatField::default() };
    assert_eq!(with_remembered(&own, kept), own);
    let _ = std::fs::remove_dir_all(&support);
}

thread_local! {
    /// Where the checks' Trash lays what it is handed, and each step a move took, in order.
    static TRASH: std::cell::RefCell<(PathBuf, Vec<String>)> = std::cell::RefCell::new((PathBuf::new(), Vec::new()));
}

fn step(name: String) {
    TRASH.with(|t| t.borrow_mut().1.push(name));
}

fn steps() -> Vec<String> {
    TRASH.with(|t| std::mem::take(&mut t.borrow_mut().1))
}

fn named(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

/// A Trash of the check's own, which the user's never sees.
fn trashed(path: &Path) -> std::io::Result<()> {
    step(format!("trash {}", named(path)));
    let bin = TRASH.with(|t| t.borrow().0.clone());
    std::fs::rename(path, bin.join(named(path)))
}

fn across(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices))
}

fn copied(from: &Path, to: &Path) -> std::io::Result<()> {
    step("copy".into());
    synced_copy(from, to)
}

fn short(from: &Path, to: &Path) -> std::io::Result<()> {
    let bytes = std::fs::read(from)?;
    std::fs::write(to, &bytes[..bytes.len() / 2])
}

fn flipped(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut bytes = std::fs::read(from)?;
    bytes[3] ^= 1;
    std::fs::write(to, &bytes)
}

/// A photo in `Scans` with its sidecar and index row, and the check's own Trash.
fn panel(root: &Path) -> (PathBuf, String, LibraryIndex) {
    let scans = root.join("Scans");
    std::fs::create_dir_all(&scans).unwrap();
    std::fs::create_dir_all(root.join("Trash")).unwrap();
    TRASH.with(|t| *t.borrow_mut() = (root.join("Trash"), Vec::new()));
    let photo = scans.join("Panel.NEF");
    std::fs::write(&photo, b"panel bytes").unwrap();
    sidecar::write(&oni_settings::PipelineSettings::default(), &photo, Default::default()).unwrap();
    let path = oni_library::paths::canonical(&photo);
    let index = LibraryIndex::open(&root.join("index.sqlite")).unwrap();
    index.upsert(&oni_library::index::Entry::new(&path, oni_settings::Date::now(), "f")).unwrap();
    (photo, path, index)
}

#[test]
fn a_photo_moved_out_trashes_its_sidecar_clears_its_row_and_takes_a_free_name() {
    let root = scratch("move-out");
    let presets = presets(&root);
    let (photo, path, index) = panel(&root);
    std::fs::create_dir_all(&presets).unwrap();
    std::fs::write(presets.join("Panel.NEF"), b"an earlier panel").unwrap();
    let same_volume = Moving::through(&trashed);
    let target = move_out(&path, &presets, Some(&index), &same_volume).expect("moved");
    assert_eq!(target, presets.join("Panel 2.NEF"));
    assert_eq!(std::fs::read(&target).unwrap(), b"panel bytes");
    assert!(!photo.exists() && !sidecar::url_for(&photo).exists() && index.all().is_empty());
    assert!(root.join("Trash").join(named(&sidecar::url_for(&photo))).exists(), "the settings can be taken back");
    assert_eq!(std::fs::read(presets.join("Panel.NEF")).unwrap(), b"an earlier panel", "the one already there stays");
    // Twin: a path with no file moves nothing and leaves the folder as it was.
    assert!(move_out(&path, &presets, Some(&index), &same_volume).is_none());
    assert_eq!(std::fs::read_dir(&presets).unwrap().count(), 2);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_move_across_volumes_syncs_checks_then_trashes_the_source() {
    let root = scratch("move-across");
    let presets = presets(&root);
    let (photo, path, index) = panel(&root);
    let target = move_out(&path, &presets, Some(&index), &Moving { rename: across, copy: copied, trash: &trashed }).expect("moved");
    assert_eq!(std::fs::read(&target).unwrap(), b"panel bytes");
    assert_eq!(steps(), ["copy".to_string(), "trash Panel.NEF".into(), format!("trash {}", named(&sidecar::url_for(&photo)))]);
    assert_eq!(std::fs::read(root.join("Trash/Panel.NEF")).unwrap(), b"panel bytes", "the original sits in the Trash");
    assert!(!photo.exists() && index.all().is_empty());
    // Twin: a short copy, or one of the right length with a byte changed, keeps the original and its sidecar.
    for wrong in [short as fn(&Path, &Path) -> std::io::Result<()>, flipped] {
        let root = scratch("move-across-wrong");
        let (photo, path, index) = panel(&root);
        let presets = super::presets(&root);
        assert!(move_out(&path, &presets, Some(&index), &Moving { rename: across, copy: wrong, trash: &trashed }).is_none());
        assert_eq!(std::fs::read(&photo).unwrap(), b"panel bytes");
        assert!(sidecar::url_for(&photo).exists() && index.all().len() == 1);
        assert_eq!(std::fs::read_dir(&presets).unwrap().count(), 0, "the bad copy is removed");
        assert!(steps().is_empty(), "nothing went to the Trash");
        let _ = std::fs::remove_dir_all(&root);
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// The masks the Swift app saved on this machine, read in place and never written.
#[test]
fn the_swift_apps_own_masks_read_whole() {
    let support = crate::timing::support();
    let found = entries(&support);
    if found.is_empty() {
        println!("SKIPPED: no custom flat-field in {}", support.display());
        return;
    }
    for e in &found {
        let Some(mask) = read(&support, &e.id) else { panic!("{}: {} does not read", e.id, e.name) };
        assert!(mask.width > 0 && mask.pixels.len() == (mask.width * mask.height) as usize);
        assert!(mask.peaks.iter().all(|p| p.is_finite() && *p > 0.0), "{}: {:?}", e.name, mask.peaks);
    }
}
