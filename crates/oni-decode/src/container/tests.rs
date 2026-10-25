//! The container's checks: the branch its bytes decide, a linear scan's read, the probe's reads.

use super::*;

/// A little-endian LinearRaw TIFF of `w` by `h`, 16-bit RGB in one strip, with an optional table.
fn tiff(w: usize, h: usize, samples: &[u16], table: Option<&[u16]>, cfa: bool, photometric: u16) -> Vec<u8> {
    let data_at = 8usize;
    let data: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    let table_at = data_at + data.len();
    let table_bytes: Vec<u8> = table.unwrap_or(&[]).iter().flat_map(|v| v.to_le_bytes()).collect();
    let ifd_at = table_at + table_bytes.len();
    let mut entries: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, w as u32),
        (257, 4, 1, h as u32),
        (258, 3, 1, 16),
        (259, 3, 1, 1),
        (262, 3, 1, u32::from(photometric)),
        (273, 4, 1, data_at as u32),
        (277, 3, 1, 3),
        (278, 4, 1, h as u32),
        (279, 4, 1, data.len() as u32),
        (50717, 4, 1, 1000),
        (50714, 4, 1, 100),
    ];
    if let Some(t) = table {
        entries.push((50712, 3, t.len() as u32, table_at as u32));
    }
    if cfa {
        entries.push((33422, 1, 4, 0x0201_0100));
    }
    entries.sort();
    let mut out = b"II".to_vec();
    out.extend(42u16.to_le_bytes());
    out.extend((ifd_at as u32).to_le_bytes());
    out.extend(data);
    out.extend(table_bytes);
    out.extend((entries.len() as u16).to_le_bytes());
    for (tag, t, n, v) in entries {
        out.extend(tag.to_le_bytes());
        out.extend(t.to_le_bytes());
        out.extend(n.to_le_bytes());
        if t == 3 && n == 1 {
            out.extend((v as u16).to_le_bytes());
            out.extend([0, 0]);
        } else {
            out.extend(v.to_le_bytes());
        }
    }
    out.extend(0u32.to_le_bytes());
    out
}

#[test]
fn a_linear_scan_reads_through_its_table_black_and_white() {
    let samples: Vec<u16> = (0..4 * 2 * 3).map(|i| (i % 4) as u16).collect();
    let table = [100u16, 325, 550, 1000];
    let bytes = tiff(4, 2, &samples, Some(&table), false, 34892);
    assert_eq!(kind(&bytes), Some(Kind::LinearRgb));
    let l = decode(&bytes, None).unwrap();
    assert_eq!((l.width, l.height), (4, 2));
    // Index 1 reads 325 on the table: (325 - 100) / (1000 - 100).
    assert!((l.pixels[0][1] - 0.25).abs() < 1e-6, "{:?}", l.pixels[0]);
    assert!(l.pixels[0][0].abs() < 1e-6 && (l.pixels[1][0] - 1.0).abs() < 1e-6, "{:?}", l.pixels[1]);
    // Twin: without the table the index itself is taken, under the black level.
    let bare = decode(&tiff(4, 2, &samples, None, false, 34892), None).unwrap();
    assert!(bare.pixels[0][1] < 0.0);
}

/// Kind and reads of the file through a window of `reach` bytes.
fn probed(at: &Path, reach: u64) -> (Option<Kind>, usize) {
    let file = File::open(at).unwrap();
    let len = file.metadata().unwrap().len();
    let r = reader(Bytes::File(RefCell::new(Window::new(file, len, reach)))).unwrap();
    let kind = classify(&r);
    let Bytes::File(w) = &r.bytes else { unreachable!() };
    (kind, w.borrow().fills)
}

#[test]
fn a_probe_reads_a_window_not_a_field_at_a_time() {
    let tif = std::env::temp_dir().join(format!("oni-probe-{}.tif", std::process::id()));
    let (w, h) = (256usize, 256usize);
    let table: Vec<u16> = (0..4096).collect();
    let bytes = tiff(w, h, &vec![7u16; w * h * 3], Some(&table), false, 34892);
    std::fs::write(&tif, &bytes).unwrap();
    let (found, fills) = probed(&tif, WINDOW);
    assert_eq!(found, kind(&bytes));
    assert_eq!(found, Some(Kind::LinearRgb));
    assert!(fills <= 3, "{fills} reads");
    // Twin: a window as small as a field reads once a field, the 4096 entries of the table each.
    let (found, fills) = probed(&tif, 2);
    assert_eq!(found, Some(Kind::LinearRgb));
    assert!(fills > 4096, "{fills} reads");
    let _ = std::fs::remove_file(&tif);
}

/// A LinearRaw TIFF `w` wide in one column of tiles `tile_w` wide, padded with `pad`.
fn tiled(w: usize, h: usize, tile_w: usize, tile_h: usize, samples: &[u16], pad: u16) -> Vec<u8> {
    let tiles = h.div_ceil(tile_h);
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    for t in 0..tiles {
        offsets.push(8 + data.len());
        for y in t * tile_h..(t + 1) * tile_h {
            for x in 0..tile_w {
                for c in 0..3 {
                    let v = if x < w && y < h { samples[(y * w + x) * 3 + c] } else { pad };
                    data.extend(v.to_le_bytes());
                }
            }
        }
    }
    let offsets_at = 8 + data.len();
    let ifd_at = offsets_at + offsets.len() * 4;
    let entries: Vec<(u16, u16, u32, u32)> = vec![
        (256, 4, 1, w as u32),
        (257, 4, 1, h as u32),
        (258, 3, 1, 16),
        (259, 3, 1, 1),
        (262, 3, 1, 34892),
        (277, 3, 1, 3),
        (322, 4, 1, tile_w as u32),
        (323, 4, 1, tile_h as u32),
        (324, 4, tiles as u32, if tiles == 1 { offsets[0] as u32 } else { offsets_at as u32 }),
        (50714, 4, 1, 100),
        (50717, 4, 1, 1000),
    ];
    let mut out = b"II".to_vec();
    out.extend(42u16.to_le_bytes());
    out.extend((ifd_at as u32).to_le_bytes());
    out.extend(data);
    out.extend(offsets.iter().flat_map(|&o| (o as u32).to_le_bytes()));
    out.extend((entries.len() as u16).to_le_bytes());
    for (tag, t, n, v) in entries {
        out.extend(tag.to_le_bytes());
        out.extend(t.to_le_bytes());
        out.extend(n.to_le_bytes());
        if t == 3 && n == 1 {
            out.extend((v as u16).to_le_bytes());
            out.extend([0, 0]);
        } else {
            out.extend(v.to_le_bytes());
        }
    }
    out.extend(0u32.to_le_bytes());
    out
}

#[test]
fn a_tile_wider_than_the_image_reads_its_rows_at_its_own_width() {
    let (w, h) = (5usize, 6usize);
    let samples: Vec<u16> = (0..w * h * 3).map(|i| (100 + i * 3) as u16).collect();
    let strips = decode(&tiff(w, h, &samples, None, false, 34892), None).unwrap();
    let tiles = decode(&tiled(w, h, 16, 4, &samples, 1000), None).unwrap();
    assert_eq!((tiles.width, tiles.height), (w, h));
    assert_eq!(tiles.pixels, strips.pixels, "the tiles hold the strips' picture");
    // Twin: the padding reads 1.0 and the picture never does, so a row run into the padding shows.
    assert!(strips.pixels.iter().all(|p| p[0] < 0.9) && tiles.pixels.iter().all(|p| p[0] < 0.9));
}

#[test]
fn the_bytes_decide_the_branch() {
    let samples = vec![0u16; 2 * 2 * 3];
    assert_eq!(kind(&tiff(2, 2, &samples, None, true, 34892)), Some(Kind::Mosaic));
    assert_eq!(kind(&tiff(2, 2, &samples, None, false, 2)), Some(Kind::Unsupported("PhotometricInterpretation 2".into())));
    assert_eq!(kind(b"\xff\xd8\xff\xe0 a JPEG header"), None);
    // Twin: the same LinearRaw bytes are claimed.
    assert_eq!(kind(&tiff(2, 2, &samples, None, false, 34892)), Some(Kind::LinearRgb));
}

#[test]
fn a_reduced_read_takes_the_comb_mean_of_each_step() {
    assert_eq!(comb_offsets(1), vec![0]);
    assert_eq!(comb_offsets(2), vec![0, 1]);
    assert_eq!(comb_offsets(8), vec![1, 3, 5, 7]);
    // A column ramp: each step's mean is its centre, not its first sample.
    let (w, h) = (8usize, 2usize);
    let samples: Vec<u16> = (0..w * h).flat_map(|i| [(100 + 100 * (i % w)) as u16; 3]).collect();
    let l = decode(&tiff(w, h, &samples, None, false, 34892), Some(2.0)).unwrap();
    assert_eq!((l.width, l.height), (2, 1));
    let centre = |x0: usize| ((100 + 100 * (x0 + 1)) as f32 + (100 + 100 * (x0 + 2)) as f32) / 2.0;
    let want = (centre(0) - 100.0) / 900.0;
    assert!((l.pixels[0][0] - want).abs() < 1e-6, "{} against {want}", l.pixels[0][0]);
    // Twin: point decimation would read the step's first column.
    assert!((l.pixels[0][0] - 0.0).abs() > 0.1);
}
