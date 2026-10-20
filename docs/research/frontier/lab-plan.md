# Frontier Look Lab: implementation plan

Follows [recherche-frontier.md](recherche-frontier.md), [analyse-table-frontier.md](analyse-table-frontier.md)
and [neutre-et-frontier.md](neutre-et-frontier.md). **[M]** measured, **[I]** inferred. ΔE = Oklab distance × 100.

## 1. Goal

Make the Frontier machine on the Scanner face something the eye can steer toward the SP-3000 look it
remembers. Today it is a measured 27³ table that cannot be adjusted, and it renders blues that drift
toward violet and skin that turns a urine yellow where it should be golden. The Lab replaces the table
with a short chain of named parameters: a 3×3 dye matrix, a paper curve edited as a graph, a gain and
offset per channel, a hue and chroma stage per sector, a grey cast by lightness, and a "measured character"
blend that adds back what the parameters miss. Each parameter shows two reference values. "Neutral" is
the value that reproduces ONI Pro at rest (ΔE 0.32 median). "Frontier" is the value fitted to the 63
real machine renders. With every parameter on Frontier and the character at 1, the Lab renders exactly
what ships today. Nothing moves on its own: a frame without Lab values renders the measured table, as
in the Swift app.

## 2. The chain today

`Pipeline::image` → `framed` → `scanner::density` (`scanner_density` kernel: transmittance → density
→ minus the roll median → separation `S(0.6)` → × `1.3 / green window` → + neutral + click offset →
clamp to `reached` → normalised to the cube domain [−0.15, 2.6]) → `color_cube` (`frontier.cube`, 27³,
density → display sRGB code) → warmth look (17³) → `scanner_output` (sRGB code → linear Rec.2020,
Scanner Contrast) → the Scanner face's finishing (curves, spectrogram, saturation, vignette) → output
clip. Code: `oni-pipeline/src/scanner.rs`, `oni-pipeline/src/lib.rs::frontier`,
`oni-render/src/kernel/scanner.rs`. The Lab only replaces the `color_cube` input. Density, clicks,
warmth and finishing stay the same.

## 3. Diagnosis: where the violet blues and the yellow skin come from

**A. The table itself, not its input** [M, research §4]: against ONI Pro at equal lightness, the cube
turns scene blues **+14 to +20°** toward violet (a scene at 247° comes out at 259–264°, and 15–33 % of
blues pass 270°). It turns skin **+18°** toward yellow, with 0.006–0.017 less chroma and its red
component (Oklab a) divided by about 4. The input adds little: camera crosstalk +4°, dose 0.6 −2.8°,
gain, clamp and clip about 0°.

**B. Hue against chroma** (`teinte-vs-chroma.png`, measured renders and cube agree within 1–3°) [M]: at a
fixed scene hue, the rendered blue hue is highest at low chroma and falls about 25° as chroma rises
(scene 248°: ~277° at C 0.005, ~258° at C 0.03, ~251° at C 0.09). **The violet is worst on pale blues
(sky, blue shadows), which is where the eye looks.** Reds and oranges climb the other way: scene 42°
renders at ~83° and scene 58° at ~93° once C passes 0.03. Skin therefore lands in Oklab's yellow-orange
on the machine's own renders (light skin 46° → 97° on the Base render) [M].

**C. Which parameter carries it** (new probe for this plan, numpy on `frontier.cube`) [M on the cube, I
for the reference]. Setup:
- Model: the paper stage of §4, with the published Frontier and neutral values and a pivot at the green
  neutral density 0.7244. Against the cube it reaches ΔE 1.77 median / p90 4.03 / p99 6.66 on the 1 440
  nodes inside `reached` with a channel spread under 0.6, all of them touched by a measurement (2.20
  without the pivot).
- Reference: the neutral model shifted −0.363 density along the grey axis, so its grey prints at the
  cube's L 0.548.
- Samples: skin is reference h 50–66°, C 0.04–0.09, L 0.55–0.80 (273 samples). Blue is h 238–258°,
  C 0.06–0.14, L 0.40–0.70 (1 998 samples).

| | Reference | Cube | Frontier paper model |
|---|---|---|---|
| Skin hue / chroma | 58.8° / 0.069 | **69.5° (+10.6°)** / 0.070 | 66.1° / 0.058 |
| Blue hue / chroma | 246.2° / 0.120 | **258.7° (+12.0°)** / 0.148 | 261.5° / 0.155 |

Each row below moves one Frontier parameter to its neutral value inside the model and gives the hue
change (median):

| Parameter moved to neutral | Skin Δh | Blue Δh |
|---|---|---|
| Whole matrix | **−19.8°** | −7.6° |
| Matrix, green borrows red (+0.100 → −0.023) | **−12.4°** | −3.0° |
| Matrix, red borrows green (+0.047 → −0.181) | −6.1° | −1.4° |
| Matrix, green borrows blue (−0.023 → +0.041) | −1.4° | −3.9° |
| Paper offset B (−0.047 → −0.119) | +13.7° (opposite) | −2.5° |
| Paper offset R | −3.6° | +1.5° |
| Paper curve (Frontier S → neutral) | −1.8° (C −0.031) | **−8.0°** (C −0.068) |
| Paper gains R, B | 0° | 0° |

Reading [I]:
- **Yellow skin.** The Frontier's green row borrows red density, so a skin, rich in red density, lifts
  its green output. Skin goes yellow and its a falls. The paper offset of B pulls the other way, and the
  research's "offsets carry the skin" result reads the same shared gauge from the other side.
- **The chroma deficit.** The sector chroma gains at 30° and 60° (0.89, 0.95) add the missing chroma
  that makes it look like urine rather than gold.
- **Violet blues.** The steep per-channel S curve (1.4 code per density unit at neutral) pushes saturated
  blues toward the display's blue primary (about 264° in Oklab). The green row of the matrix adds −3 to
  −4°. The chroma-dependent turn (B above) is the sector stage's job.
- **The likely root cause** [I, research §4]: the table was learned on synthetic negatives without
  crosstalk or a real mask. A machine tuned for real film then "corrects" a bleed that is not there.
  Only a scan pair (the same frame on an SP-3000 and on a camera) would settle it, so the eye decides.
- **Rust-only unknown**: the LibRaw decode moves every RAW render against Swift's. Its share of the
  blue and skin shift on roll #8 has not been measured yet (step 4).

The Lab parameters that address each error:
- **Skin**: Matrix "green borrows red" first, then "red borrows green". After that, paper offset B, and
  sector chroma at 30° and 60°.
- **Blues**: sector hue and twist at 240°, 270° and 300°, then the paper curve's mid slope, then the
  matrix "green borrows blue".

A third-party Frontier emulator's shader, examined locally and not in the repository, has the same
skeleton: a density shaper, a 3×3 matrix, a shaper, a 32³ cube, a luma-indexed per-channel offset and a
mask-driven local shift. It also adds a **separate 65³ skin correction lattice**. Others needed a
dedicated skin stage too.

## 4. The model

Input `x`: the three cube-input densities after the clamp, before normalisation, so that
`scanner_density` is unchanged. Output: display sRGB code, like the cube.

1. **Matrix**: `x' = M x`. Each row sums to 1, so six off-diagonals are free.
2. **Paper**: `code_c = P(g_c (x'_c − p) + p + o_c)`. Here `p` = 0.7244 (green neutral), `g_G = 1` and
   `o_G = 0`. `P` is a 15-node curve over density −0.2 … 2.0, monotone cubic (Fritsch–Carlson), so the
   graph draws exactly what renders.
3. **Colour, in Oklab on that code**:
   - hue `h += hue(h) · min(C / 0.03, 1) + twist(h) · C / 0.1`;
   - chroma `C *= chroma(h) · (1 + satL · (L − 0.5))`;
   - then `a += greyA(L)` and `b += greyB(L)`.

   The sector values interpolate periodically between 12 centres (0°, 30° … 330°, the output hue). The
   grey cast is piecewise linear over L 0.1 / 0.3 / 0.5 / 0.7 / 0.9.
4. **Character**: `code += k · R(x)`, with `R = cube − model(Frontier)` on the cube's own 27³ nodes. At
   Frontier values and `k = 1` the bake equals the shipped cube at every node, so opening the Lab moves
   nothing. At `k = 0` it is the pure model.
5. Clip to [0, 1].

**Render path: bake, no new kernel.** Evaluate the chain on the CPU at the 27³ nodes of the cube domain
(19 683 evaluations, about 1 ms in release) into a `ColorCube`, and hand it to the existing `color_cube`
kernel (it has its CPU reference and test already). Bake on the render worker, keep one baked cube keyed
by the Lab values, and re-upload only when they change. No new WGSL means no new GPU/CPU pair to keep in
step.

The values of the sector, saturation-by-L and grey-cast terms came from a session script that is not in
any repository, and their exact formulation (ramp, pivots) is not recorded. They are seeds: step 2
refits them with the formulation above, in a tool kept in the repository.

## 5. Parameters

Sectors are output Oklab hues: 0° pink-red, 60° orange-skin, 90° yellow, 150° green, 240° blue, 270°
blue-violet, 300° violet. Frontier and neutral values do not share a gauge one by one; only the whole
set is comparable. The input stage (dose 0.6, window gain 1.3) stays constant: it is the same in both
looks and moves hue by less than 3°.

| Stage (where it acts) | Parameter | Neutral | Frontier | Unit, range |
|---|---|---|---|---|
| Matrix (cube-input density) | Red borrows green, red borrows blue | −0.181, −0.008 | +0.047, +0.024 | density, ±0.3 |
| | Green borrows red, green borrows blue | −0.023, +0.041 | +0.100, −0.023 | idem |
| | Blue borrows red, blue borrows green | +0.053, +0.063 | +0.011, +0.233 | idem |
| Paper (density → code) | Curve: 15 nodes at density −0.200 … 2.000, step 0.157 | 0, 0, .139, .341, .519, .659, .771, .860, .929, .984, 1, 1, 1, 1, 1 | .001, .001, .015, .059, .129, .272, .475, .680, .825, .906, .950, .974, .986, .998, 1 | code 0–1, monotone |
| | Gain R, B relative to green | 1.088, 0.947 | 1.090, 0.951 | ×, 0.8–1.2 |
| | Offset R, B relative to green | −0.025, −0.119 | −0.050, −0.047 | density, ±0.2 |
| Colour (Oklab of the code) | Hue offset, 12 sectors | +0.5 +0.6 +0.8 +0.3 +0.7 +1.9 +1.1 −2.1 −3.8 −1.8 +1.0 +1.3 | +0.4 −0.2 −4.7 −0.6 +10.5 +13.0 +2.2 −8.7 −11.2 −10.7 −10.3 −5.8 | degrees, ±20 |
| | Twist per 0.1 of chroma, 12 sectors | −0.8 −0.4 +0.4 +0.4 +0.1 +0.1 −0.1 −0.9 −0.8 +0.3 +1.0 +0.1 | −3.9 +0.1 +5.9 +2.3 −4.3 −1.2 +7.8 +11.2 +9.8 +10.5 +9.7 +2.3 | °/0.1 C, ±15 |
| | Chroma gain, 12 sectors | 1.00 1.00 1.01 1.03 1.04 1.06 1.08 1.07 1.01 0.95 0.94 0.98 | 0.96 0.89 0.95 1.00 0.91 0.83 0.97 1.14 1.12 1.00 1.00 1.03 | ×, 0.5–1.5 |
| | Saturation by lightness | +0.02 | +0.06 | per unit of L, ±0.3 |
| Grey cast (Oklab, after colour) | a at L 0.1 / 0.3 / 0.5 / 0.7 / 0.9 | +.005 +.004 +.002 +.003 +.002 | +.009 +.002 −.009 +.002 +.004 | ±0.03 |
| | b at the same L | +.001 −.003 −.001 −.002 −.001 | +.007 +.004 +.011 +.012 +.004 | ±0.03 |
| Character (added to the code) | Measured character | 0 | 1 | 0–1 |

That is 73 values. The sector values are seeds until the step 2 refit. Left out on purpose:
- Gradation and global saturation: the Scanner face's Contrast and finishing already cover them.
- Hypertone and Hyper-sharpness: local operations that no table measures. They need research of their
  own.
- Dose and window gain: they stay constants until the Lab proves them wrong.

## 6. The paper curve as a graph

- **Plot**: the DS `Plot` (`ds/plots.slint`), laid out by Rust like `editors/curve.rs`. The x axis is
  cube-input density −0.2 … 2.0 (green, after the matrix) and the y axis is display code. `AxisCaptions`
  carries the units.
- **Nodes**: 15 nodes at fixed density. A drag moves a node vertically only, between its two neighbours,
  so the curve stays monotone and keeps the measured table's sampling. The line is the Fritsch–Carlson
  spline the bake uses.
- **Reference curves** (`PlotLine`): the neutral curve in grey and the Frontier curve in its own ink,
  drawn under the live one. A vertical guide marks the neutral density (0.7244), as in
  `courbe-papier-neutre-frontier.png`.
- **Gestures**: a double-click on a node returns it to its Frontier value, the Lab's rest. Arrow keys
  step the focused node by 0.005 code. Each gesture is one undo step through the command layer.
- **Code reuse**: the curve editor's `nearest`, `screen` and `normalized`. Adding and removing points
  does not apply here.

## 7. Interface

- **Where**: on the Scanner face, in the Color tab's Machine section, a **Look Lab** key appears when
  Frontier is picked. It swaps the panel's content for the Lab, with a **Done** key and Esc to return.
- **Why not a sheet**: the canvas stays live beside it, because the eye judges the image while the hand
  moves. A `Sheet` would cover the canvas.
- **No Settings toggle**: a key that changes nothing until pressed decides nothing. Hiding it is the
  owner's call, if the owner asks for it.
- **Name**: the Swift "Lab" is the keypad screen (`app/lab.rs`). The new code is named `look_lab`
  everywhere.
- **Sections** (DS `Section`, folded state remembered like the other blocks):
  - **Matrix**: six `StoreSlider`s titled by what they do ("Green borrows red").
  - **Paper**: the graph, then four sliders.
  - **Colour**: twelve sector tabs (the `SpectrogramEditor` pattern: tabs, then the chosen sector's hue,
    twist and chroma sliders), plus saturation by L.
  - **Grey**: five lightness tabs, with a and b for each.
  - **Character**: one slider.
- **Neutral and Frontier on every parameter**:
  - **Slider marks (DS)**: `Slider` gets `marks: [SliderMark]` (`{ fraction, ink, label }`), two ticks on
    the track, "N" in `SliderStyle.mark-neutral` and "F" in `SliderStyle.mark-frontier`. Both are new
    `theme.slint` values, commented as having no Swift token. A click on a tick sets that value in one
    undo step. A double-click on the track returns to Frontier.
  - **Section header**: each section gets a `ButtonGroup` with **Neutral** and **Frontier** keys that set
    the whole stage, and the top of the Lab gets the same pair for everything.
  - **Readouts**: they show the value and, dimmed, the distance to each reference.
  - **Plot inks**: `Ink.reference-neutral` and `Ink.reference-frontier` for the graph's reference
    curves.
  - **Catalogue**: every new piece appears in `ds/catalogue.slint`, and `tests/design_system.rs` stays
    green with an empty `strict::ALLOW`.
- **Commands**: the knobs are addressed like the Spectrogram's bands in `oni-core/src/knob.rs`, so the
  keyboard, the phone and undo reach them the same way.

## 8. Storage

- **Settings**: `PipelineSettings.frontier_lab: Option<FrontierLab>`, serialised as `"frontierLab"`
  (camelCase like every Swift key). Absent, it renders the measured table, as in Swift.
- **Sidecar keys**: `matrix` [6], `paper` [15], `paperGain` [2], `paperOffset` [2], `hue` [12],
  `twist` [12], `chroma` [12], `saturationByL`, `greyA` [5], `greyB` [5], `character`.
- **Reading**: the reader is tolerant like `ScannerClicks`. Every absent or unreadable key takes its
  Frontier value, a value past its range lands on its stop, and an array of the wrong length is dropped.
- **Version**: `SettingsCodec.version` stays 19. Swift's own rule is that a field that is only added
  never bumps it.
- **Swift compatibility**: Swift's `Codable` ignores the unknown key, so a roll opens in Swift on the
  measured table. If Swift saves the frame, the key is lost and the frame falls back to the measured
  table. That is acceptable and recorded in `docs/ui-spec.md`. The Swift app is frozen, so it gets no
  change.
- **Paste and presets**: `"frontierLab"` joins the `scanner` row's `covers` in
  `oni-core/src/parameters.rs`, so paste, sync to the roll and presets carry it with the clicks, and
  `Field::FrontierLab` joins `oni-core/src/fields.rs`.
- **Fitted values**: the neutral and Frontier sets ship as `oni-pipeline/resources/scanner/lab.json`
  next to `frontier.json`, written by the fit tool and never by hand.

## 9. Checks

Every numeric check has a twin, a wrong variant asserted to fail.

- **Paper model against the cube**: the paper-only model at Frontier values against the cube on the
  constrained nodes (inside `reached`, channel spread under 0.6) has a median ΔE of at most 2.0 (probe:
  1.77). Twin: the identity matrix with the gains and offsets made common goes above 2.5.
- **Full model against the cube**: the full model at Frontier values has a median of at most 1.7 on the
  same nodes. Twin: without the Oklab stage it is worse by at least 0.15.
- **Character**: at Frontier values and `k = 1` the bake equals `frontier.cube` within 1e-5 at every
  node. Twin: `k = 0` is above 1 ΔE median.
- **Bake against the analytic chain**: p99 ΔE under 0.5 on 100 000 random inputs inside `reached`.
  Twin: a 9³ bake fails it.
- **Neutral against Pro**: after the refit, the model at neutral values is within ΔE 0.5 median of ONI
  Pro at rest, composed from the existing CPU kernel references on a synthetic density grid. Twin: the
  Frontier values fail it by far.
- **Diagnosis kept as a test** (`look_lab::tests`): the probe of §3C on the skin and blue sets. The cube
  turns skin and blue by more than +5° against the neutral reference; the model with "green borrows red"
  at neutral turns skin by more than 5° back. Twin: the reference against itself is 0°.
- **Absent field**: Frontier renders of roll #8 (two frames, `oni-pipeline/tests/roll8.rs`) are
  bit-identical to today's. Twin: a Lab at neutral values moves them.
- **Sidecar**: round trip; absent keys read as Frontier; a Swift-written fixture without the key reads
  unchanged; malformed values are tolerated. Twin: a key past its range is clamped, not passed.
- **Performance**: a release bench of the bake stays under 2 ms. One slider notch costs one bake, one
  upload of 315 KB and the cube onward, measured with `ONI_BENCH=1` like #106.
- **Window**: open the Lab, drag a slider (the canvas moves), click an N tick, undo, Done, and the
  gallery round trip. Then drive the real app on roll #8 as CLAUDE.md requires before saying it works.
- **Reference roll by eye**: a strip of roll #8 frames, from the cube, from the Lab at Frontier with
  `k = 0`, and from the Lab at neutral values, with hue and chroma statistics on a hand-picked sky region
  and skin region. The owner's eye is the last check.

## 10. Steps

Each step is one commit logged in `docs/tasks.json`. Steps 1 to 4 change no render and no file of the
user.

1. **Paper model and bake (one day).** Done when all four tests pass:
   - New `oni-pipeline/src/look_lab.rs` with `LabParams` (matrix, paper curve, gains, offsets),
     `NEUTRAL` and `FRONTIER` constants from §5, and `paper(x) → code` (Fritsch–Carlson spline).
   - `bake(&LabParams, n) → ColorCube` over `MACHINE`'s domain.
   - The tests: paper model against the cube (with its twin), bake against the analytic chain (with its
     twin), the §3C diagnosis test, and a test of both pivots that keeps the one passing (0.7244
     expected).
   - No settings, no UI, no render change.

   Measured (#167) [M], on the 1 440 constrained nodes:
   - Paper model at the published Frontier values: median ΔE **1.67** (pivot 0: 2.07). Twin, identity
     matrix with common gains and offsets: 3.47.
   - 27³ bake against the chain on 100 000 inputs inside `reached`: p99 ΔE **0.34**. Twin, 9³: 2.72.
   - Diagnosis, on a lattice four times finer than the cube's inside `reached` (199 skin and 1 886 blue
     points; the neutral reference slides −0.367 along grey): the cube turns skin **+10.6°** and blue
     **+12.5°**; "green borrows red" at neutral turns skin −12.5°. The probe of §3C is reproduced.
2. **Colour stage, character, refit.**
   - The Oklab stage and `R`, with the full-model and character checks.
   - `cargo run -p oni-pipeline --example look_lab_fit` (Levenberg–Marquardt on the constrained nodes)
     writes `lab.json`, and `LabParams` loads it.
   - The fit's ΔE goes into the doc beside this plan.

   Measured (#168, refitted in #169 with values on their stops held) [M], on the 1 440 constrained
   nodes against the cube:
   - Published seeds: paper only 1.67, full model 1.51 median.
   - **Fitted Frontier, full model: median ΔE 1.19** (p90 2.99, p99 5.40). The same set with its colour
     stage at rest: 1.95 (twin). The fit is against the cube's nodes, not the 63 renders, hence below the
     research's 1.65.
   - The twist at 210° sits on its −15 stop, beside a +17.8° hue offset: they cancel to +3° at C 0.1.
     Saturation by L lands at −0.16, the seed's sign reversed. The colour stage and the paper share a
     gauge, so single values do not compare with the seeds.
   - Character: at the fitted Frontier values and `k = 1`, the 27³ bake equals `frontier.cube` at every
     node (worst gap 0). The clip applies to the model alone: the cube's nodes dip to −0.11 where it
     extrapolates, and the character keeps them. At `k = 0`: 1.19.
   - **Open for step 6**: the full model's 27³ bake misses the bake target, p99 ΔE **0.59** against the
     chain (the paper chain alone: 0.34). 33³ gives 0.41, but only a 27³ bake keeps the character exact.
3. **Neutral refit against Pro**: a synthetic density grid, rendered by Pro at rest from the existing
   CPU references, refits the neutral set, with its check.

   Measured (#169) [M]:
   - Grid: `scanner::check_roll`'s balanced roll, negatives at its grey ±0.75 density by 0.05 per
     channel, kept where the Frontier reads them unclamped with a cube-input spread under 0.6 (9 948).
     Pro at rest: `stage47`, the points' saturation, the output clip, then sRGB code. Window gain 0.87,
     where the research's roll had 0.81; Pro prints the grey at L 0.76.
   - Published neutral seeds against it: 5.61 median (a different roll and gauge).
   - **Refitted neutral: median ΔE 0.19** (p90 0.40, p99 0.80); colour stage at rest 1.67. Twin, the
     Frontier set against Pro: 15.2.
   - The neutral matrix mixes (every off-diagonal but one positive, B←G +0.26), and the sector chroma
     gains climb to 1.1–1.5 (one on its stop at 180°) to win the chroma back. Matrix and chroma share a
     gauge here. The N ticks will sit far from 0 and 1, so step 7 should weigh a light pull toward
     identity before they ship.
   - The fit now holds a value that sits on its stop while the descent pushes it out. Without that, both
     fits stalled early (neutral 0.31).
4. **Eye check before any UI**: `--example look_lab_strip <raw dir> <out>` writes PNG strips of two
   roll #8 frames: cube, Lab Frontier `k = 0`, Lab neutral, and Lab Frontier with "green borrows red"
   and the 240–300° sectors at neutral. The same tool measures the Rust (LibRaw) Frontier render
   against Swift's on the same frames to size the decode's share. The owner looks before the interface
   is built.

   Measured (#170) [M], frames DSC_6432 (sky, a blue roof) and DSC_6442 (a face, a hand, sky) of the
   test copy of roll #8, LibRaw decode reduced to 604 px, the Frontier stages on the CPU (they match the
   app's pipeline on the same pixels to under 1e-5). A fifth panel puts "green borrows red" at the
   published seed's neutral (−0.023) instead of the refit's. Region medians, hue (chroma):

   | Region | Table | Lab F, k 0 | Lab neutral | F, G←R + 240–300° neutral | idem, seed G←R |
   |---|---|---|---|---|---|
   | 6432 sky | 262.1° (.069) | 263.5° (.065) | 251.9° (.031) | 254.5° (.086) | 252.3° (.084) |
   | 6432 roof | 263.0° (.102) | 261.2° (.100) | 250.3° (.039) | 255.6° (.129) | 254.3° (.126) |
   | 6442 face | 63.0° (.039) | 80.0° (.039) | 47.5° (.017) | 69.2° (.039) | 56.4° (.039) |
   | 6442 hand | 45.4° (.027) | 58.8° (.021) | 32.4° (.011) | 51.8° (.026) | 39.2° (.028) |
   | 6442 sky | 266.0° (.050) | 266.1° (.048) | 255.0° (.025) | 259.2° (.062) | 257.8° (.061) |

   - **The refit moves the meaning of "green borrows red"**: refitted, Frontier lends +0.046 and neutral
     +0.113, so its N tick turns skin 6° *more* yellow. At the seed's −0.023 it turns skin 7° redder, as
     the diagnosis says. Single values do not carry meaning across the two refitted sets (§5's warning),
     and the N ticks of step 7 need a gauge fixed before they ship.
   - The 240–300° sectors at neutral take 7–9° of violet off sky and roof, with 25 % more chroma.
   - Lab Frontier at `k = 0` keeps sky and roof within 2° of the table, but turns skin 14–17° more
     yellow: the model misses the table's low-chroma warm hues, which the character carries.
   - By eye: the table and `k = 0` look alike, `k = 0` a shade colder on the white walls. The neutral
     set is light and flat (Pro at rest prints grey near L 0.76), its skin pinker. The two moved panels
     give a deeper, less violet sky and a cyan-blue roof; the skin change is hard to see at this size.
   - **Rust (LibRaw) against Swift on the Frontier**, Swift's own `--render` at 1000 px
     (`scripts/frontier-swift-dump.sh`): mean worst-channel gap 0.019 and 0.027, p99 0.071 and 0.076.
     The pipeline alone sits near 1e-3 on Swift's decode (`roll8.rs`), so this is the decode's share.
     Rust's skies are **4.3° and 5.8° more violet** than Swift's (262° against 258°, 266° against 260°),
     with 10–15 % less chroma; the face's hue matches (62.0° against 61.5°) with more chroma (0.039
     against 0.023). The violet the owner sees in the Rust app is partly LibRaw's.
5. **Settings and sidecar**: `FrontierLab`, the codec, `fields.rs`, the `parameters.rs` row, and the
   sidecar tests. This is adversarial-review territory (the user's files).
6. **Render path**: `Pipeline::frontier` bakes when the field is present, keeps one baked cube, and
   prewarms it (#111). Add the roll8 identity test and the bench.
7. **DS pieces**: `SliderMark`, the reference inks, the mark styles, and catalogue entries.
8. **Lab panel**: the Look Lab key, the sections with sliders (no graph yet), the N/F keys, the knob
   addressing and the window check.
9. **Paper graph editor** (§6), with its window check.
10. **Owner session on roll #8**: findings become values the owner names. Nothing is applied by default,
    and a "Frontier (ONI)" set ships only on the owner's word.
11. **Later, each only once use asks for it**: dose and window-gain sliders, a Lab set saved as a preset,
    and Hypertone and Hyper-sharpness (their own research first). Lab rows go into a new
    `docs/parity/beyond-swift.md`, counted apart from parity in its README.

## 11. What the measurements cannot settle

- What a real SP-3000 does to real film: no measured render contains real film. "Golden" skin is
  neither reproduced nor refuted, only the cube's drift.
- How the measured machine was set up (Hypertone, gradation, saturation). A scan pair (the same frame on
  an SP-3000 and on the owner's camera) would turn the eye's call into a measurement.
- Beyond about two steps from the measurements (saturated cyans and yellow-greens at gain 1.3, dark
  blues), the cube and the model are both guessing. The Lab says so next to the sector tabs.
