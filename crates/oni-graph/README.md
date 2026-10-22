# oni-graph

The render graph of Open Negative Initiative in Rust: what Core Image's lazy evaluation does for the
Swift app, on `wgpu` 30 (Metal on macOS, Direct3D 12 on Windows, Vulkan elsewhere).

An `Image` is a recipe: a `Source` resident on the GPU followed by a chain of `Stage`s. Nothing runs
until a region is asked for; then the graph works out, stage by stage from the end, which pixels each
stage needs, runs only those, and reads the region back as `Vec<[f32; 4]>`.

## The model

- **Two precisions.** Every stage writes `rgba32float`; a source may be held in `rgba16float`
  (`upload_half`, `materialise_half`), which stages read alike. Only the internal pool creates
  textures, stages never see one, and data enters and leaves as `[f32; 4]`.
- **Pooled sizes.** A stage's texture is taken from the pool with its sides rounded up to `STEP`, so
  stages a few pixels apart in reach share textures; each dispatch clamps reads to the rectangle held.
- **Coordinates.** Whole pixels, origin top-left, rows running down as the decoded buffers do. A
  `Rect` is `x, y, w, h`. Extents keep their coordinates through a crop, as Core Image's do.
- **A stage** is a WGSL function `fn pixel(p: vec2<i32>) -> vec4<f32>` giving the output at image
  coordinate `p`, plus two rules:
  - `extent(input) -> Rect`: what it produces (a crop intersects, a reduction rescales);
  - `input_region(output) -> Rect`: every input pixel it reads for that output (a 3 x 3 blur adds
    one pixel each side; a Lanczos reduction maps through its scale and adds its support).

  Inside the shader, `load(q)` reads the input at image coordinate `q`, the nearest edge pixel outside
  the input's extent; `inside(q)` tells whether `q` is in it. Parameters are a uniform at
  `@group(0) @binding(3)`, whose bytes `params()` returns.
- **Regions.** For a request `R`, each depth needs `input_region(next) ∩ extent`. Evaluating a region
  therefore gives exactly the pixels of the full render cut to that region.
- **Auxiliary inputs.** `then_reading(stage, &[images])` binds up to `MAX_AUX` further images, read in
  the shader with `load1(q)`, `inside1(q)` and so on, each in its own coordinates; `aux_region(k, output)`
  declares what is read of each. A render is a graph, not a chain: nodes are keyed by everything upstream
  of them, so a prefix an auxiliary image shares with the chain runs once, over the union of what its
  readers need, and its texture returns to the pool once its last reader has run.
- **Tiles.** A source past the texture side (16 384 px, or the device's limit if lower) is laid in tiles,
  one texture each. A stage that needs pixels from several tiles gets them copied into one texture, so
  a filter across a seam reads its neighbour. A request whose needs pass the side at any depth is halved
  until each piece fits, and the pieces are read back into one buffer.
- **The pool** recycles intermediates of the same size. A texture retired during one render and not
  reused by the next is destroyed at once; `purge()` frees every idle one.
- **The cache.** A `Cache` holds each stage's last result, keyed by a hash of the source and of every
  stage up to it (name, shader, parameters). A render starts from the deepest result still current that
  covers what is needed, so a notch on a late stage reruns only that stage and those after it. A cache
  belongs to one view of one `Graph`; renders without one (export, thumbnails) keep nothing.

## API

```rust
use oni_graph::{Cache, Graph, Image, Rect};
use oni_graph::stages::{BoxBlur3, Gain};

let mut graph = Graph::new()?;                         // or Graph::with_max_side(n), Graph::from_device(..)
let source = graph.upload(width, height, &pixels)?;    // &[[f32; 4]], row-major
let image = Image::new(&source)
    .then(Gain([1.2, 1.0, 0.9, 1.0]))
    .then(BoxBlur3);
let mut cache = Cache::default();
let view: Vec<[f32; 4]> = graph.render(&image, Rect::new(0, 0, 800, 600), Some(&mut cache))?;

// A preview held as pixels: Lanczos-3 to a 2000 px long side, evaluated once.
let side = image.extent().w.max(image.extent().h) as f64;
let preview = graph.materialise(&image.clone().lanczos(2000.0 / side))?;
```

| Item | What it is |
|---|---|
| `Graph::new`, `Graph::with_max_side` | The device, the compiled stages, the pool. |
| `Graph::upload` | Pixels to a `Source`, tiled past the side. |
| `Graph::render(image, region, cache)` | The region, evaluated and read back. |
| `Graph::materialise(image)` | The whole image evaluated once into a new `Source`. |
| `Graph::stats`, `Graph::purge` | Dispatch, copy and allocation counters; freeing idle textures. |
| `Image::new`, `Image::then`, `Image::lanczos`, `Image::extent` | Building a recipe. |
| `Stage` | The trait a WGSL stage implements. |
| `Cache` | Per-stage results for one view. |
| `stages::Lanczos` | Lanczos-3 along one axis; `Image::lanczos` adds both. |
| `stages::{Gain, BoxBlur3, Crop}` | The stages the tests drive. |
| `Image::then_reading`, `Stage::aux_region` | A stage reading further images. |
| `Graph::upload_at`, `Graph::materialise_region` | A source anywhere; a region held on the GPU. |
| `cpu` | The scalar reference of every kernel above. |

## Checks

`cargo test --release` runs them; each carries a twin that must fail.

- `tests/graph.rs`, on noise: a region equals the full render cut (twins: a blur declaring no margin,
  a Lanczos declaring two pixels less reach); each kernel matches its reference (twins: a 1 % gain
  error, a missing blur); a late change reruns one stage, counted in dispatches, and another source
  reruns all (twins: the counter without a cache, the stale picture); a source in 64 px tiles equals
  one texture to the bit (twin: tiles blurred apart show their seams); the pool stops allocating on a
  repeated shape and frees a shape left behind; a materialised image, held whole or in tiles, renders
  as its recipe (twin: one stage short); an enlargement keeps both. The judge scores a NaN or an
  infinity on one side as infinite, checked by its own test.
- `tests/roll8.rs`, on roll #8 (`ONI_RAW_DIR`, skipped when absent): every frame's 2000 px
  preview and 1400 px measuring copy, tiled at 2048 px against untiled and against the scalar Lanczos
  (twin: the reference of a softened frame); a 1:1 canvas region across tile seams and a late slider
  notch on DSC_6448. Decodes run one at a time: oni-decode's LibRaw is built without thread safety.

## Not here

The pipeline's stages themselves (the fused stage 4 to 7 kernel lives in `oni-render`), geometry's
rotation and the fractions-to-pixels quantisation, the canvas's memory budget and its switch to full
resolution, and the canvas's own display surface.
