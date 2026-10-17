//! `dump <out folder> <side> <raw>...`: each RAW decoded and box-reduced to `side` on its long edge,
//! written as the Swift app's RenderDump does (Int32 width, Int32 height, RGBA f32 rows).

use std::io::Write;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (out, side) = (Path::new(&args[0]), args[1].parse::<usize>().expect("side"));
    std::fs::create_dir_all(out).expect("out folder");
    for (i, raw) in args[2..].iter().enumerate() {
        let d = oni_decode::decode(Path::new(raw), 1.0).expect("decode");
        let scale = d.width.max(d.height) as f64 / side as f64;
        let (w, h) = ((d.width as f64 / scale).round() as usize, (d.height as f64 / scale).round() as usize);
        let mut px = vec![[0f32; 4]; w * h];
        for y in 0..h {
            let (y0, y1) = ((y as f64 * scale) as usize, (((y + 1) as f64 * scale) as usize).min(d.height));
            for x in 0..w {
                let (x0, x1) = ((x as f64 * scale) as usize, (((x + 1) as f64 * scale) as usize).min(d.width));
                let mut s = [0f64; 3];
                for yy in y0..y1 {
                    for p in &d.pixels[yy * d.width + x0..yy * d.width + x1] {
                        for c in 0..3 {
                            s[c] += p[c] as f64;
                        }
                    }
                }
                let n = ((y1 - y0) * (x1 - x0)) as f64;
                px[y * w + x] = [(s[0] / n) as f32, (s[1] / n) as f32, (s[2] / n) as f32, 1.0];
            }
        }
        let mut f = std::fs::File::create(out.join(format!("{i:05}.f32"))).expect("file");
        f.write_all(&(w as i32).to_le_bytes()).unwrap();
        f.write_all(&(h as i32).to_le_bytes()).unwrap();
        f.write_all(bytemuck_cast(&px)).unwrap();
        println!("{raw}: {}x{} -> {w}x{h}", d.width, d.height);
    }
}

fn bytemuck_cast(px: &[[f32; 4]]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(px.as_ptr() as *const u8, std::mem::size_of_val(px)) }
}
