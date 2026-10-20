//! `blocks <out> <block> <raw>...`: each RAW averaged over `block`-pixel squares into `<name>.libraw.f32`
//! (Int32 width, height, RGBA f32); `ONI_BLOCKS_AT=x,y` starts the grid where `CIRAWFilter`'s frame does.

use std::io::Write;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (out, block) = (Path::new(&args[0]), args[1].parse::<usize>().expect("block"));
    std::fs::create_dir_all(out).expect("out folder");
    let at: Vec<usize> = std::env::var("ONI_BLOCKS_AT").map_or(vec![0, 0], |v| v.split(',').map(|n| n.parse().expect("x,y")).collect());
    let (ox, oy) = (at[0], at[1]);
    for raw in &args[2..] {
        let d = oni_decode::decode(Path::new(raw), 1.0).expect("decode");
        let (w, h) = ((d.width - ox) / block, (d.height - oy) / block);
        let mut bytes = Vec::with_capacity(8 + w * h * 16);
        bytes.extend_from_slice(&(w as i32).to_le_bytes());
        bytes.extend_from_slice(&(h as i32).to_le_bytes());
        for y in 0..h {
            for x in 0..w {
                let mut s = [0f64; 3];
                for yy in oy + y * block..oy + (y + 1) * block {
                    let row = yy * d.width + ox;
                    for p in &d.pixels[row + x * block..row + (x + 1) * block] {
                        for c in 0..3 {
                            s[c] += p[c] as f64;
                        }
                    }
                }
                let n = (block * block) as f64;
                for v in [s[0] / n, s[1] / n, s[2] / n, 1.0] {
                    bytes.extend_from_slice(&(v as f32).to_le_bytes());
                }
            }
        }
        let name = Path::new(raw).file_name().expect("name").to_string_lossy();
        std::fs::File::create(out.join(format!("{name}.libraw.f32"))).expect("file").write_all(&bytes).expect("write");
        println!("{name}: {}x{} -> {w}x{h}", d.width, d.height);
    }
}
