//! A batch laid on an image: what its members give chained one by one, bit for bit, in passes whose
//! count their number leaves alone.

mod common;

use common::{Stamp, error, noise};
use oni_graph::stages::{AnchoredLanczos, Axis, BoxBlur3, Gain};
use oni_graph::{Batch, Cache, Graph, Image, Laid, Rect, Source, Stage};

const W: u32 = 300;
const H: u32 = 200;

/// The most a batch's render may lie from its members' chained one by one, as `common::error` counts:
/// Metal rounds a kernel compiled for many instances a few ulps apart from the one compiled for one (1e-6 seen).
const ULPS: f32 = 1e-5;

fn graph(check: &str) -> Option<Graph> {
    Graph::new().map_err(|e| println!("SKIPPED {check}: {e}")).ok()
}

/// The image moved by whole pixels, as a batch's `moved` does at no pass.
struct Moved(i32, i32);

impl Stage for Moved {
    fn name(&self) -> &'static str {
        "moved"
    }

    fn shader(&self) -> &'static str {
        r#"
struct Params { by: vec4<i32> }
@group(0) @binding(3) var<uniform> params: Params;
fn pixel(p: vec2<i32>) -> vec4<f32> { return load(p - params.by.xy); }
"#
    }

    fn params(&self) -> Vec<u8> {
        [self.0, self.1, 0, 0].iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn extent(&self, input: Rect) -> Rect {
        Rect::new(input.x + self.0, input.y + self.1, input.w, input.h)
    }

    fn input_region(&self, output: Rect) -> Rect {
        Rect::new(output.x - self.0, output.y - self.1, output.w, output.h)
    }
}

/// A member: its pixels, where it lands on the image, and the shifts of its reduction.
struct Member {
    size: (u32, u32),
    pixels: Vec<[f32; 4]>,
    at: (i32, i32),
    shift: (f64, f64),
    blurred: bool,
}

const SCALE: f64 = 0.6;

impl Member {
    fn along(&self, axis: Axis) -> AnchoredLanczos {
        let (from, shift) = if axis == Axis::X { (self.size.0, self.shift.0) } else { (self.size.1, self.shift.1) };
        AnchoredLanczos { axis, origin: 0, from, scale: SCALE, shift }
    }

    /// Where its reduction lands on the image.
    fn tile(&self) -> Rect {
        Rect::new(self.at.0, self.at.1, self.along(Axis::X).to(), self.along(Axis::Y).to())
    }
}

/// Members of several sizes across the frame: two pairs that overlap, one on the edge, one far larger.
fn members() -> Vec<Member> {
    let mut list: Vec<Member> = (0..12)
        .map(|k| {
            let size = (9 + (k as u32 % 5) * 7, 8 + (k as u32 % 4) * 5);
            Member { size, pixels: noise(size.0, size.1, 300 + k), at: (6 + 23 * k as i32, 10 + (k as i32 % 3) * 58), shift: (0.13 * (k % 4) as f64, 0.21 * (k % 3) as f64), blurred: k % 2 == 0 }
        })
        .collect();
    let extra = |size: (u32, u32), at, seed| Member { size, pixels: noise(size.0, size.1, seed), at, shift: (0.4, 0.7), blurred: true };
    list.extend([extra((30, 26), (10, 14), 401), extra((30, 26), (14, 20), 402), extra((40, 30), (280, 186), 403), extra((170, 120), (120, 60), 404)]);
    list
}

/// Every member's pixels on one sheet, in a row, and where each lies there.
fn sheet(g: &mut Graph, members: &[Member]) -> (Source, Vec<Rect>) {
    let (wide, high) = (members.iter().map(|m| m.size.0).sum::<u32>(), members.iter().map(|m| m.size.1).max().unwrap());
    let mut pixels = vec![[0.0f32; 4]; (wide * high) as usize];
    let mut at = Vec::new();
    let mut x = 0;
    for m in members {
        for row in 0..m.size.1 {
            let from = (row * m.size.0) as usize;
            let to = (row * wide + x) as usize;
            pixels[to..to + m.size.0 as usize].copy_from_slice(&m.pixels[from..from + m.size.0 as usize]);
        }
        at.push(Rect::new(x as i32, 0, m.size.0, m.size.1));
        x += m.size.0;
    }
    (g.upload(wide, high, &pixels).expect("the sheet"), at)
}

/// The members laid as a batch; `order` names them in the order laid.
fn laid(g: &mut Graph, base: &Image, members: &[Member], order: &[usize]) -> Image {
    let picked: Vec<&Member> = order.iter().map(|&k| &members[k]).collect();
    let (sheet, at) = sheet(g, &picked.iter().map(|m| Member { size: m.size, pixels: m.pixels.clone(), at: m.at, shift: m.shift, blurred: m.blurred }).collect::<Vec<_>>());
    let batch = Batch::of(&sheet, at)
        .then(picked.iter().map(|m| Some(m.along(Axis::X))).collect())
        .then(picked.iter().map(|m| Some(m.along(Axis::Y))).collect())
        .then(picked.iter().map(|m| m.blurred.then_some(BoxBlur3)).collect())
        .moved(picked.iter().map(|m| [m.at.0, m.at.1]).collect());
    let stamps = picked.iter().map(|m| Stamp { at: m.tile(), area: Some(m.tile()) }).collect();
    base.clone().then_laid(&Laid::new(stamps, &[batch])).then(Gain([0.8; 4]))
}

/// The same members chained one by one, each an image of its own.
fn chained(g: &mut Graph, base: &Image, members: &[Member], order: &[usize]) -> Image {
    let mut image = base.clone();
    for &k in order {
        let m = &members[k];
        let source = g.upload(m.size.0, m.size.1, &m.pixels).expect("a member");
        let mut tile = Image::new(&source).lanczos_shifted(SCALE, m.shift);
        if m.blurred {
            tile = tile.then(BoxBlur3);
        }
        image = image.then_reading(Stamp { at: m.tile(), area: Some(m.tile()) }, &[tile.then(Moved(m.at.0, m.at.1))]);
    }
    image.then(Gain([0.8; 4]))
}

fn regions() -> Vec<Rect> {
    let whole = Rect::new(0, 0, W, H);
    vec![whole, Rect::new(0, 0, 60, 50), Rect::new(W as i32 - 30, H as i32 - 20, 30, 20), Rect::new(100, 50, 120, 90), Rect::new(17, 22, 1, 1), Rect::new(130, 70, 25, 1), Rect::new(60, 0, 200, 9)]
}

#[test]
fn a_batch_laid_renders_what_its_members_chained_one_by_one_do() {
    let Some(mut g) = graph("a_batch_laid_renders_what_its_members_chained_one_by_one_do") else { return };
    let source = g.upload(W, H, &noise(W, H, 7)).expect("upload");
    let base = Image::new(&source).then(Gain([1.1, 0.9, 1.2, 1.0]));
    let members = members();
    let order: Vec<usize> = (0..members.len()).collect();
    let (together, one_by_one) = (laid(&mut g, &base, &members, &order), chained(&mut g, &base, &members, &order));
    assert_eq!(together.extent(), one_by_one.extent());
    let mut worst: f32 = 0.0;
    for region in regions() {
        let (got, want) = (g.render(&together, region, None).expect("laid"), g.render(&one_by_one, region, None).expect("chained"));
        worst = worst.max(error(&got, &want));
        assert!(error(&got, &want) < ULPS, "{region:?} lies {} from the members chained", error(&got, &want));
        // A render of a region is the whole render's own pixels there, and a cache changes none.
        let whole = g.render(&together, Rect::new(0, 0, W, H), None).expect("whole");
        assert_eq!(got, common::cut(&whole, Rect::new(0, 0, W, H), region), "{region:?} is not the whole render's part");
    }
    println!("a batch lies {worst:e} at most from its members chained");
    let mut cache = Cache::frontier();
    for region in regions() {
        assert_eq!(g.render(&together, region, Some(&mut cache)).expect("cached"), g.render(&together, region, None).expect("fresh"), "{region:?} cached");
    }

    // Twin: the overlapping pair laid the other way round is another picture, which the comparison sees.
    let mut swapped = order.clone();
    swapped.swap(12, 13);
    let other = laid(&mut g, &base, &members, &swapped);
    let pair = Rect::new(0, 0, 60, 50);
    let (turned, straight) = (g.render(&other, pair, None).unwrap(), g.render(&one_by_one, pair, None).unwrap());
    assert!(error(&turned, &straight) > 1e-2, "the order of two members that meet went unseen: {}", error(&turned, &straight));
    let other_chained = chained(&mut g, &base, &members, &swapped);
    assert!(error(&turned, &g.render(&other_chained, pair, None).unwrap()) < ULPS);
    // Twin: a member reduced with another shift lands elsewhere, far past the tolerance.
    let mut off = members;
    off[3].shift.0 += 0.3;
    let shifted = laid(&mut g, &base, &off, &order);
    let whole = Rect::new(0, 0, W, H);
    assert!(error(&g.render(&shifted, whole, None).unwrap(), &g.render(&one_by_one, whole, None).unwrap()) > 1e-2, "a member's own parameters went unseen");
}

#[test]
fn a_batch_s_passes_do_not_grow_with_its_members() {
    let Some(mut g) = graph("a_batch_s_passes_do_not_grow_with_its_members") else { return };
    let source = g.upload(W, H, &noise(W, H, 9)).expect("upload");
    let base = Image::new(&source).then(Gain([1.1; 4]));
    // Members of one size, apart from each other: one layer, one size class.
    let apart = |n: usize| -> Vec<Member> {
        (0..n).map(|k| Member { size: (12, 10), pixels: noise(12, 10, 500 + k as u64), at: (4 + 14 * (k as i32 % 20), 6 + 16 * (k as i32 / 20)), shift: (0.2, 0.3), blurred: true }).collect()
    };
    let whole = Rect::new(0, 0, W, H);
    let passes = |g: &mut Graph, image: &Image| {
        let before = g.stats().dispatches;
        g.render(image, whole, None).expect("render");
        g.stats().dispatches - before
    };
    let (few, many) = (apart(4), apart(200));
    let order = |m: &[Member]| (0..m.len()).collect::<Vec<usize>>();
    let (laid_few, laid_many) = (laid(&mut g, &base, &few, &order(&few)), laid(&mut g, &base, &many, &order(&many)));
    let (c_few, c_many) = (chained(&mut g, &base, &few, &order(&few)), chained(&mut g, &base, &many, &order(&many)));
    assert!(error(&g.render(&laid_many, whole, None).unwrap(), &g.render(&c_many, whole, None).unwrap()) < ULPS);
    let (four, two_hundred) = (passes(&mut g, &laid_few), passes(&mut g, &laid_many));
    println!("{four} passes for 4 members laid, {two_hundred} for 200");
    assert_eq!(four, two_hundred, "a batch of 200 took more passes than one of 4");
    assert!(two_hundred <= 12, "{two_hundred} passes for a batch");
    // Twin: chained one by one, each member costs its own passes, which the counter sees.
    let (chain_four, chain_two_hundred) = (passes(&mut g, &c_few), passes(&mut g, &c_many));
    println!("{chain_four} passes for 4 members chained, {chain_two_hundred} for 200");
    assert!(chain_two_hundred >= chain_four + 196 * 5, "{chain_four} then {chain_two_hundred} passes chained");
}

#[test]
fn a_batch_past_one_turn_and_one_laid_last_render_as_their_chains() {
    let Some(mut g) = graph("a_batch_past_one_turn_and_one_laid_last_render_as_their_chains") else { return };
    let source = g.upload(W, H, &noise(W, H, 11)).expect("upload");
    let base = Image::new(&source);
    // More members than a turn holds, each meeting its neighbours.
    let many: Vec<Member> = (0..2300usize)
        .map(|k| Member { size: (5, 4), pixels: noise(5, 4, 900 + k as u64), at: (4 * (k as i32 % 72), 3 * (k as i32 / 72)), shift: (0.0, 0.5), blurred: false })
        .collect();
    let order: Vec<usize> = (0..many.len()).collect();
    let whole = Rect::new(0, 0, W, H);
    let (gathered, at) = sheet(&mut g, &many);
    let tiles = Batch::of(&gathered, at).moved(many.iter().map(|m| [m.at.0, m.at.1]).collect());
    let stamps = |members: &[Member]| members.iter().map(|m| Rect::new(m.at.0, m.at.1, m.size.0, m.size.1)).map(|at| Stamp { at, area: Some(at) }).collect::<Vec<_>>();
    // Laid last: the render's own output is the image the batch was laid on.
    let together = base.clone().then_laid(&Laid::new(stamps(&many), &[tiles]));
    let mut one_by_one = base.clone();
    for &k in &order {
        let m = &many[k];
        let at = Rect::new(m.at.0, m.at.1, m.size.0, m.size.1);
        let tile = g.upload_at(at, &m.pixels).expect("a member");
        one_by_one = one_by_one.then_reading(Stamp { at, area: Some(at) }, &[Image::new(&tile)]);
    }
    let reference = g.render(&one_by_one, whole, None).expect("chained");
    assert_eq!(g.render(&together, whole, None).expect("laid"), reference);
    // Kept as a source, and laid on a sheet, the same pixels.
    let kept = g.materialise(&together).expect("materialised");
    assert_eq!(g.render(&Image::new(&kept), whole, None).unwrap(), reference);
    let on_sheet = g.laid_out(&[(&together, (3, 2))], (W + 5, H + 4)).expect("a sheet");
    assert_eq!(g.render(&Image::new(&on_sheet), Rect::new(3, 2, W, H), None).unwrap(), reference);
    // Laid last on a source in half floats, a region no member reaches is still read as full floats.
    let half = g.upload_half(W, H, &noise(W, H, 11)).expect("upload");
    let (tiles_sheet, tiles_at) = sheet(&mut g, &many[..3]);
    let few = Laid::new(stamps(&many[..3]), &[Batch::of(&tiles_sheet, tiles_at).moved(many[..3].iter().map(|m| [m.at.0, m.at.1]).collect())]);
    let away = Rect::new(200, 150, 40, 30);
    let bare = g.render(&Image::new(&half), away, None).expect("the source alone");
    assert_eq!(g.render(&Image::new(&half).then_laid(&few), away, None).expect("laid last, none shown"), bare);
    // Twin: the region beside it reads other pixels, which the comparison sees.
    assert_ne!(g.render(&Image::new(&half).then_laid(&few), Rect::new(160, 150, 40, 30), None).unwrap(), bare);
    // Twin: a member left out of the batch is seen.
    let (fewer_sheet, fewer_at) = sheet_without_last(&mut g, &many);
    let fewer = base.clone().then_laid(&Laid::new(stamps(&many[..many.len() - 1]), &[Batch::of(&fewer_sheet, fewer_at).moved(many[..many.len() - 1].iter().map(|m| [m.at.0, m.at.1]).collect())]));
    assert_ne!(g.render(&fewer, whole, None).unwrap(), reference, "a member missing went unseen");
}

fn sheet_without_last(g: &mut Graph, members: &[Member]) -> (Source, Vec<Rect>) {
    let kept: Vec<Member> = members[..members.len() - 1].iter().map(|m| Member { size: m.size, pixels: m.pixels.clone(), at: m.at, shift: m.shift, blurred: m.blurred }).collect();
    sheet(g, &kept)
}
