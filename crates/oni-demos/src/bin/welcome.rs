//! `welcome interface OUT [--language en|fr|es] [--before DIR] [--strip PNG]`: the interface film of `ONI_WELCOME_PHOTO`,
//! its GIF, strip and stills; `--before` lays the 1.5.6 frames ahead, `--strip` is their photograph, every frame's own.

use std::path::{Path, PathBuf};

use oni_demos::encode::{self, Clip, Tuning};
use oni_demos::film::{self, Demo, FPS};
use oni_demos::photo::Rgba;
use oni_demos::welcome::{self, interface};
use oni_i18n::Language;

fn main() {
    if let Err(e) = run() {
        eprintln!("welcome: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let usage = "welcome interface OUT [--language en|fr|es] [--before DIR] [--strip PNG]";
    let what = args.next().ok_or(usage)?;
    let out = PathBuf::from(args.next().ok_or(usage)?);
    let (mut language, mut before, mut strip) = (Language::English, None, None);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} takes a value"));
        match arg.as_str() {
            "--language" => language = Language::from_key(&value()?).ok_or("--language en|fr|es")?,
            "--before" => before = Some(PathBuf::from(value()?)),
            "--strip" => strip = Some(welcome::read_png(&PathBuf::from(value()?))?),
            other => return Err(format!("unknown argument {other}; {usage}")),
        }
    }
    if what != "interface" {
        return Err(format!("no film {what}; {usage}"));
    }
    oni_i18n::set(language);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let name = |n: &str| out.join(format!("{}-{n}", language.key()));
    let mut demo = interface::Interface::new(welcome::photo()?);
    let (frames, photo) = film::shoot(&mut demo)?;
    for (at, still) in interface::still_frames(frames.len()) {
        let file = name(&format!("{still}.png"));
        frames[at].png(&file)?;
        println!("{}", file.display());
    }
    let (all, moments) = match before {
        Some(folder) => (welcome::joined(welcome::frames(&folder)?, frames)?, &interface::JOINED[..]),
        None => (frames, &interface::STRIP[..]),
    };
    if let Some(strip) = strip {
        let row = strip.width as usize * 4;
        for (n, frame) in all.iter().enumerate() {
            let held = (0..strip.height as usize).all(|y| {
                let at = ((photo.y as usize + y) * frame.width as usize + photo.x as usize) * 4;
                frame.pixels[at..at + row] == strip.pixels[y * row..(y + 1) * row]
            });
            if !held || (strip.width, strip.height) != (photo.width, photo.height) {
                return Err(format!("frame {n}'s photograph is not the before's: the wipe would break through it"));
            }
        }
    }
    let at = |s: &f32| ((s * FPS as f32) as usize).min(all.len() - 1);
    let shown: Vec<Rgba> = moments.iter().map(|s| all[at(s)].clone()).collect();
    let rows: Vec<Vec<Rgba>> = shown.chunks(6).map(<[Rgba]>::to_vec).collect();
    let sheet = name("interface-strip.png");
    oni_demos::sheet::write(&rows, &sheet)?;
    let seconds = all.len() as f32 / FPS as f32;
    let tuning: Tuning = demo.tuning();
    let gif = encode::gif(&Clip { frames: all, delay: (100 / FPS) as u16, photo }, tuning)?;
    let file = name("interface.gif");
    write(&file, &gif)?;
    println!("{}: {} KB, {seconds:.1} s; {}", file.display(), gif.len().div_ceil(1024), sheet.display());
    Ok(())
}

fn write(file: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(file, bytes).map_err(|e| format!("{}: {e}", file.display()))
}
