//! `oni-demos [--out DIR] [--sheet FILE] [TOPIC…]`: films every demo, or those named, from the demos'
//! photograph into the app's resources, with each GIF's weight; `--sheet` lays a few frames in a PNG.

use std::path::PathBuf;

use oni_demos::film::{self, FPS};
use oni_demos::{demos, photo, resources, sheet};
use oni_i18n::Language;

fn main() {
    if let Err(e) = run() {
        eprintln!("oni-demos: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (mut out, mut sheet_file, mut topics) = (resources(), None, Vec::new());
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = PathBuf::from(args.next().ok_or("--out DIR")?),
            "--sheet" => sheet_file = Some(PathBuf::from(args.next().ok_or("--sheet FILE")?)),
            topic => topics.push(topic.to_string()),
        }
    }
    let source = photo::source();
    if !source.is_file() {
        return Err(format!("the demos' photograph {} is missing: mount it or point ONI_DEMO_PHOTO at it", source.display()));
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut clips = Vec::new();
    // A demo showing words is filmed once per language, the tabs named from the app's catalogues.
    for language in Language::ALL {
        oni_i18n::set(language);
        for mut demo in demos::all()? {
            let wanted = topics.is_empty() || topics.iter().any(|t| t == demo.topic());
            if !wanted || (language != Language::English && !demo.worded()) {
                continue;
            }
            let (gif, frames) = film::film(demo.as_mut())?;
            let file = out.join(oni_app::demos::file(demo.topic(), language));
            std::fs::write(&file, &gif).map_err(|e| format!("{}: {e}", file.display()))?;
            let seconds = frames.len() as f32 / FPS as f32;
            println!("{}: {} KB, {seconds:.1} s, {} frames", file.display(), gif.len().div_ceil(1024), frames.len());
            if language == Language::English {
                clips.push(frames);
            }
        }
    }
    if let Some(file) = sheet_file {
        sheet::write(&clips, &file)?;
        println!("{}", file.display());
    }
    Ok(())
}
