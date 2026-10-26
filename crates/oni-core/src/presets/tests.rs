use super::*;

fn folder(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-core-presets-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn sample() -> Preset {
    let mut settings = PipelineSettings::default();
    settings.sharpen.amount = 1.5;
    settings.geometry.angle = 3.0;
    Preset::new("Portra / soft", BTreeSet::from(["sharpen".to_string()]), Tint::Moss, settings)
}

#[test]
fn a_preset_round_trips_through_its_file_and_a_rename_leaves_one_file() {
    let dir = folder("round");
    let mut preset = sample();
    save(&preset, &dir).unwrap();
    assert_eq!(load(&dir), vec![preset.clone()]);
    preset.name = "Renamed".into();
    save(&preset, &dir).unwrap();
    assert_eq!(files(&dir).len(), 1);
    assert_eq!(load(&dir)[0].name, "Renamed");
    // Twin: a duplicate is a second file, with an id of its own.
    save(&preset.duplicated(), &dir).unwrap();
    assert_eq!(files(&dir).len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_preset_carries_its_rows_and_never_the_framing() {
    let preset = sample();
    let target = PipelineSettings::default();
    let laid = preset.apply(&target);
    assert_eq!((laid.sharpen.amount, laid.geometry.angle), (1.5, 0.0));
    // Twin: with no row ticked nothing moves.
    let empty = Preset { parameters: BTreeSet::new(), ..preset };
    assert_eq!(empty.apply(&target), target);
}

#[test]
fn an_old_preset_reads_its_groups_and_its_retired_rows() {
    let mut doc: Value = serde_json::from_str(&codec::encode_settings(&PipelineSettings::default()).unwrap()).unwrap();
    doc["name"] = "Old".into();
    doc["groups"] = serde_json::json!(["detail", "nonsense"]);
    let read = decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert_eq!(read.parameters, BTreeSet::from(["chroma".to_string(), "sharpen".to_string()]));
    doc["parameters"] = serde_json::json!(["fade", "saturation.shadows", "autoBalance.classic"]);
    let read = decode(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert_eq!(read.parameters, BTreeSet::from(["autoBalance.filters".to_string(), "saturation".to_string()]));
    // Twin: a blank name is no preset.
    doc["name"] = "  ".into();
    assert!(decode(&serde_json::to_vec(&doc).unwrap()).is_none());
}

#[test]
fn names_and_files_are_formed_as_the_shelf_expects() {
    assert_eq!(default_name(&["Preset 01", " preset 02 "]), "Preset 03");
    assert_eq!(default_name(&["Preset 02"]), "Preset 01");
    let p = sample();
    assert!(file_name(&p).starts_with("Portra-soft-"));
    assert_eq!(p.abbreviation(), "POR");
    let mark = |n: &str| Preset { name: n.into(), ..sample() }.abbreviation();
    assert_eq!([mark("Ektar"), mark("b&w"), mark("x"), mark("  ")], ["EKT", "BW", "X", "  "]);
    // Twin: the mark follows the name, so two names give two marks.
    assert_ne!(mark("Portra 400"), mark("Ektar"));
    assert_eq!(default_name(&[]), "Preset 01");
    assert_eq!(default_name(&["Preset 01", "Preset 03"]), "Preset 02", "the gap, not the highest plus one");
    let ninety_nine: Vec<String> = (1..=99).map(|n| format!("Preset {n:02}")).collect();
    let names: Vec<&str> = ninety_nine.iter().map(String::as_str).collect();
    assert_eq!(default_name(&names), "Preset 100");
    assert_eq!(default_name(&["Portra 400", "Ektar"]), "Preset 01");
    let hostile = Preset { name: "../../Kodak: 400/800".into(), ..sample() };
    let file = file_name(&hostile);
    assert!(!file.contains('/') && !file.contains(':') && !file.starts_with('.'), "{file}");
    let long = Preset { name: "x".repeat(300), ..sample() };
    assert_eq!(file_name(&long).len(), 64 + 1 + 8 + 5, "the stem stops at 64");
}

/// `PresetTint.selfCheck`: every card's text is readable.
#[test]
fn every_tint_carries_a_readable_ink() {
    let worst = Tint::ALL.iter().map(|t| t.contrast()).fold(f64::MAX, f64::min);
    assert!(worst >= 4.5, "the worst tint reads at {worst:.2}:1");
    let inks: BTreeSet<bool> = Tint::ALL.iter().map(|t| t.wants_dark_ink()).collect();
    assert_eq!(inks.len(), 2, "twin: both inks serve, so a constant ink would fail");
    assert_eq!(Tint::from_raw("chartreuse"), None);
    assert_eq!((Tint::default(), Tint::Slate.label()), (Tint::Slate, "Slate".to_string()));
}

#[test]
fn a_share_code_is_one_line_and_reads_back_as_the_same_look() {
    let mut settings = PipelineSettings::default();
    settings.gains.stops = [0.3, -0.1, 0.2];
    settings.sharpen.amount = 1.25;
    settings.geometry.angle = 4.0;
    let looked = Preset {
        note: "Daylight, scanned on the light table".into(),
        ..Preset::new("Portra 400", ["gains", "sharpen", "saturation"].iter().map(|s| s.to_string()).collect(), Tint::Rust, settings)
    };
    let code = share_code(&looked).unwrap();
    assert!(code.starts_with(SHARE_CODE_PREFIX) && !code.chars().any(char::is_whitespace), "{code}");
    let back = from_share_code(&code).unwrap();
    assert!(back.same_look(&looked));
    assert_eq!(back.settings.geometry.angle, 0.0, "the code carries only the applied rows");
    let wrapped: String = code.chars().enumerate().map(|(i, c)| if i % 60 == 59 { format!("{c}\n ") } else { c.to_string() }).collect();
    assert!(from_share_code(&format!("  {wrapped}\n")).is_some_and(|p| p.same_look(&looked)));
    assert!(from_share_code(&code[..code.len() - 12]).is_none(), "a truncated code is refused whole");
    // Twin: one character of the payload changed is refused, not read as another look.
    let mut edited: Vec<char> = code.chars().collect();
    let middle = edited.len() / 2;
    edited[middle] = if edited[middle] == 'A' { 'B' } else { 'A' };
    assert!(from_share_code(&edited.into_iter().collect::<String>()).is_none());
    assert!(from_share_code("hello").is_none());
    let dir = folder("code");
    save(&looked, &dir).unwrap();
    assert!(adopting(vec![back], &dir).is_empty(), "one's own code lays nothing");
    assert_eq!(load(&dir).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A code written by the Swift app: raw deflate, sorted keys, FNV-1a, base64url.
#[test]
fn a_code_built_as_the_swift_app_builds_it_reads_back() {
    use base64::Engine;
    use std::io::Write;
    let json = br#"{"preset":{"id":"3676F1B6-B28F-4B26-9125-71C8039CD81B","name":"Azure","note":"","parameters":["sharpen"],"tint":"teal"},"settings":{"sharpen":{"amount":1.5}},"version":19}"#;
    let mut deflater = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    deflater.write_all(json).unwrap();
    let mut packed = deflater.finish().unwrap();
    packed.extend_from_slice(&checksum(json));
    let body = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&packed);
    let read = from_share_code(&format!("ONI1:{body}")).unwrap();
    assert_eq!((read.name.as_str(), read.tint, read.settings.sharpen.amount), ("Azure", Tint::Teal, 1.5));
    assert_eq!(read.settings.sharpen.radius, PipelineSettings::default().sharpen.radius, "the codec restores dropped keys");
    // Twin: the same body under a zlib header is no Swift code.
    let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    zlib.write_all(json).unwrap();
    let mut wrapped = zlib.finish().unwrap();
    wrapped.extend_from_slice(&checksum(json));
    assert!(from_share_code(&format!("ONI1:{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&wrapped))).is_none());
}

/// A code whose JSON inflates past the cap is refused before it is held whole; one under it reads.
#[test]
fn a_code_inflating_past_the_cap_is_refused_and_one_under_it_reads() {
    use base64::Engine;
    use std::io::Write;
    let code = |note_len: usize| {
        let json = format!(
            r#"{{"preset":{{"id":"3676F1B6-B28F-4B26-9125-71C8039CD81B","name":"Azure","note":"{}","parameters":["sharpen"],"tint":"teal"}},"settings":{{"sharpen":{{"amount":1.5}}}},"version":19}}"#,
            " ".repeat(note_len)
        );
        let mut deflater = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        deflater.write_all(json.as_bytes()).unwrap();
        let mut packed = deflater.finish().unwrap();
        packed.extend_from_slice(&checksum(json.as_bytes()));
        format!("ONI1:{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&packed))
    };
    let cap = MAX_SHARE_JSON as usize;
    let bomb = code(cap);
    assert!(bomb.len() < 10_000, "a short paste inflating past a megabyte");
    assert!(from_share_code(&bomb).is_none());
    // Twin: a JSON just under the cap is a well-formed code and reads back.
    assert!(from_share_code(&code(cap - 1000)).is_some_and(|p| p.name == "Azure"));
}

#[test]
fn importing_never_overwrites_a_held_preset() {
    let dir = folder("import");
    let outside = folder("import-mail");
    std::fs::create_dir_all(&outside).unwrap();
    let held = Preset::new("Gold 200", BTreeSet::from(["gains".to_string()]), Tint::Amber, PipelineSettings::default());
    save(&held, &dir).unwrap();
    let sent = outside.join("gold.json");
    std::fs::write(&sent, shareable(&held).unwrap()).unwrap();
    assert!(importing(&[sent], &dir).is_empty() && load(&dir).len() == 1, "a held preset lays nothing");
    let mut retuned = held.clone();
    retuned.settings.gains.stops[0] += 0.5;
    let clash = outside.join("gold-retuned.json");
    std::fs::write(&clash, shareable(&retuned).unwrap()).unwrap();
    let garbage = outside.join("notes.json");
    std::fs::write(&garbage, br#"{"hello": 1}"#).unwrap();
    let taken = importing(&[clash, garbage], &dir);
    let after = load(&dir);
    assert_eq!((taken.len(), after.len()), (1, 2));
    assert!(after.contains(&held) && taken[0].id != held.id && taken[0].settings == retuned.settings);
    // Twin: saving the clashing file as it came replaces the held one.
    let naive = folder("import-naive");
    save(&held, &naive).unwrap();
    save(&retuned, &naive).unwrap();
    assert!(load(&naive).len() == 1 && !load(&naive).contains(&held));
    let copy = held.duplicated();
    assert!(copy.id != held.id && copy.name == "Gold 200 copy" && copy.settings == held.settings && copy.tint == held.tint);
    for d in [dir, outside, naive] {
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn untouched_shipped_presets_are_taken_back_once() {
    let dir = folder("retire");
    let shipped = retired();
    assert_eq!(shipped.len(), RETIRED.len(), "every retired file is a readable preset");
    for p in &shipped {
        save(p, &dir).unwrap();
    }
    let mut renamed = shipped[0].clone();
    renamed.name += " mine";
    save(&renamed, &dir).unwrap();
    let own = Preset::new("Own", BTreeSet::from(["gains".to_string()]), Tint::Slate, PipelineSettings::default());
    save(&own, &dir).unwrap();
    let mut trashed = Vec::new();
    let gone = retire_shipped(&shipped, &dir, false, &mut |f| {
        trashed.push(f.to_path_buf());
        std::fs::remove_file(f)
    });
    let left = load(&dir);
    assert_eq!((gone.len(), left.len(), trashed.len()), (shipped.len() - 1, 2, shipped.len() - 1));
    assert!(left.iter().any(|p| p.name == renamed.name) && left.iter().any(|p| p.id == own.id));
    // Twin: once recorded, the pass leaves a preset laid back afterwards alone.
    for p in &shipped {
        save(p, &dir).unwrap();
    }
    assert!(retire_shipped(&shipped, &dir, true, &mut |f| std::fs::remove_file(f)).is_empty());
    assert!(!retire_shipped(&shipped, &dir, false, &mut |f| std::fs::remove_file(f)).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_single_file_list_is_taken_over_once_then_archived() {
    let support = folder("migrate");
    std::fs::create_dir_all(&support).unwrap();
    let mut element: Value = serde_json::from_str(&codec::encode_settings(&PipelineSettings::default()).unwrap()).unwrap();
    element["name"] = "Old one".into();
    element["groups"] = serde_json::json!(["detail"]);
    std::fs::write(legacy_file(&support), serde_json::to_vec(&serde_json::json!([element, {"nothing": 1}])).unwrap()).unwrap();
    let shelf = load_shelf(&support);
    assert_eq!(shelf.len(), 1);
    assert_eq!(shelf[0].parameters, BTreeSet::from(["chroma".to_string(), "sharpen".to_string()]));
    assert!(!legacy_file(&support).exists() && support.join("presets.json.migrated").exists());
    // Twin: a second load finds no list and keeps the one file.
    assert_eq!(load_shelf(&support).len(), 1);
    assert_eq!(files(&directory(&support)).len(), 1);
    let _ = std::fs::remove_dir_all(&support);
}

#[test]
fn a_blank_name_keeps_the_one_on_disk() {
    let dir = folder("blank");
    let mut p = sample();
    save(&p, &dir).unwrap();
    p.name = "   ".into();
    assert_eq!(writable_name(&p, &dir), "Portra / soft");
    save(&p, &dir).unwrap();
    assert_eq!(load(&dir)[0].name, "Portra / soft");
    // Twin: never on disk, a blank name falls back to the stem.
    let fresh = Preset { name: " ".into(), ..Preset::new("x", BTreeSet::new(), Tint::Slate, PipelineSettings::default()) };
    assert_eq!(writable_name(&fresh, &dir), "Preset");
    let mut trashed = 0;
    delete(&load(&dir)[0], &dir, &mut |f| {
        trashed += 1;
        std::fs::remove_file(f)
    });
    assert_eq!((trashed, files(&dir).len()), (1, 0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_preview_is_what_a_click_writes() {
    let preset = sample();
    let target = PipelineSettings::default();
    assert_eq!(previewed(None, &target), target);
    assert_eq!(previewed(Some(&preset), &target), preset.apply(&target));
    // Twin: the preset does change the destination, so an ignored preview fails above.
    assert_ne!(previewed(Some(&preset), &target), target);
}
