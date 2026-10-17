//! The decode must stay linear: one stop more doubles every channel's mean.

use std::path::PathBuf;

/// The test roll: `ONI_RAW_DIR`, else roll #8 as staged in the cloud workspace.
fn frames() -> Vec<PathBuf> {
    let dir = std::env::var("ONI_RAW_DIR").unwrap_or_else(|_| "/mnt/user-data/uploads/Downloads/Roll #8".into());
    let mut out: Vec<_> = std::fs::read_dir(&dir)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    out.retain(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("NEF" | "RW2" | "CR3" | "ARW" | "DNG")));
    out.sort();
    out
}

fn means(px: &[[f32; 4]]) -> [f64; 3] {
    let mut s = [0f64; 3];
    for p in px {
        for c in 0..3 {
            s[c] += p[c] as f64;
        }
    }
    s.map(|v| v / px.len() as f64)
}

#[test]
fn one_stop_doubles_every_channel() {
    let files = frames();
    assert!(!files.is_empty(), "no RAW found; set ONI_RAW_DIR");
    for f in files.iter().take(3) {
        let base = oni_decode::decode(f, 1.0).expect("decode");
        let up = oni_decode::decode(f, 2.0).expect("decode +1 EV");
        let (a, b) = (means(&base.pixels), means(&up.pixels));
        let ratio = [b[0] / a[0], b[1] / a[1], b[2] / a[2]];
        let peak = base.pixels.iter().flat_map(|p| p[..3].to_vec()).fold(0f32, f32::max);
        println!("{}: {}x{} mean {:.5?} peak {peak:.4} ratio {:.4?}", f.file_name().unwrap().to_string_lossy(), base.width, base.height, a, ratio);
        // The bound of the Swift check: |ratio - 2| <= 2 * 0.01 on every channel.
        for r in ratio {
            assert!((r - 2.0).abs() <= 0.02, "not linear: {ratio:?}");
        }
    }
}
