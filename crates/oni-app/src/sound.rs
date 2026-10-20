//! `KeycapSound`: the key click, its pitch wandering a few tens of cents per press so repeats read as a
//! struck object. Each press renders its own copy at the drawn speed and plays it over the last.

use std::cell::Cell;
use std::hash::{BuildHasher, RandomState};

/// `KeycapStyle.hardened`: the pitch's wander either way, and the loudness.
pub const PITCH_JITTER_CENTS: f64 = 120.0;
pub const VOLUME: f32 = 0.5;

const DOWN: &[u8] = include_bytes!("../resources/click-down.wav");
const UP: &[u8] = include_bytes!("../resources/click-up.wav");

/// A 16-bit PCM file's samples, interleaved, and its layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Pcm {
    pub channels: u16,
    pub rate: u32,
    pub samples: Vec<i16>,
}

/// The `fmt ` and `data` chunks of a RIFF WAVE, past any other; `None` unless 16-bit PCM.
pub fn read(wav: &[u8]) -> Option<Pcm> {
    if wav.get(..4)? != b"RIFF" || wav.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut at, mut format, mut samples) = (12, None, None);
    while at + 8 <= wav.len() {
        let size = u32::from_le_bytes(wav[at + 4..at + 8].try_into().ok()?) as usize;
        let body = wav.get(at + 8..(at + 8 + size).min(wav.len()))?;
        match &wav[at..at + 4] {
            b"fmt " if body.len() >= 16 => {
                let word = |i: usize| u16::from_le_bytes([body[i], body[i + 1]]);
                let pcm16 = word(0) == 1 && word(14) == 16;
                format = pcm16.then(|| (word(2), u32::from_le_bytes(body[4..8].try_into().unwrap())));
            }
            b"data" => samples = Some(body.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect::<Vec<_>>()),
            _ => {}
        }
        at += 8 + size + size % 2;
    }
    let (channels, rate) = format?;
    Some(Pcm { channels, rate, samples: samples? })
}

/// `AVAudioUnitVarispeed`: played `speed` times as fast, so pitch and length move together, each
/// channel read between its two nearest samples; then scaled by `volume`.
pub fn varied(pcm: &Pcm, speed: f64, volume: f32) -> Pcm {
    let channels = pcm.channels.max(1) as usize;
    let frames = pcm.samples.len() / channels;
    let out_frames = ((frames as f64) / speed).floor() as usize;
    let mut samples = Vec::with_capacity(out_frames * channels);
    for i in 0..out_frames {
        let at = i as f64 * speed;
        let (j, t) = (at as usize, (at.fract()) as f32);
        for c in 0..channels {
            let a = pcm.samples[j * channels + c] as f32;
            let b = pcm.samples.get((j + 1) * channels + c).map_or(a, |&s| s as f32);
            samples.push(((a + (b - a) * t) * volume).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16);
        }
    }
    Pcm { channels: pcm.channels, rate: pcm.rate, samples }
}

/// The samples as a RIFF WAVE again, for a player that takes a file in memory.
pub fn wav(pcm: &Pcm) -> Vec<u8> {
    let data = pcm.samples.len() as u32 * 2;
    let block = pcm.channels as u32 * 2;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&pcm.channels.to_le_bytes());
    out.extend_from_slice(&pcm.rate.to_le_bytes());
    out.extend_from_slice(&(pcm.rate * block).to_le_bytes());
    out.extend_from_slice(&(block as u16).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    pcm.samples.iter().for_each(|s| out.extend_from_slice(&s.to_le_bytes()));
    out
}

thread_local! {
    /// Drawn at the press and reused at the release, since two pitches would sound like two keys.
    static SPEED: Cell<f64> = const { Cell::new(1.0) };
    static DRAWS: Cell<u64> = const { Cell::new(0) };
}

/// A ratio, because pitch is geometric: an additive jitter would be audible at one pitch only.
fn drawn_speed() -> f64 {
    let draw = DRAWS.with(|d| {
        d.set(d.get() + 1);
        RandomState::new().hash_one(d.get())
    });
    let cents = (draw as f64 / u64::MAX as f64 * 2.0 - 1.0) * PITCH_JITTER_CENTS;
    2f64.powf(cents / 1200.0)
}

/// The key's press: a fresh pitch, then the down click.
pub fn press() {
    let speed = drawn_speed();
    SPEED.with(|s| s.set(speed));
    play(DOWN, speed);
}

/// The key's rise, at the press's pitch.
pub fn release() {
    play(UP, SPEED.with(|s| s.get()));
}

fn play(file: &[u8], speed: f64) {
    if let Some(pcm) = read(file) {
        out::play(wav(&varied(&pcm, speed, VOLUME)));
    }
}

#[cfg(target_os = "macos")]
mod out {
    use objc2::AllocAnyThread;
    use objc2::rc::Retained;
    use objc2_app_kit::NSSound;
    use objc2_foundation::NSData;
    use std::cell::RefCell;

    thread_local! {
        static LAST: RefCell<Option<Retained<NSSound>>> = const { RefCell::new(None) };
    }

    /// Stops the last click, as `.interrupts` does, so a fast hand never queues them.
    pub fn play(wav: Vec<u8>) {
        let data = NSData::with_bytes(&wav);
        let Some(sound) = NSSound::initWithData(NSSound::alloc(), &data) else { return };
        LAST.with(|last| {
            if let Some(previous) = last.borrow_mut().take() {
                previous.stop();
            }
            sound.play();
            *last.borrow_mut() = Some(sound);
        });
    }
}

#[cfg(windows)]
mod out {
    use std::cell::RefCell;
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

    thread_local! {
        /// The playing file, held until the next replaces it: the player reads it while it sounds.
        static PLAYING: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    /// A new sound stops the one playing, as `.interrupts` does.
    pub fn play(wav: Vec<u8>) {
        // SAFETY: SND_MEMORY reads a whole WAVE from the pointer, kept alive in `PLAYING` until the
        // next call, which stops this sound before the buffer is dropped.
        unsafe { PlaySoundW(wav.as_ptr().cast(), std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
        PLAYING.with(|p| *p.borrow_mut() = wav);
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod out {
    pub fn play(_: Vec<u8>) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_clicks_read_and_write_back_unchanged() {
        let down = read(DOWN).expect("the down click is 16-bit PCM");
        assert_eq!((down.channels, down.rate), (2, 48_000));
        assert_eq!(down.samples.len(), 995 * 2, "20.7 ms at 48 kHz, past the JUNK chunk");
        assert!(read(UP).is_some_and(|up| up.samples.len() > down.samples.len()));
        assert_eq!(read(&wav(&down)), Some(down.clone()));
        assert_eq!(varied(&down, 1.0, 1.0), down, "at rest the copy is the file");
        // Twin: a file that is no WAVE reads as nothing.
        assert_eq!(read(&DOWN[4..]), None);
    }

    #[test]
    fn a_faster_click_is_shorter_and_reads_between_samples() {
        let ramp = Pcm { channels: 1, rate: 48_000, samples: vec![0, 100, 200, 300, 400] };
        let up = varied(&ramp, 1.25, 1.0);
        assert_eq!(up.samples, vec![0, 125, 250, 375]);
        assert_eq!(varied(&ramp, 1.0, 0.5).samples, vec![0, 50, 100, 150, 200]);
        // Twin: a copy that dropped the nearest sample instead of reading between would differ.
        assert_ne!(up.samples, vec![0, 100, 200, 300]);
        for _ in 0..200 {
            let speed = drawn_speed();
            let cents = 1200.0 * speed.log2();
            assert!(cents.abs() <= PITCH_JITTER_CENTS + 1e-9, "{cents} cents");
        }
    }
}
