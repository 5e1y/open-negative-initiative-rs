//! The demo tool's output is a function of its inputs: the same frames give the same bytes, and a
//! film of the demos' photograph shot twice gives the same GIF.

use oni_demos::demos::FilmType;
use oni_demos::encode::{self, Clip, Region, Tuning};
use oni_demos::film;
use oni_demos::photo::{self, Rgba};

const TUNING: Tuning = Tuning { tolerance: 10, colours: 64 };

/// A photograph-like gradient with a flat key moving across it.
fn frames(key_at: impl Fn(u32) -> u32) -> Vec<Rgba> {
    (0..8)
        .map(|n| {
            let (w, h) = (64u32, 48u32);
            let mut pixels = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h {
                for x in 0..w {
                    let key = y >= 30 && x >= key_at(n) && x < key_at(n) + 8;
                    let p = if key { [40, 40, 40, 255] } else if y < 28 { [(x * 4) as u8, (y * 5) as u8, 120, 255] } else { [30, 30, 30, 255] };
                    pixels.extend_from_slice(&p);
                }
            }
            Rgba { width: w, height: h, pixels }
        })
        .collect()
}

fn gif(frames: Vec<Rgba>) -> Vec<u8> {
    encode::gif(&Clip { frames, delay: 5, photo: Region { x: 0, y: 0, width: 64, height: 28 } }, TUNING).unwrap()
}

/// The pictures a player shows, each frame laid over the last.
fn played(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(bytes).unwrap();
    let width = usize::from(decoder.width());
    let mut picture = vec![0u8; width * usize::from(decoder.height()) * 4];
    let mut out = Vec::new();
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        let (left, top, w) = (usize::from(frame.left), usize::from(frame.top), usize::from(frame.width));
        for (y, row) in frame.buffer.chunks_exact(w * 4).enumerate() {
            for (x, p) in row.chunks_exact(4).enumerate() {
                if p[3] > 0 {
                    let at = ((top + y) * width + left + x) * 4;
                    picture[at..at + 4].copy_from_slice(p);
                }
            }
        }
        for _ in 0..frame.delay / 5 {
            out.push(picture.clone());
        }
    }
    out
}

#[test]
fn the_same_frames_give_the_same_bytes() {
    let a = gif(frames(|n| n * 6));
    assert_eq!(a, gif(frames(|n| n * 6)));
    // Twin: the key a pixel further on one frame gives another file.
    assert_ne!(a, gif(frames(|n| n * 6 + u32::from(n == 3))));
}

#[test]
fn the_interface_plays_back_exact_and_a_still_frame_only_lengthens_the_last() {
    let source = frames(|n| (n * 6).min(24));
    let shown = played(&gif(source.clone()));
    assert_eq!(shown.len(), source.len(), "every frame's time is kept");
    for (n, (s, p)) in source.iter().zip(&shown).enumerate() {
        // Below the photograph, flat colours only: each frame as drawn, to the bit.
        let below = 30 * 64 * 4;
        assert_eq!(s.pixels[below..], p[below..], "frame {n}'s interface");
    }
    // Twin: a frame shown a step late is seen.
    assert_ne!(source[2].pixels[30 * 64 * 4..], shown[1][30 * 64 * 4..]);
}

#[test]
fn a_film_of_the_photograph_shot_twice_is_the_same_gif() {
    if !photo::source().is_file() {
        return println!("SKIPPED: no demos' photograph at {}, the film is not exercised", photo::source().display());
    }
    if oni_app::render::Device::open().is_err() {
        return println!("SKIPPED: no GPU, the film is not exercised");
    }
    let (first, frames) = film::film(&mut FilmType::new()).unwrap();
    let (second, _) = film::film(&mut FilmType::new()).unwrap();
    assert!(first == second, "two films of one demo differ");
    // Twin: the same frames packed another way are other bytes, so the equality above saw the film.
    let other = encode::gif(&Clip { frames, delay: 5, photo: Region::default() }, TUNING).unwrap();
    assert_ne!(first, other);
}

#[test]
fn every_demo_filmed_is_one_the_app_plays() {
    let topics: Vec<&str> = oni_demos::demos::all().unwrap().iter().map(|d| d.topic()).collect();
    for topic in &topics {
        assert!(oni_app::demos::TOPICS.contains(topic) && oni_app::demos::clip(topic).is_some(), "{topic} is filmed but the app never plays it");
    }
    assert_eq!(topics.len(), oni_app::demos::TOPICS.len(), "every clip the app plays is filmed here");
    // Twin: a topic no demo films is not among them.
    assert!(!topics.contains(&"film-stock"));
}

#[test]
fn a_demo_showing_words_is_one_the_app_plays_per_language() {
    let worded: Vec<&str> = oni_demos::demos::all().unwrap().iter().filter(|d| d.worded()).map(|d| d.topic()).collect();
    assert_eq!(worded, oni_app::demos::WORDED, "the demos filmed per language are those the app plays per language");
    // Twin: a demo without words is filmed once.
    assert!(!worded.contains(&"lighting"));
}
