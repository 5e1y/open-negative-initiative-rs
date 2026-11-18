//! The As Shot white read off real RAWs: their metadata alone, no pixel decoded.
mod common;

use std::path::Path;

#[test]
fn roll_8s_as_shot_reads_as_daylight() {
    let Some(frames) = common::frames("roll_8s_as_shot_reads_as_daylight", 2) else { return };
    for path in frames.iter().take(2) {
        let shot = oni_decode::as_shot(path).expect("a Z f names its As Shot multipliers");
        println!("{}: {shot:?}", path.file_name().unwrap().to_string_lossy());
        assert!((4000.0..7500.0).contains(&shot.kelvin) && shot.tint.abs() < 30.0, "{shot:?}");
    }
}

#[test]
fn a_file_with_no_mosaic_names_no_as_shot() {
    let dir = std::env::temp_dir().join(format!("oni-as-shot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let jpeg = dir.join("plain.png");
    image::RgbImage::from_pixel(4, 4, image::Rgb([128, 128, 128])).save(&jpeg).unwrap();
    assert_eq!(oni_decode::as_shot(&jpeg), None);
    assert_eq!(oni_decode::as_shot(Path::new("/nonexistent/frame.NEF")), None);
    let _ = std::fs::remove_dir_all(&dir);
}
