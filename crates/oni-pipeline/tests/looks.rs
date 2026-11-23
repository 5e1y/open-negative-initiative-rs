//! A `.cube` file in the person's Stocks folder as a film of its own: a digital photo on it is read through the cube in
//! display sRGB code in place of a negative, its film's exposure first; on a paper its positive prints through the
//! internegative. Each verdict has a twin.

mod common;

use common::{V4, graph, judge, mapped, whole};
use oni_pipeline::stocks::{Loader, look_key};
use oni_pipeline::{Pipeline, Request, conversion, paper};
use oni_render::{ColorCube, ScannerEncode, ScannerOutput, cpu};
use oni_settings::{ConversionMode, PipelineSettings, VirtualStock, stocks};

/// A cube of `n` nodes a side whose every node is `f` of its own place.
fn cube(n: usize, f: impl Fn([f32; 3]) -> [f32; 3]) -> String {
    let mut out = format!("TITLE \"test\"\nLUT_3D_SIZE {n}\n");
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let o = f([r, g, b].map(|v| v as f32 / (n - 1) as f32));
                out += &format!("{} {} {}\n", o[0], o[1], o[2]);
            }
        }
    }
    out
}

fn stock(key: &str) -> VirtualStock {
    serde_json::from_value(key.into()).unwrap()
}

/// The look's reference: exposed, encoded, read through `nodes`, decoded, as the kernels' references lay them.
fn reference(p: V4, stops: f32, n: usize, nodes: &[[f32; 4]]) -> V4 {
    let k = ColorCube { cube_dimension: n, cube_data: nodes.to_vec() };
    let code = cpu::scanner_encode(p.map(|v| v * stops.exp2()), &ScannerEncode { root: 1.0 });
    let code = [code[0], code[1], code[2], p[3]];
    let read = cpu::color_cube(code, &k);
    cpu::scanner_output(read, read, &ScannerOutput { warmth_share: 0.0, lift: 0.0, contrast: 0.0 })
}

#[test]
fn a_cube_in_the_stocks_folder_is_a_film_a_digital_photo_is_read_through() {
    assert_eq!(look_key("My Look / Portra-800"), "lut-my-look-portra-800");
    let dir = std::env::temp_dir().join(format!("oni-looks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let curve = |v: [f32; 3]| [v[2].powf(0.8), v[1] * 0.9, v[0].sqrt()];
    std::fs::write(dir.join("Swapped.cube"), cube(9, curve)).unwrap();
    std::fs::write(dir.join("broken.cube"), "LUT_3D_SIZE 3\n0 0 0\n").unwrap();
    let mut loader = Loader::new(dir.clone());
    let reloaded = loader.reload().expect("a first read");
    assert!(reloaded.refused.iter().any(|t| t.refusal.name == "broken.cube" && t.refusal.reason.contains("27")), "{:?}", reloaded.refused);
    stocks::install(reloaded.catalogue);

    let swapped = stock("lut-swapped");
    let (entry, look) = swapped.look().expect("the cube is a film with its look");
    assert_eq!((entry.name, look.size), ("test", 9));
    assert!(VirtualStock::default().look().is_none() && stock("lut-gone").look().is_none(), "a film of no cube, or of none, has no look");
    // Offered to digital photos alone, under its own brand.
    let kind = oni_settings::film::stocks().iter().find(|k| k.film == Some(swapped)).copied().expect("listed");
    assert!(!kind.offered && kind.brand == "LUT");

    let Some(mut g) = graph("a_cube_in_the_stocks_folder_is_a_film_a_digital_photo_is_read_through") else { return };
    let (w, h) = (23, 11);
    let pixels: Vec<V4> = common::noise(w, h, 3, 0.0, 1.2).iter().map(|p| p.map(|v| v * v)).collect();
    let source = g.upload(w, h, &pixels).unwrap();
    let mut s = PipelineSettings::rest(ConversionMode::Digital, true).on_stock(swapped);
    (s.grain.amount, s.print_paper, s.virtual_film.as_mut().unwrap().exposure) = (0.0, None, 0.5);
    let mut pipeline = Pipeline::new();
    let render = |p: &mut Pipeline, g: &mut oni_graph::Graph, s: &PipelineSettings| p.render(g, &source, &s.per_pixel_only(), &Request::default(), &Default::default(), None).unwrap().1;
    let out = render(&mut pipeline, &mut g, &s);
    let expected = mapped(&pixels, |p| reference(p, 0.5, 9, &look.nodes));
    // Twin: the same cube without the film's exposure.
    judge("a photo through its look", &out, &expected, &mapped(&pixels, |p| reference(p, 0.0, 9, &look.nodes)));

    // On a paper the positive prints through the internegative: the copy reads it at unit light, the paper's print laid.
    let on_paper = PipelineSettings { print_paper: Some(Default::default()), ..s.clone() };
    let copy = paper::internegative::copy(&on_paper).expect("a look on a paper is copied");
    assert_eq!((copy.light.r_vector[0], copy.light.bias_vector[0]), (1.0, 0.0));
    assert!(conversion::stage47(&on_paper).paper.is_some() && paper::internegative::copy(&s).is_none());
    let printed = render(&mut pipeline, &mut g, &on_paper);
    let positive = g.upload(w, h, &expected).unwrap();
    let filmed = pipeline.filmed(&mut g, oni_graph::Image::new(&positive).then(oni_pipeline::stages::color_matrix(&copy.light)), &copy.settings).unwrap();
    let through = whole(&mut g, &oni_pipeline::stages::converted(filmed, &conversion::stage47(&on_paper)));
    let clipped = mapped(&through, |p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0), p[2].clamp(0.0, 1.0), p[3]]);
    // Twin: the positive shown as it is, no paper.
    judge("a look printed on its paper", &printed, &clipped, &expected);
    let _ = std::fs::remove_dir_all(&dir);
}
