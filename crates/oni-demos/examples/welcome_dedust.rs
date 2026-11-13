//! `welcome_dedust OUT [--language L] [--sensitivity S] [--zoom Z] [--rehearsal 1]`: the De-dust film shot from the
//! photograph `ONI_WELCOME_DUST` names, its GIF, a strip of its frames and its three stills into OUT.

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
    let out = PathBuf::from(args.next().ok_or("welcome_dedust OUT | --probe DIR FILE…")?);
    // `--probe DIR FILE…`: each file's specks, and its ring stills at rest and at 50 % into DIR.
    if out.as_os_str() == "--probe" {
        let dir = PathBuf::from(args.next().ok_or("--probe DIR FILE…")?);
        for file in args {
            println!("{file}\n{}", dedust::probe(&PathBuf::from(&file), Some(&dir))?);
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
    crops(&shot, &out.join(format!("{code}-dedust-under-rings.png")))?;
    let pick = |k: usize| shot.frames[k * (shot.frames.len() - 1) / 11].clone();
    let rows = [(0..6).map(pick).collect(), (6..12).map(pick).collect()];
    oni_demos::sheet::write(&rows, &out.join(format!("{code}-dedust-strip.png")))?;
    Ok(())
}

/// Under each ring, a row at four times: the scan at 100 %, then the film's dusty and removed stills.
fn crops(shot: &dedust::Shot, out: &std::path::Path) -> Result<(), String> {
    use oni_demos::photo::{Rgba, magnified};
    use oni_demos::stage::SCALE;
    const HALF: u32 = 30;
    let scan = dedust::Scan::open(&dedust::source()?)?;
    let cut = |p: &Rgba, x: u32, y: u32| {
        let mut pixels = Vec::new();
        for row in y - HALF..y + HALF {
            let at = ((row * p.width + x - HALF) * 4) as usize;
            pixels.extend_from_slice(&p.pixels[at..at + (2 * HALF * 4) as usize]);
        }
        magnified(&Rgba { width: 2 * HALF, height: 2 * HALF, pixels }, 4, 8 * HALF, 8 * HALF)
    };
    let side = 8 * HALF;
    // Every speck is ringed; the strip shows those inside it.
    let (w, h) = (shot.region.width as f32 / SCALE, shot.region.height as f32 / SCALE);
    let rings: Vec<(f32, f32)> = shot.seen[shot.stills[1]].rings.iter().copied().filter(|&(x, y)| x > 0.0 && y > 0.0 && x < w && y < h).collect();
    let mut sheet = Rgba { width: 3 * side + 16, height: rings.len() as u32 * (side + 8), pixels: Vec::new() };
    sheet.pixels = vec![40; (sheet.width * sheet.height * 4) as usize];
    for (row, &(px, py)) in rings.iter().enumerate() {
        let k = SCALE / shot.view.zoom;
        let (fx, fy) = ((shot.view.x as f32 + px * k) as u32, (shot.view.y as f32 + py * k) as u32);
        let view = dedust::View { x: fx - HALF, y: fy - HALF, zoom: 1.0, width: 2 * HALF, height: 2 * HALF };
        let scan_crop = magnified(&scan.detail(&scan.settings, view)?, 4, side, side);
        let (sx, sy) = ((shot.region.x as f32 + px * SCALE) as u32, (shot.region.y as f32 + py * SCALE) as u32);
        let tiles = [scan_crop, cut(&shot.frames[shot.stills[0]], sx, sy), cut(&shot.frames[shot.stills[2]], sx, sy)];
        for (col, tile) in tiles.iter().enumerate() {
            let (x0, y0) = (col as u32 * (side + 8), row as u32 * (side + 8));
            for y in 0..side {
                let from = (y * side * 4) as usize;
                let to = (((y0 + y) * sheet.width + x0) * 4) as usize;
                sheet.pixels[to..to + (side * 4) as usize].copy_from_slice(&tile.pixels[from..from + (side * 4) as usize]);
            }
        }
    }
    sheet.png(out)
}
