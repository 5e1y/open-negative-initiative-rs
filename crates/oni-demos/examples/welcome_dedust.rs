//! `welcome_dedust OUT [--language L] [--sensitivity S] [--zoom Z] [--rehearsal 1]`: the De-dust film shot from the
//! negative `ONI_WELCOME_DUST` names, its GIF, a strip of its frames and its three stills into OUT.

use std::path::PathBuf;

use oni_demos::welcome::dedust::{self, Ask};
use oni_i18n::Language;

fn main() {
    if let Err(e) = run() {
        eprintln!("welcome_dedust: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("welcome_dedust OUT | --probe RAW…")?);
    if out.as_os_str() == "--probe" {
        for raw in args {
            println!("{raw}\n{}", dedust::probe(&PathBuf::from(&raw))?);
        }
        return Ok(());
    }
    let (mut ask, mut language) = (Ask::default(), Language::English);
    while let Some(arg) = args.next() {
        let value = args.next().ok_or(format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--language" => language = Language::ALL.into_iter().find(|l| l.code() == value).ok_or(format!("no language {value}"))?,
            "--sensitivity" => ask.sensitivity = Some(value.parse().map_err(|_| "--sensitivity 0 to 1")?),
            "--rehearsal" => ask.rehearsal = value == "1",
            "--zoom" => ask.zoom = Some(value.parse().map_err(|_| "--zoom 2, 1 or 0.5")?),
            other => return Err(format!("unknown {other}")),
        }
    }
    oni_i18n::set(language);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let shot = dedust::shoot(&dedust::source()?, ask)?;
    println!("{}", shot.report);
    if !shot.detected || !shot.removed {
        return Err("the hand's clicks never reached DETECT and REMOVE".into());
    }
    let code = if language == Language::English { "en" } else { language.code() };
    for (name, k) in ["dusty", "rings", "removed"].iter().zip(shot.stills) {
        shot.frames[k].png(&out.join(format!("{code}-dedust-{name}.png")))?;
    }
    let gif = dedust::gif(&shot)?;
    let file = out.join(format!("{code}-dedust.gif"));
    std::fs::write(&file, &gif).map_err(|e| e.to_string())?;
    let seconds = shot.frames.len() as f32 / oni_demos::film::FPS as f32;
    println!("{}: {} KB, {seconds:.1} s, {} frames", file.display(), gif.len().div_ceil(1024), shot.frames.len());
    let pick = |k: usize| shot.frames[k * (shot.frames.len() - 1) / 11].clone();
    let rows = [(0..6).map(pick).collect(), (6..12).map(pick).collect()];
    oni_demos::sheet::write(&rows, &out.join(format!("{code}-dedust-strip.png")))?;
    Ok(())
}
