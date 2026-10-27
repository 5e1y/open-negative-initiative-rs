//! LibRaw's frame on DNGs built here: a LinearRaw file read whole at every size, a mosaic's default
//! crop placed inside its ActiveArea.

use crate::source::tests::{Tag, directory, scratch};
use crate::{HALF, Linear, Neutral, decode, decode_for, decode_on};
use std::path::Path;

const W: usize = 96;
const H: usize = 64;

fn short(v: u16) -> Vec<u8> {
    v.to_le_bytes().to_vec()
}

fn longs(v: &[usize]) -> Vec<u8> {
    v.iter().flat_map(|&v| (v as u32).to_le_bytes()).collect()
}

/// A DNG `w` by `h` named for the Z f, one strip of 16-bit `samples`: a mosaic RGGB for one sample
/// a pixel, LinearRaw for three; `extra` tags join its directory.
fn dng(path: &Path, (w, h): (usize, usize), spp: usize, samples: &[u16], extra: Vec<Tag>) {
    let strip: Vec<u8> = samples.iter().flat_map(|v| v.to_le_bytes()).collect();
    let rational = |v: f64| [((v * 10000.0).round() as i32).to_le_bytes(), 10000i32.to_le_bytes()].concat();
    let matrix: Vec<u8> = [1.1607, -0.4491, -0.0977, -0.4522, 1.246, 0.2304, -0.0458, 0.1519, 0.7616].iter().flat_map(|&v| rational(v)).collect();
    let mut tags: Vec<Tag> = vec![
        (254, 4, 1, longs(&[0])),
        (256, 4, 1, longs(&[w])),
        (257, 4, 1, longs(&[h])),
        (258, 3, spp as u32, short(16).repeat(spp)),
        (259, 3, 1, short(1)),
        (262, 3, 1, short(if spp == 1 { 32803 } else { 34892 })),
        (271, 2, 18, b"NIKON CORPORATION\0".to_vec()),
        (272, 2, 10, b"NIKON Z f\0".to_vec()),
        (273, 4, 1, Vec::new()),
        (274, 3, 1, short(1)),
        (277, 3, 1, short(spp as u16)),
        (278, 4, 1, longs(&[h])),
        (279, 4, 1, longs(&[strip.len()])),
        (284, 3, 1, short(1)),
        (50706, 1, 4, vec![1, 4, 0, 0]),
        (50708, 2, 10, b"Nikon Z f\0".to_vec()),
        (50717, 4, 1, longs(&[65535])),
        (50721, 10, 9, matrix),
        (50778, 3, 1, short(21)),
    ];
    if spp == 1 {
        tags.push((33421, 3, 2, short(2).repeat(2)));
        tags.push((33422, 1, 4, vec![0, 1, 1, 2]));
    }
    tags.extend(extra);
    tags.sort_by_key(|t| t.0);
    let mut out = b"II*\0".to_vec();
    out.extend(longs(&[8]));
    directory(&mut out, &tags, &strip, false);
    std::fs::write(path, out).unwrap();
}

/// A sample that names its place on the sensor, whatever the file's own origin.
fn at(x: usize, y: usize) -> u16 {
    (2000 + x * 300 + y * 150 + (x * 31 + y * 17) * 7919 % 900) as u16
}

fn crop((x, y): (usize, usize), (w, h): (usize, usize)) -> Vec<Tag> {
    vec![(50719, 4, 2, longs(&[x, y])), (50720, 4, 2, longs(&[w, h]))]
}

fn bits(l: &Linear) -> (usize, usize, Vec<u32>) {
    (l.width, l.height, l.pixels.iter().flatten().map(|v| v.to_bits()).collect())
}

#[test]
fn a_linear_dng_is_read_whole_at_factor_one() {
    let root = scratch("linear-half");
    let linear = root.join("Linear.DNG");
    let samples: Vec<u16> = (0..W * H).flat_map(|i| [0, 1, 2].map(|c| at(i % W, i / W) + c * 500)).collect();
    dng(&linear, (W, H), 3, &samples, crop((8, 4), (W - 16, H - 8)));
    let whole = decode(&linear, 1.0).unwrap();
    let (small, factor) = decode_for(&linear, 1.0, HALF).unwrap();
    assert_eq!((factor, small.width, small.height), (1, W - 16, H - 8), "LibRaw halves no LinearRaw file");
    assert!(bits(&small) == bits(&whole), "the small copy is the whole decode");
    // Twin: a mosaic of the same size and crop is halved, a factor of 2.
    let mosaic = root.join("Mosaic.DNG");
    dng(&mosaic, (W, H), 1, &(0..W * H).map(|i| at(i % W, i / W)).collect::<Vec<_>>(), crop((8, 4), (W - 16, H - 8)));
    let (m, factor) = decode_for(&mosaic, 1.0, HALF).unwrap();
    assert_eq!((factor, m.width, m.height), (2, (W - 16) / 2, (H - 8) / 2));
    let _ = std::fs::remove_dir_all(&root);
}

/// LibRaw counts the crop from the raw's corner, the ActiveArea's margins included; the image it hands
/// out starts at the margins, so the frame is the one a file without them shows.
#[test]
fn a_default_crop_lands_inside_the_active_area() {
    let root = scratch("active-area");
    let (left, top) = (4, 2);
    let (size, origin) = ((80, 52), (2, 2));
    let framed = root.join("Framed.DNG");
    let mut tags = vec![(50829, 4, 4, longs(&[top, left, H, W]))];
    tags.extend(crop(origin, size));
    dng(&framed, (W, H), 1, &(0..W * H).map(|i| at(i % W, i / W)).collect::<Vec<_>>(), tags);
    let (vw, vh) = (W - left, H - top);
    let visible: Vec<u16> = (0..vw * vh).map(|i| at(i % vw + left, i / vw + top)).collect();
    let bare = root.join("Bare.DNG");
    dng(&bare, (vw, vh), 1, &visible, crop(origin, size));
    let (a, b) = (decode(&framed, 1.0).unwrap(), decode(&bare, 1.0).unwrap());
    assert_eq!((a.width, a.height), size);
    assert!(bits(&a) == bits(&b), "the same frame with or without the margins");
    // Twin: a window shifted by the margins differs, so the comparison sees the shift.
    let shifted = root.join("Shifted.DNG");
    dng(&shifted, (vw, vh), 1, &visible, crop((origin.0 + left, origin.1 + top), size));
    let s = decode(&shifted, 1.0).unwrap();
    assert_eq!((s.width, s.height), size);
    assert!(bits(&a) != bits(&s));
    let _ = std::fs::remove_dir_all(&root);
}

/// A slide's decode is balanced at 5500 K and kept apart from a negative's, so a frame turned to a
/// positive never reads the other's decode back.
#[test]
fn a_slide_decodes_cooler_and_is_kept_under_its_own_key() {
    let root = scratch("slide");
    crate::cache::keep_in(root.join("Cache"), u64::MAX);
    let mosaic = root.join("Slide.DNG");
    dng(&mosaic, (W, H), 1, &(0..W * H).map(|i| at(i % W, i / W)).collect::<Vec<_>>(), crop((8, 4), (W - 16, H - 8)));
    let mean = |l: &Linear| [0, 1, 2].map(|c| l.pixels.iter().map(|p| f64::from(p[c])).sum::<f64>());
    let (swift, _) = decode_on(&mosaic, 1.0, 0, Neutral::Swift).unwrap();
    let (slide, _) = decode_on(&mosaic, 1.0, 0, Neutral::Slide).unwrap();
    let (a, b) = (mean(&swift), mean(&slide));
    assert!(b[2] / b[0] < a[2] / a[0], "a 5500 K balance lowers blue against red: {a:?} {b:?}");
    // Twin: the negative's decode read back from the cache is its own, bit for bit.
    let (again, _) = decode_on(&mosaic, 1.0, 0, Neutral::Swift).unwrap();
    assert!(bits(&again) == bits(&swift) && bits(&again) != bits(&slide));
    assert!(bits(&decode_on(&mosaic, 1.0, 0, Neutral::Slide).unwrap().0) == bits(&slide));
    let _ = std::fs::remove_dir_all(&root);
}
