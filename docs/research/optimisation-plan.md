# Extreme optimisation: the plan

The goal: every parameter slides at a sustained 30 fps image with no spike on an 8 GB M1, so the M2 Max
has room to spare; a frame opens in a blink, zoom and pan never hitch, the interface never waits. Each
step is measured before and after, in the real window or on roll #8, and logged in `docs/tasks.json`;
a change that does not move its number is reverted.

The M1 is the budget: about 68 GB/s of memory bandwidth against the M2 Max's 400, an 8-core GPU against
38, 8 GB shared with macOS and the GPU. Every rgba32float pass over a 2000 × 1333 frame moves 85 MB, so
bandwidth, not arithmetic, decides most costs.

## Where the time goes (24 MP NEF, M2 Max, release)

| Step | Before | Now | Task |
|---|---|---|---|
| Notch to screen, p50 | 33 ms | 4-9 ms | #89 |
| One notch's render, light frame, GPU waited | 4.7 ms | 2.4 ms | #106 |
| Texture / Sharpen notch, heavy frame | 10.4 / 10.5 ms | 2.6 / 2.2 ms | #106 |
| Vignette / Exposure notch, heavy frame | 10 / 10.7 ms | 6.7 / 7.2 ms | #106 |
| CPU side of a notch's render | 1.2-2.5 ms | 0.3-0.7 ms | #106 |
| Levels + curves inputs after a notch | 40 ms, every notch | 4 ms under the hand, 0 past the levels, 16 ms at rest | #105 |
| Auto White Balance press | 420 ms | 40 ms (copy made at open) | #100 |
| LibRaw full decode / half-size | 980 / 345 ms | same | #96 |
| Frame to histogram, p50 | 6-7 ms | 5.5 ms | #93 |

A heavy frame here carries vignette, Chrome, sharpen, texture and chroma denoise. On an M1 the GPU
lines are to be multiplied by roughly five: the heavy Exposure notch, about 35 ms, is still over budget.

## The pipe, stage by stage

A notch travels: Slint's pointer event → `on_drag` (the 30 Hz hand pacing) → the store's `apply` →
`publish` (every panel's properties) → `request_render` → the render slot → `Engine::resident` (the
recipe, the graph's plan, one dispatch per stage) → the owned texture copy → the UI thread's display
kernel → Slint's frame; then the histogram count, its readback and its trace; then the inputs reading.

Stages in graph order, each a full read and write of its region:

| Stage | Passes | Bound by | Parameters that start here |
|---|---|---|---|
| Capture exposure | 1 | bandwidth | capture stops |
| Flat-field | 3 | bandwidth | flat amount, temperature, tint (last pass only) |
| Geometry | up to 4 | gather | crop, angle, quarter turns, mirrors |
| Dust corrections | 5-8 each | blur | strokes |
| `stage47` | 1 | ALU, up to ~250 transcendentals a pixel | temperature, tint, density, exposure, highlights, shadows, contrast, gains, levels, black and white points, dye separation |
| Look (ONI / Frontier) | 3-4 | bandwidth + cube | machine |
| Curves | 1 + 2 LUT | bandwidth | manual curves, curve contrast |
| Spectrogram | 1 | ALU (8 bands) | colour bands |
| Points and hand saturation | 1 each | bandwidth | saturation curve, black/white points |
| Vignette | 1 | bandwidth | vignette |
| Detail (denoise + sharpen) | 2 | taps (`erf` per tap) | chroma, sharpen amount, radius, halo |
| Texture | 10 | bandwidth | texture |
| Chrome, luma, output clip, proof | 1 each | bandwidth | Chrome, paper R/G/B |

## 1. The render graph (the M1's first wall)

1. **Done: the frontier cache** (#106). A notch reruns only its stage and what follows. Nothing is left
   to gain for sliders late in the chain; the early ones (`stage47`'s whole family) still run the tail.
2. **Fuse the per-pixel runs into one pass**: `stage47`, curves' lookup, spectrogram, the saturations,
   vignette, Chrome, luma, the output clip and the matrix proof each expose a pure `*_px` function; a
   `Fused` stage concatenating them with one packed uniform turns 6-9 passes into 1-2. On an M1 that is
   the difference between ~35 ms and ~10 ms for an Exposure notch on a heavy frame. The CPU references
   stay per stage; a fused GPU-vs-CPU check per combination proves it. Keep a cache boundary after
   `stage47` so finishing sliders still start late.
3. **Tables instead of transcendentals in `stage47`**: `shaped(n)`, the hunt and follow terms are scalar
   functions of one channel under uniforms; 4096-entry tables built per settings change, with the CPU
   reference reading the same tables and a twin bounding the interpolation error.
4. **Blur weights computed once on the CPU** (Gaussian, detail rows and columns), and tap loops reading
   through workgroup memory with a per-stage workgroup shape: 20-40 % of every blur, up to 3× at large
   radius.
5. **Texture's ten passes to seven**: the narrow blur writes `(n, n², 0, 1)`, the moments pass goes, the
   coefficient folds into the first wide blur.
6. **No allocation per dispatch**: one ring of uniform buffers written with `queue.write_buffer`, bind
   groups cached per texture pair, a shader id instead of hashing the WGSL text on every node.
7. **No owned copy per notch**: the last stage writes into a ring of two canvas textures the display
   kernel reads, instead of a 43 MB allocation and copy each time.
8. **Pipelines prewarmed** on a background thread at launch, so the first drag never compiles Metal.

## 2. The interface thread

1. **`publish` does only what a notch changes**: menus, Lab, framing, export menu and the pasteboard
   query run every notch; `is_modified` builds 13 rest settings a notch; settings are cloned about
   eight times. Each moves behind a check of what it reads. (Dust rows and marks: done, #104.)
2. **The histogram trace off the interface thread**, the saturation plot only while its tab shows, the
   2 ms readback poll replaced by the readback's own wake-up.
3. **Deliveries coalesced**: one pending render slot on the UI side, so a busy frame never queues
   stale renders.
4. **Sidecar writes off the interface thread**, one writer, reads of a pending path answered from
   memory (touches the user's files: adversarial review first).
5. **The column and the grid windowed**: only the rows and tiles in view are instantiated.

## 3. Pacing and latency

1. Done: one render in flight, one waiting, latest wins; the inputs read on the small copy under the
   hand and at rest on the full one (#105).
2. **The hand's 30 Hz released by the render's landing**, not a free-running timer, removing up to
   16 ms of mean added delay without exceeding one render in flight. Changes the pacing rule:
   owner's call.
3. **Thumbnails and the loupe pause under the hand**, re-checked right before they take the engine.

## 4. Decode and opening

1. **Half-size reads for the editor's preview and the thumbnails**: 650 ms an open, pixels moved by a
   hair: owner's call.
2. **LibRaw's demosaic and the camera-to-linear conversion on every core** (~1 s → ~0.3 s, to measure).
3. **A preview cache on disk** per fingerprint, and the neighbours' previews prefetched once the hand
   rests, so walking a roll is instant.
4. **Full resolution built in the background** once the frame is idle, not at the first zoom.

## 5. Memory (8 GB)

Steady state at fit is about 1.25 GB, a cold open of a 24 MP frame peaks near 3.2 GB (45 MP: 4.9 GB).

1. **One gate for every full decode** (thumbnail painter, dust heal, export join the three gates):
   −0.8 GB of peak at 24 MP.
2. **LibRaw's buffers freed before the f32 copy**, and staging flushed band by band: the decode peak
   780 → ~540 MB, the upload's second copy gone.
3. **The thumbnail memory bounded in bytes** (~200 MB) with recency refreshed on use: −350 MB steady on a
   2,000-photo library.
4. **The full-resolution source kept as packed 16-bit** and expanded to rgba32float only for the visible
   region: −244 MB at 24 MP, −455 MB at 45 MP, lossless.
5. **The texture pool under a byte budget**; the full-resolution budget read from the device.
6. Prepare the Roll's 360 px sources capped; the contact sheet composed tile by tile; export streamed.

## 6. Measuring all of it

1. The notch probe (render with the GPU waited, light and heavy frames, per slider) becomes a bench.
2. The bench report gains resident memory (start, peak, after leaving the editor) and the pool's bytes.
3. A walk bench and a zoom bench with frame-time centiles and the worst gap.
4. An M1 run of every bench before the milestone closes.
