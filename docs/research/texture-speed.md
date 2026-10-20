# Texture, faster

Specification for cutting Texture's GPU cost (Details tab). Texture is the largest share of a busy
render: 6.96 ms on a 2000 px preview on the M2 Max, Texture 3.65 ms of it, chroma denoise 1.49, sharpen
1.35 (#153; history in #119, #138, `docs/research/performance.md`). Exact stays the default; a technique
that matches the exact path within float tolerance ships with no choice, and only a technique that does
not becomes an opt-in setting, for the screen alone.

## 1. What runs today

`oni_pipeline::texture` (`crates/oni-pipeline/src/lib.rs`), a port of `Pipeline.applyTexture`
(`Sources/OpenNegative/Pipeline/Texture.swift`): a self-guided filter on the luma.

| # | Stage | Reads | Writes | Taps a pixel at 2000 px / 24 MP |
|---|-------|-------|--------|---------------------------------|
| 1 | `BlurOf(narrow, Before::Luma)`, rows | source | N₁ | 5 / 11 |
| 2 | `Blur(narrow)`, columns | N₁ | N (narrow luma) | 5 / 11 |
| 3 | `BlurOf(wide, Before::Moments)`, rows | N | M₁ | 13 / 35 |
| 4 | `Blur(wide)`, columns | M₁ | M (n̄, n̄²) | 13 / 35 |
| 5 | `BlurOf(wide, Before::Coeff)`, rows | M (coefficients per tap) | C₁ | 13 / 35 |
| 6 | `Blur(wide)`, columns | C₁ | C (ā, b̄) | 13 / 35 |
| 7 | `texture(gain)` | source, N, C | output | 3 / 3 |

The radii are fractions of the rendered long side: narrow 0.0002, wide 0.0008, so 0.4 and 1.6 px on
the 2000 px preview (reach `ceil(3.5σ)` = 2 and 6), 1.21 and 4.84 px on a 6048 px frame (reach 5
and 17); a 1:1 view renders its region with the full frame's radii. Every intermediate is `rgba32float`,
although N carries one value and M, C two. Every pass recomputes its erf weights per pixel (`taps()` in
`detail_taps.wgsl`), and pass 5 recomputes the coefficients for each of its 13 taps.

**Traffic.** A 2000 × 1333 frame is 42.7 MB; a 6048 × 4032 frame (24.4 MP) 390 MB. At one read per
input and one write per output, the chain moves 16 frames, 256 B a pixel: 683 MB on the preview, 6.24
GB on 24 MP. 683 MB in 3.65 ms is an effective 187 GB/s, under half the M2 Max's ~400 GB/s, so tap
fetches through the texture cache and seven dispatches with their barriers cost about as much as the
bytes. The estimates below scale 3.65 ms by bytes moved (the bandwidth model) and give a range for the
fetch share; the first commit measures each pass so later commits are judged on numbers.

**Finding beside the brief.** `Engine::prewarm` renders a 64 px frame. `finishing::texture` rests when
the wide radius is under `MINIMUM_BASE` (0.6 px, a long side under 750 px), so Texture never runs in the
prewarm and its four shaders compile on the first notch, the half second the prewarm exists to hide.

## 2. Candidates

| Id | Technique | Exact? | Hard rules touched | Texture, 2000 px (est.) |
|----|-----------|--------|--------------------|-------------------------|
| E | Tap weights computed once on the CPU | within 1e-7 | none | −0.1 to −0.2 ms |
| A | Final kernel folded into the last column blur | bit-identical arithmetic | none | 3.2 ms (−0.45) |
| B | Each blur one pass, rows held in workgroup memory | bit-identical arithmetic | none (graph model extended) | with A: 2.2 ms (1.8–2.6) |
| D | One- and two-channel `r32float`/`rg32float` intermediates | bit-identical values | rule 2's letter | with A+B: 1.1–1.5 ms |
| C | Wide blurs at half resolution, coefficients upsampled | no | parity (opt-in only) | 1:1 view: about half of A+B |
| — | `rgba16float` intermediates | no: the variance, mean of n² less n̄², cancels in half precision | rule 2 | rejected |
| — | Taps truncated at 3σ instead of 3.5σ | no, 2.7e-3 tail | parity | rejected: −15 % taps, not worth a choice |

### E. Weights from the CPU (exact)
The `Gaussian` uniform carries the reach's weights (`Taps::centre`/`next`, as the CPU reference already
computes them), up to reach 32 in `array<vec4<f32>, 9>`; the shader stops computing `detail_erf`. It
also serves the detail rows and columns (chroma denoise, sharpen). Gain: 34 erf (each an `exp` and a
polynomial) a pixel on the preview, 84 at 24 MP. Error: GPU and CPU erf differ by an ulp at most; the GPU then
matches the CPU reference closer than today. Item 4 of `docs/research/optimisation-plan.md`.

### A. Texture folded into the last blur (exact)
A stage `BlurThenTexture(gain)`: the columns blur of C₁, then `texture_px(load1(p), load2(p), ā b̄)`,
reading the source and N as auxiliary inputs. C is never written nor read: 14 frames instead of 16, one
dispatch less. Fits the graph as it is (`then_reading`, two aux, `aux_region` = output). Trade-off: a
Texture-slider notch today reruns pass 7 only (N and C are cached by the frontier cache); folded, it
reruns pass 6 too, about +0.3 ms on that one slider, against −0.45 ms on every notch upstream of Texture.

### B. One pass per blur, the rows in workgroup memory (exact)
A tile stage computes a 32 × 32 output block: each thread of a 16 × 16 workgroup sums the horizontal
taps for the block's columns over rows `y − reach … y + 32 + reach` (read with `load`, clamped to the
extent as today), keeps those sums in workgroup memory as `f32`, then sums the vertical taps from it.
The intermediate N₁, M₁, C₁ never reaches memory, and Texture becomes three passes:
1. narrow blur of luma → N (2 frames);
2. wide blur of moments(N) → M (2 frames);
3. wide blur of coeff(M), then `texture` with source and N (A folded in): 4 frames.

8 frames instead of 16 (341 MB on the preview, 3.12 GB on 24 MP), three dispatches instead of seven.
The horizontal sums over the apron cost (32 + 2·reach)/32 of the rows' fetches: 1.375× at reach 6,
2.06× at reach 17; the vertical taps read workgroup memory. Texture-cache fetches go from 65 to about
43 a pixel on the preview. Workgroup memory holds only the channels used (1 for luma, 2 for moments and
coefficients): (32 + 2·reach) × 32 × channels × 4 B, 11 KB at reach 6, 17 KB at reach 17, under the 32
KB Metal and DirectX 12 give (the graph already requests `adapter.limits()`). Above a reach of 24 the
two-pass path stays. The tap order and the `f32` storage are the two-pass path's, so the arithmetic is
the same; only the shader compiler's FMA contraction can move the last ulp.
`performance.md` notes workgroup memory alone does not help a separable blur under a reach of ~9; the
gain here is not tap reuse but the intermediate frame and the dispatch it removes.
Estimate with A: 1.83 ms on the bandwidth model, up to 2.6 ms if the apron fetches dominate; 24 MP
export from ~33 ms (bandwidth model, likely 40–60 ms with 35 taps) to about half.
Later, exact too: passes 1 and 2 in one tile (N computed with its own apron) save N's re-read, 1 frame.

### D. Fewer channels, same precision (exact, owner's call)
N as `r32float`, M and C₁/M₁ as `rg32float`. Values are the same `f32`, so the result is bit-identical;
only the letter of rule 2 ("images are rgba32float between every stage") changes. Graph cost: a format
per texture in the pool (keyed by size and format), the bind group layout keyed by (arity, output
format), `Stage::format()` defaulting to `Rgba32Float`; `textureLoad` already returns `vec4` and
`textureStore` drops the unused channels, so kernels are untouched. Alone: 132 B a pixel (3.65 → ~1.9
ms); with A+B: 76 B (→ 1.1–1.5 ms). Proposed wording if approved: "32-bit float per channel between
every stage; a stage whose output carries one or two values (luma, moments) may store r32float or
rg32float." No user setting: it is exact.

### C. Wide blurs at half resolution (opt-in "Fast")
The fast guided filter (He and Sun 2015): moments box-averaged 2 × 2 from the full-resolution N
(mean of n², never square of the mean), both wide blurs and the coefficients at half resolution, ā and
b̄ upsampled bilinearly (four `textureLoad`s, as `rgba32float` is not filterable) and applied to the
full-resolution N. Deviations corrected for the box and the upsampling so the total variance matches:
σ½ = √(σ² − 0.25)/2 for the moments, √(σ² − v_up)/2 for the coefficients, v_up measured. The half
grid is anchored on the image's extent, never on the requested region, or a pan shifts it and the
band shimmers. Stages: `BlurOf(Before::MomentsDown)` with `extent` = ceil(input/2) and `input_region`
2× outset, the half-resolution columns and coefficient blurs as today, `TextureUp` reading C at half
size (`aux_region` = output/2 outset 1).
Gain on top of A+B: the wide passes run on a quarter of the pixels at a reach of 3 (preview) or 9 (1:1),
cutting their fetches 4–8×; bytes 128 → 112 B a pixel. On the 2000 px preview that is only 0.3–0.5 ms
after A+B, since σ = 1.6 px is already near the pixel and halving it aliases (the Gaussian's response at
the half grid's Nyquist is 4 %). Its case is the 1:1 view: a ~2400 × 1600 region of a 6048 px frame
with 39 px margins, 35 taps a wide pass, estimated 8–14 ms for Texture today (past a 120 Hz frame),
roughly halved again by C after A+B.
C runs only where the wide radius is at least `FAST_MINIMUM_WIDE` (proposed 3 px, a long side of 3750
px or more, so 1:1 views and not the preview), unless §4's measure shows the preview within bounds too.

## 3. Error, and how it is measured

- **Exact techniques (E, A, B, D)**: against today's seven-pass chain on the GPU and the CPU chain
  (`oni_render::cpu` gaussian, `texture_moments`, `texture_coeff`, `texture`), with the metric of
  `tests/fused.rs`: |gpu − reference| / (1 + |reference|), expected ≤ 1e-6, asserted ≤ 1e-4 as the stage
  checks; at 780 px (narrow under its floor), 2000 px, 4000 px and 6048 × 64 px (reach 17), whole
  frames and regions touching each edge. "Identical within float tolerance" means that bound holds;
  then it ships with no setting.
- **C**: on the reference roll (`ONI_RAW_DIR`, two frames by default, the roll on demand), the whole
  pipeline, Fast against Exact, Texture at −1, 0.5, 1 and 2, at the preview and at a 1:1 region. Report
  max, mean and 99.9th centile of |Δ| on linear output, and in display code values (the 8-bit canvas
  encoding) plus mean ΔE2000. **Bound to offer it**: 99.9th centile ≤ 1 code value, max ≤ 3, mean
  ΔE2000 ≤ 0.3 at amount 1; amount 2 reported. A ×32 difference image is written to a scratch folder
  (never beside the RAWs) for the owner's eye.

## 4. The setting

Offered only if C passes §3's bound **and** saves at least 30 % of Texture at the 1:1 view after A+B
(rule 4: no setting before use proves the constant wrong). Model: `HALF_SIZE_PREVIEWS`.

- **Name**: "Texture on screen". **Values**: "Exact" (default), "Fast". **Help line**: "Fast works
  Texture's wide blur at half resolution while you edit. Exports are always exact."
- **Storage**: `preferences::FAST_TEXTURE = "fastTextureOnScreen"` (bool, absent = false, doc comment "a
  choice this build adds", as `halfSizePreviews`); `render::FAST_TEXTURE: AtomicBool` loaded in
  `app/shell.rs`, written by the action `fast-texture`, which stores, remembers and asks a canvas render
  (no decode; the frontier cache keeps every node upstream of Texture).
- **Where it applies**: `oni_pipeline::Request` gains `texture: TextureMode { Exact, Fast }`, default
  `Exact`. Only the canvas path (`Engine::resident` with a view, the loupe reading it) sets `Fast`.
  Export (`export.rs`), the measurer (`oni-measure`, so the automatic keys write the same values
  whatever the setting), contact sheets, roll previews and thumbnails build `Request::default()`, so
  they are exact by construction. The output histogram follows the canvas, as it describes what is
  shown.
- **Settings screen**: a gear key (`Icons.settings`, help "Settings (⌘,)") at the right end of the
  header (`toolbar.slint`), and "Settings…" ⌘, in the app menu, open a modal sheet built from the design
  system (ds containers, a two-value `ButtonGroup` per row, no literal sizes). One section, "Rendering":
  "RAW previews: Whole | Half size" (the existing `half-size` action, its menu item kept, help "Half
  size opens a RAW faster; fine grain looks softer.") and "Texture on screen: Exact | Fast". Escape and
  a click outside close it. The sheet exists with the RAW row alone if C does not pass its gate.

## 5. How the graph carries the variants

- **Cache keys**: a node's key hashes its input's key, its stage's name, shader text and parameters
  (`chain.rs`). Each variant has its own stage names and shaders (`gaussian-tile-moments`,
  `texture-up`...), so Exact and Fast results never share a key; switching reuses every node upstream
  of Texture and recomputes from there. No app-level cache holds a render past the screen except
  through `Request::default()`.
- **Tile stages** (B): `Stage` gains `fn entry(&self) -> Entry { PerPixel, Tile { block: [u32; 2] } }`,
  default `PerPixel`. For a tile, the prelude omits its `main`, the stage's WGSL declares
  `@compute @workgroup_size(16, 16) fn main(...)` writing through `textureStore(dst, ...)` within
  `frame.out_size`, and `dispatch` divides by the block instead of 8. Still one stage, one pass, one
  output texture. The shader's workgroup array is sized by a constant per reach class (≤ 8, ≤ 24), so
  at most two shaders per blur kind; `oni-graph/README.md` documents the entry kinds.
- **Formats** (D, if approved): see §2 D; `FloatTexture` keeps the only constructor, now taking one of
  three float formats.
- **Prewarm**: render a frame long enough for Texture to run: a 768 × 8 strip (wide 0.61 px, reach 3)
  and a 6048 × 8 strip (reach 17) cover both reach classes; with the setting, both modes. `Stats` gains
  `pipelines`, and the prewarm check asserts a busy notch after it compiles none.

## 6. Checks

- **CPU reference parity**: every new kernel has its scalar reference beside it in
  `oni-render/src/kernel/` (`gaussian.rs` for the tile blur, which reuses `gaussian` twice;
  `texture.rs` gains `texture_moments_down` and `texture_up`), and a GPU-against-CPU test on settings
  reaching every branch (each `Before`, both reach classes, the two-pass fallback, regions on every edge).
- **Twins**, each asserted to fail: B with its apron one row short (reach − 1); A reading N one pixel
  off; E with the weights shifted by one tap; D storing moments in one channel; C with nearest instead
  of bilinear upsampling and with the uncorrected σ/2 (both must break §3's bound, or the bound is too
  loose); the prewarm check against the 64 px prewarm (Texture compiles on the first notch).
- **Setting**: an export of a frame is byte-identical with Fast on and off (twin: the canvas render
  differs where C applies); `preferences.json` holds `fastTextureOnScreen` after the toggle; the window
  check opens the sheet from the gear, toggles both rows and closes it.
- **Bench**: `crates/oni-pipeline/tests/cost.rs`, `ONI_BENCH_COST=1`, release, 41 interleaved runs,
  medians. `texture_s_gpu_time_on_the_preview` prints each pass (already) and the chains: seven passes,
  A, A+B, A+B+D, A+B+C, at 2000 × 1333, 6048 × 4032 whole, and a 2400 × 1600 region of it (1:1 view).
  `each_feature_s_share_of_a_busy_render` gives the end-to-end figure recorded in each task.
- **Real app**: before saying it works, open the gallery, double-click into a frame of the reference
  roll, drag Texture and Exposure at fit and at 100 %, switch the setting, export, go back.

## 7. Plan, one commit each, each logged in `docs/tasks.json` before it

1. Bench: per-pass times and the 24 MP and 1:1 cases in `cost.rs`; record the baseline.
2. Prewarm reaches Texture (strips), `Stats::pipelines`, its check and twin.
3. E: weights in the uniform for `Gaussian` and the detail stages; checks; bench.
4. A: `BlurThenTexture`; parity at four sizes and on regions; twin; bench.
5. Graph: `Entry::Tile`, a toy tile stage checked against per-pixel, README.
6. B: tile blurs for narrow, moments and coefficients(+texture); reach classes and fallback; checks;
   bench; `docs/research/performance.md` and #153's figures updated.
7. D, only on the owner's word: formats in the pool and layouts, rule 2's wording in `CLAUDE.md`, the
   format check amended; bench.
8. C behind `Request::texture`: stages, CPU references, the roll measure of §3; bench at 1:1.
9. Gate: record C's error and gain; stop here if it misses §4's conditions.
10. Preference, atomic, `Request` plumbing, prewarm of both modes; export-identity check.
11. Settings sheet, gear key, ⌘, menu item, the RAW row moved in; window check; drive the real app.

## Summary of expected numbers (M2 Max, estimates until step 1 measures)

| Path | Texture, 2000 px | Busy render, 2000 px | Texture, 24 MP export |
|------|------------------|----------------------|-----------------------|
| Today (7 passes) | 3.65 ms (measured) | 6.96 ms (measured) | ~33 ms (bandwidth model), 40–60 likely |
| E + A | ~3.1 ms | ~6.4 ms | ~29 ms |
| E + A + B | ~2.0 ms (1.8–2.6) | ~5.3 ms | ~17–25 ms |
| + D (owner) | ~1.1–1.5 ms | ~4.5 ms | ~10–15 ms |
| + C (Fast, screen only) | unchanged at fit unless §3 allows | — | never (export exact) |
