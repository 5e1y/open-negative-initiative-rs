//! `decoder_fit <dumps>`: every `<name>.apple.f32` beside its `<name>.libraw.f32` (`blocks`'s grid), fitted
//! LibRaw to Apple by per-channel gains, then by a 3×3 matrix; residuals printed in density (log10).

use std::path::Path;

type Rgb = [f64; 3];

fn dump(path: &Path) -> (usize, usize, Vec<Rgb>) {
    let b = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let int = |i: usize| i32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
    let f: Vec<f64> = b[8..].chunks_exact(4).map(|c| f64::from(f32::from_le_bytes(c.try_into().unwrap()))).collect();
    (int(0), int(4), f.chunks_exact(4).map(|c| [c[0], c[1], c[2]]).collect())
}

/// Blocks off the frame's edge, above the floor and under the clip in both decodes.
fn pairs(apple: &Path, libraw: &Path) -> Vec<(Rgb, Rgb)> {
    let ((w, h, a), (lw, lh, l)) = (dump(apple), dump(libraw));
    let (cw, ch) = (w.min(lw), h.min(lh));
    let mut out = Vec::new();
    for y in 1..ch - 1 {
        for x in 1..cw - 1 {
            let (p, q) = (a[y * w + x], l[y * lw + x]);
            if p.iter().chain(&q).all(|v| *v > 2e-4 && *v < 0.9) {
                out.push((p, q));
            }
        }
    }
    out
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Each row minimising the relative error `(M q)_c / p_c - 1` by its 3×3 normal equations.
fn matrix(data: &[(Rgb, Rgb)]) -> [Rgb; 3] {
    [0, 1, 2].map(|c| {
        let (mut ata, mut atb) = ([[0f64; 3]; 3], [0f64; 3]);
        for (p, q) in data {
            let row = q.map(|v| v / p[c]);
            for i in 0..3 {
                atb[i] += row[i];
                for j in 0..3 {
                    ata[i][j] += row[i] * row[j];
                }
            }
        }
        solve(ata, atb)
    })
}

fn solve(a: [Rgb; 3], b: Rgb) -> Rgb {
    let det = |m: [Rgb; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    [0, 1, 2].map(|k| {
        let mut m = a;
        (0..3).for_each(|r| m[r][k] = b[r]);
        det(m) / d
    })
}

/// Density error per channel, model minus Apple: its mean and root mean square.
fn residual(data: &[(Rgb, Rgb)], model: impl Fn(Rgb) -> Rgb) -> (Rgb, Rgb) {
    let (mut mean, mut sq) = ([0f64; 3], [0f64; 3]);
    let n = data.len() as f64;
    for (p, q) in data {
        let m = model(*q);
        for c in 0..3 {
            let d = p[c].log10() - m[c].max(1e-9).log10();
            mean[c] += d / n;
            sq[c] += d * d / n;
        }
    }
    (mean, sq.map(f64::sqrt))
}

fn main() {
    let dir = std::env::args().nth(1).expect("dumps folder");
    let dir = Path::new(&dir);
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dumps")
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".apple.f32").map(str::to_owned))
        .filter(|n| dir.join(format!("{n}.libraw.f32")).is_file())
        .collect();
    names.sort();
    let mut all = Vec::new();
    println!("frame: Apple over LibRaw per channel, stops (median of blocks)");
    for n in &names {
        let data = pairs(&dir.join(format!("{n}.apple.f32")), &dir.join(format!("{n}.libraw.f32")));
        let g = [0, 1, 2].map(|c| median(data.iter().map(|(p, q)| (p[c] / q[c]).log2()).collect()));
        println!("  {n}: {:+.3} {:+.3} {:+.3} on {} blocks", g[0], g[1], g[2], data.len());
        all.extend(data);
    }
    let g = [0, 1, 2].map(|c| median(all.iter().map(|(p, q)| (p[c] / q[c]).log2()).collect()));
    let m = matrix(&all);
    println!("gains, stops: {:.4},{:.4},{:.4}", g[0], g[1], g[2]);
    let flat: Vec<String> = m.iter().flatten().map(|v| format!("{v:.5}")).collect();
    println!("matrix, rows: {}", flat.join(","));
    let apply = |q: Rgb| [0, 1, 2].map(|r| m[r][0] * q[0] + m[r][1] * q[1] + m[r][2] * q[2]);
    for (what, (mean, rms)) in [
        ("none", residual(&all, |q| q)),
        ("gains", residual(&all, |q| [0, 1, 2].map(|c| q[c] * g[c].exp2()))),
        ("matrix", residual(&all, apply)),
    ] {
        println!("  {what:7} density error mean {:+.4} {:+.4} {:+.4}, rms {:.4} {:.4} {:.4}", mean[0], mean[1], mean[2], rms[0], rms[1], rms[2]);
    }
    println!("residual after the matrix by Apple's density, stops (blocks):");
    for c in 0..3 {
        let mut row = format!("  {}", ["R", "G", "B"][c]);
        for lo in (2..32).step_by(2).map(|d| d as f64 / 10.0) {
            let r: Vec<f64> = all
                .iter()
                .filter(|(p, _)| (lo..lo + 0.2).contains(&-p[c].log10()))
                .map(|(p, q)| (p[c] / apply(*q)[c]).log2())
                .collect();
            if r.len() > 200 {
                row += &format!(" {lo:.1}:{:+.3}", median(r));
            }
        }
        println!("{row}");
    }
}
