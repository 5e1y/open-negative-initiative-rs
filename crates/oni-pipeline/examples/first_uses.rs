//! `first_uses`: what the pipeline builds at its first use and never again, each timed in a fresh process: the stock
//! files read, every film's bake, every scanned stock's development, every paper's sheet, a slide's projection.

use oni_pipeline::{film, negative, paper};
use oni_settings::{DyeSet, PaperStock, VirtualStock};
use std::time::Instant;

fn timed<T>(what: &str, f: impl FnOnce() -> T) -> (T, f64) {
    let at = Instant::now();
    let out = f();
    let ms = at.elapsed().as_secs_f64() * 1e3;
    println!("{what:<48} {ms:>8.1} ms");
    (out, ms)
}

fn main() {
    timed("stock files read (the catalogue)", || oni_settings::stocks::current().films().len());
    // The neutral film and a look bake no profile of their own.
    let films: Vec<VirtualStock> = VirtualStock::all().into_iter().filter(|s| { let e = s.entry(); !e.neutral && e.look.is_none() }).collect();
    let (mut total, mut worst, mut bytes) = (0.0, (0.0, String::new()), 0usize);
    for s in &films {
        let at = Instant::now();
        let st = film::stock(*s);
        let ms = at.elapsed().as_secs_f64() * 1e3;
        bytes += st.table.len() * 16;
        total += ms;
        if ms > worst.0 {
            worst = (ms, format!("{s:?}"));
        }
    }
    println!("{:<48} {total:>8.1} ms ({} films, the slowest {} {:.1} ms, {:.1} MB)", "every film baked", films.len(), worst.1, worst.0, bytes as f64 / 1e6);
    timed("Portra 800 again (kept)", || film::stock(VirtualStock::default()).table.len());
    let sets = DyeSet::all();
    let (_, dev) = timed("every scanned stock's development", || sets.iter().map(|d| negative::development(negative::Film::Dyes(*d)).foot[0]).sum::<f32>());
    println!("{:<48} {:>8.1} ms each on average ({} stocks)", "", dev / sets.len() as f64, sets.len());
    timed("a black and white scan's development", || negative::development(negative::Film::Silver).foot[0]);
    timed("Ektachrome E100's projection", || film::projection::projected(VirtualStock::SLIDE).table.len());
    timed("Ektachrome E100's scans read back", || film::projection::scanned(VirtualStock::SLIDE).table.len());
    let papers = PaperStock::all();
    let (_, p) = timed("every paper's sheet", || papers.iter().map(|q| paper::sheet(*q).curve_low).sum::<f32>());
    println!("{:<48} {:>8.1} ms each on average ({} papers)", "", p / papers.len() as f64, papers.len());
    timed("the Frontier's cube", || oni_pipeline::frontier_look::CUBE.cube_dimension);
}
