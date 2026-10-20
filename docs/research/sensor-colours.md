# Sensor colours: negatives without the camera matrix

Measured on 2026-10-20 (#181) on roll #8 (Nikon Z f, 38 NEF) and the Kodak ColorPlus roll shot on a Leica M6 and
digitised on a Lumix S1R (38 RW2), both on copies with the Swift sidecars. A prototype only: no setting, no
interface, and the app's decode is unchanged (`Colours::Matrix` is still the only path the app calls).

## Short answer

- **What it is.** LibRaw demosaics exactly as today, on the same 5000 K multipliers. Then the 3×3 camera
  matrix (camera to XYZ, Bradford to D65, to Rec. 2020) is replaced by a diagonal. Every channel stays the
  sensor's own and nothing mixes. A 5000 K grey keeps today's level, so it reads (1, 1, 1)·2^baseline in
  both modes.
- **Choose the 5000 K diagonal, not "no balance".** Once the roll is prepared, the two render the same: 0.09
  ΔE×100 on average, 0.3 at most on a frame. Gains absorb a diagonal, and only the pedestal tells them apart. At rest, unprepared, the
  balanced variant looks like today's render, while the unbalanced one changes with the camera's channel
  sensitivities.
- **Neutrals.** Measured against the Swift app's render, sensor mode is a little better on the Z f (2.2 → 1.9
  ΔE×100 on the Pro face). It is clearly better on the S1R (2.9 → 1.7): most of the S1R's yellow lean comes
  from its Adobe matrix.
- **Colours.** They move away from Swift, and the eye sees it.
  - Foliage turns yellow-olive and loses saturation (Z f Pro −35° and ×0.70).
  - Sky loses chroma (×0.51).
  - Skin moves toward red (−15° on the Z f Pro face).
  - On the Frontier face of the Z f, the sky moves away from violet and the skin away from yellow. Those are
    the owner's two complaints about the Frontier, but only on this camera.
- **Blacks.** Unchanged. They depend on the base pick, not on the matrix.
- **Gamut.** Tamer. The decode never goes below zero, and fewer pixels fall outside sRGB on the Pro face
  (Z f 10 % → 3 %, S1R 28 % → 17 %).
- **Across cameras.** Slightly closer to each other, but each camera is 4 to 5 times further from a
  colorimetric reading in blue.
- **Verdict: a look, not a correction.** It is coherent and cheap to build. It suits a per-roll choice, with
  the matrix staying the default.

## 1. The mode (`oni-decode`, `Colours`)

`decode_in(path, gain, side, Colours)` sits beside `decode_for`, which calls it with `Colours::Matrix`.
`converted_by` builds the matrix that follows LibRaw's 16-bit output:

| Colours | After the demosaic | A 5000 K grey at the white level reads |
|---|---|---|
| `Matrix` (the app) | `Rec2020 · Bradford(5000 K→D65) · cam_xyz⁻¹ · undo(mul)` | (1, 1, 1)·2^b |
| `Sensor` | `diag(hi/lo · 2^b)`: the 5000 K balance alone | (1, 1, 1)·2^b |
| `SensorUnbalanced` | `diag(n_c/lo · 2^b)`: the photosites' counts | Z f (0.61, 1, 0.76), S1R (0.52, 1, 0.68) |

- **LibRaw's input is the same in all three.** The multipliers, highlight mode and white level do not
  change, so the demosaic is the same and hard rule 3 holds.
- **The camera matrix is read only to find the 5000 K neutral.** Sensor mode drops what the matrix mixes.
  Here are the white-normalised matrices from the balanced camera channels to Rec. 2020:

```
Z f   0.907  0.237 -0.143      S1R   0.791  0.272 -0.063
     -0.050  1.358 -0.308           -0.019  1.090 -0.071
     -0.006 -0.251  1.257            0.013 -0.173  1.160
```

- **Tests** (CPU, each with a twin):
  - `the_sensor_s_channels_mix_nothing_and_its_neutral_keeps_the_matrix_s_level`. Twin: a saturated colour
    is read differently through the matrix.
  - `the_unbalanced_sensor_reads_the_photosites_share_of_the_white_level`. Twin: unbalanced, the 5000 K
    grey is not grey.

## 2. Method

- **Prepare the Roll, as the app runs it.** `oni-measure/examples/sensor_colours.rs` builds a `Measurer`
  whose decode is `decode_in` at `PREVIEW_SIDE`, in each of the three colours. It then:
  - measures every frame with `key::roll_measured` from `roll::start(sidecar, film, window)`;
  - reads the base with `key::base`;
  - combines with `roll::combined`;
  - lays the result with `balance::written`.
- **Two ways to lay the roll.**
  - **look**: the roll measured again, laid under each frame's own edits (print, curves, filters, face).
    This is what switching a roll would do.
  - **prepared**: laid at rest, as on a freshly prepared roll.
- **Check of the harness.** In matrix mode it lands on the roll the owner prepared in the Rust app at 13:36
  today to 1e-6 stop, the same code on the same decode. Against Swift's reference it sits R +0.091,
  B −0.098 stop away, the decode gap #165 found.
- **The base pick.** No sidecar records where the base was picked: Swift drops the draft once a roll is
  measured. The pick is found again by searching the rebate band (7 % of each side) of every frame for the
  disc that reads the Swift reference's base.
  - Z f: on DSC_6432 at (0.417, 0.958), 0.0022 of window away.
  - S1R: on P1071951 at (0.969, 0.240), 0.0008 away.
  - **The band matters.** A first search over the whole frame matched a point inside a picture, which only
    looks like bare film on one decode.
- **The renders.** For each roll, 5 frames:
  - Z f: 6432 landscape, 6439 and 6442 street, skin and greys, 6458 skin and sky, 6468 saturated reds;
  - S1R: 1943 foliage, 1949 skin, sky and foliage, 1959 stone neutrals, 1965 saturated reds and pinks,
    1978 skin and greys.

  Each is rendered at 1000 px, `per_pixel_only`, on the Pro face and on the Frontier, through the real GPU
  pipeline.
- **Swift's render.** The installed Swift app's `--render` on clones, with the face set in each copy's
  sidecar (as `frontier-swift-dump.sh` does).
- **Oklab.** Neutral, skin, sky and foliage are masked on the Swift render: hue bands as in
  `decoder-gap/analyse.py`, plus foliage at 100–160°.
- **The roll changed during the run.** At 13:36 the owner prepared roll #8 again in the Rust app (new roll
  reference `03378E02…`), then edited frames by hand. The study therefore uses the Swift sidecars kept by
  #180, the roll's state before 13:36. The original roll's sidecars no longer hold Swift's preparation.

## 3. Numbers

### Against the Swift app's own render (mean of 5 frames; Oklab ×100; dh in degrees, + is counter-clockwise)

| Roll, face | Arm | Neutral cast | Skin dh, chroma | Sky dh, chroma | Foliage dh, chroma | Outside sRGB |
|---|---|---|---|---|---|---|
| Z f, Pro | today: matrix, Swift sidecar | 1.55 | −2.8°, ×1.27 | +8.9°, ×0.71 | −17.6°, ×1.01 | 10.1 % |
| | matrix, roll measured again | 2.18 | −2.2°, ×1.37 | +16.3°, ×0.61 | −23.2°, ×1.05 | 10.4 % |
| | **sensor**, roll measured again | **1.91** | **−14.7°**, ×1.26 | +5.2°, **×0.51** | **−34.8°, ×0.70** | **3.1 %** |
| | Swift | | | | | 15.6 % |
| Z f, Frontier | today | 1.63 | +0.4°, ×1.12 | +5.7°, ×0.78 | −10.7°, ×1.12 | 0 |
| | matrix, measured again | 1.13 | −2.5°, ×1.07 | +7.2°, ×0.86 | −11.1°, ×1.03 | 0 |
| | **sensor** | **1.01** | −8.9°, ×1.05 | −5.9°, ×0.61 | −20.2°, ×0.69 | 0 |
| S1R, Pro | today | 3.05 | +7.2°, ×1.35 | −31.1°, ×0.49 | −18.5°, ×1.26 | 27.3 % |
| | matrix, measured again | 2.89 | +5.4°, ×1.26 | −28.8°, ×0.52 | −20.2°, ×1.14 | 27.8 % |
| | **sensor** | **1.72** | −5.5°, ×1.11 | −27.0°, ×0.55 | −26.0°, ×0.97 | **17.0 %** |
| | Swift | | | | | 5.8 % |
| S1R, Frontier | today | 2.57 | +12.3°, ×1.30 | −16.0°, ×0.62 | −13.1°, ×1.17 | 0 |
| | matrix, measured again | 0.67 | −21.0°, ×0.75 | +3.4°, ×1.04 | 0.0°, ×0.64 | 0 |
| | **sensor** | 0.94 | −28.2°, ×0.84 | +1.0°, ×1.04 | +10.6°, ×0.53 | 0 |

- **How to read the signs.** A falling hue moves skin (about 60°) toward red and foliage (about 130°) toward
  yellow. A rising hue moves the sky (about 240°) toward violet.
- **The S1R's sky mask is small**: a few % of its frames, so treat those cells with care.
- **The "prepared" arms**, the roll laid at rest, lean the same way. S1R Pro neutral: matrix 2.97, sensor
  1.77. Z f: 2.19 against 1.91.
- **The decode's own effect** is the difference between the two "measured again" rows. Sensor against
  matrix, both laid the same way:
  - Z f Pro: skin −12°, sky chroma −16 %, foliage −12° and −33 % chroma;
  - S1R Pro: skin −11° and −12 % chroma, foliage −6° and −15 % chroma, neutral cast −1.2.

### The variant at rest: 5000 K diagonal or nothing

| Arm, unprepared frame at rest (Pro) | Mean a, b ×100 | Mean chroma |
|---|---|---|
| Z f matrix | −11.6, −9.6 | 0.152 |
| Z f sensor, 5000 K | −13.4, −7.6 | 0.155 |
| Z f sensor, unbalanced | −3.5, −8.1 | 0.095 |
| S1R matrix / 5000 K / unbalanced | −9.8, −8.3 / −12.0, −7.7 / 0.0, −8.1 | 0.131 / 0.145 / 0.087 |

- **All three are unusable at rest.** The orange mask is uncorrected, so every one is a cyan-blue image
  (`rest/` strips).
- **The 5000 K diagonal keeps today's rest look.** The test pins it: a grey keeps the matrix's level.
- **The unbalanced arm looks less blue only by chance.** The camera's weak red and blue happen to cancel part
  of the mask. On a positive or a black-and-white frame the same arm would be green, by a different amount for
  each camera.
- **Prepared, the two variants agree**: 0.09 ΔE×100 on average, 0.3 at most on a frame, 0.44 at the
  99th centile. Their roll blacks agree to 2e-4. Only their gains
  differ: Z f R −1.21 / −0.49, B +0.79 / +1.17.
- **Choice: `Sensor`**, the 5000 K diagonal. `SensorUnbalanced` stays in the prototype only as the
  comparison arm.

### Blacks

On the Pro face, the darkest 5 % read Oklab L ×100:

| | Today (Swift sidecar) | Matrix, measured again | Sensor, measured again | Swift |
|---|---|---|---|---|
| Z f | 2.9 | 4.3 | 4.3 | 4.4 |
| S1R | 18.8 | 17.2 | 17.2 | 20.5 |

- **The colours change no black.** The roll's black comes from the bare-film pick, neutral in every mode.
- **#180's crushed Z f blacks come back once the roll is measured again on LibRaw**, in either mode. The base
  is then read on the same decode it is rendered on.

### Gamut and range

- **Below zero in the decode.**
  - Matrix: 0.6 % of the S1R foliage frame (P1071943, blue), a few pixels elsewhere.
  - Sensor: none. A non-negative diagonal cannot produce a negative value.
- **Above one.** No decode passes 1: these negatives peak at 0.63.
- **The rendered image.**
  - Pro face, pixels outside sRGB: Z f 10.4 % → 3.1 %, S1R 27.8 % → 17.0 %.
  - Camera channels, read as Rec. 2020 primaries, are less saturated than Rec. 2020.
  - The Frontier's table clips everything into range in every mode.

### Per camera: do the Z f and the S1R come closer?

- **The colour set.** Apple's 8-pixel decodes of all ten frames, 5.6 M squares.
- **How each camera reads it.** Through each camera's fitted decode-to-Apple 3×3 (correlation 0.9996 and
  0.9997).
- **The measure.** The density RMS that no per-channel gain can remove.

| | R | G | B |
|---|---|---|---|
| Z f against S1R, matrix | 0.0002 | 0.0027 | **0.0292** |
| Z f against S1R, sensor | 0.0065 | 0.0075 | **0.0138** |
| Z f against Apple, matrix / sensor | 0.010 / 0.004 | 0.007 / 0.015 | 0.014 / **0.067** |
| S1R against Apple, matrix / sensor | 0.010 / 0.004 | 0.006 / 0.008 | 0.016 / **0.055** |

- **The two cameras come closer: 0.029 → 0.017 overall.** The matrix-mode gap between them is the S1R's
  Adobe matrix's blue, the yellow lean of #180.
- **Each moves 4 to 5 times further from the colorimetric reading in blue.**
- **In the render**, the neutral casts against Swift converge in size but keep their hue apart:
  - matrix: Z f 2.2 at 43°, S1R 2.9 at 89°;
  - sensor: Z f 1.9 at 31°, S1R 1.7 at 76°.

## 4. What the strips show (`~/Downloads/sensor-colours/`)

Each frame has its own PNG: matrix today | sensor (roll measured again, same look) | Swift, with the Pro face
above and the Frontier below. `rest/` holds the three colours at rest.

- **Z f.**
  - Greens go olive: 6432's rice field turns golden, its vegetables lose green.
  - Skies go paler.
  - Skin and reds lean red: 6442's face is pinker, 6468's tent more vermilion than orange.
  - Greys are about as close to Swift as today.
- **S1R.**
  - The matrix's yellow-orange cast over everything (1949, 1943) is mostly gone in sensor mode. Pro greys
    look closer to Swift.
  - Foliage goes tan-olive (1943 Pro).
  - On the Frontier the whole S1R rendering turns cooler and teal (1943, 1949).
  - Saturated reds and pinks (1965) hold up. Sensor mode is not visibly duller there.

## 5. Risks

- **The working-space contract.** Every stage downstream assumes linear Rec. 2020: the Oklab measures and
  hue-dependent rules (print-curve hue compensation, chroma decompression, dye separation), the Frontier
  table, the Look Lab, and the display and export conversions. Sensor mode feeds them camera RGB labelled as
  Rec. 2020.
  - The pipeline runs, and its outputs stay in range.
  - The calibrations measured on Apple's Rec. 2020 decode no longer mean what they measured. The Frontier
    table is the one that matters.
- **CLAUDE.md, "The balance is the Swift app's 5000 K…".** Sensor mode keeps the 5000 K multipliers, derived
  from the camera's matrix, but drops Bradford and the matrix. The decision needs a sentence: "except on rolls
  set to sensor colours".
- **CLAUDE.md, "LibRaw replaces CIRAWFilter, so every RAW render moves once".** Sensor rolls move further:
  foliage up to −35°. They are a look, outside the parity goal, like the Frontier Look Lab. The default must
  stay the matrix.
- **#165's per-camera "Swift rendition" table** composes with the matrix only. Sensor mode bypasses it.
- **Hard rules.** No automatic decision as long as the choice is the user's. Floating point is unchanged. The
  demosaic is unchanged (rule 3). Zero anticipation: this is the owner's request, as a choice, not a
  slider.
- **Swift compatibility.**
  - The Swift app renders every RAW through `CIRAWFilter` in Rec. 2020, so a roll measured in sensor colours
    and opened in Swift is laid on the wrong space. The roll gains differ by Z f R −0.12, B −0.44 stop and
    S1R R −0.15, B −0.19, a strong cast.
  - Swift drops unknown fields when it writes, so a field alone does not survive a frame edited in Swift.
  - A `version` bump would make Swift open the roll read-only.
  - "A roll opens in either app" therefore becomes "opens in either app, renders right only in the one whose
    colours it was measured in". The interface has to say so where the choice is made.
- **DNG and linear scans.** Only the mosaic branch has a sensor space with a meaning. Linear TIFF and DNG
  LinearRaw stay as they are. A mosaic DNG would read its own ColorMatrix for the neutral.
- **Unknown or monochrome sensors.** Three colours are needed, as today (`NO_COLOUR_MATRIX`).

## 6. Proposed product design

1. **Settings**: "Colours of new rolls". The choices are *Camera matrix* (default, as the Swift app) and
   *Sensor*. It is read once, when a roll is first prepared, and never applied to a roll already prepared.
2. **Prepare the Roll**: the same choice on the film step, beside the film, preset from Settings. On a
   prepared roll it reads "Colours: matrix / sensor".
3. **Switching re-measures the roll** with its stored window, ticked and flagged frames and **base pick**,
   then lays the new reference under each frame's own look (`balance::written(reference, frame)`). It is
   one gallery undo step.
   - Hand-placed level handles and the print filters of Auto White Balance are kept as values. They were
     placed on the other decode, and a note says so.
4. **Storage, as two records that agree.**
   - Each frame's sidecar gets `decodeColours: "sensor"`. Absent means matrix, so every sidecar today reads
     as today.
   - The Rust library records the roll's colours keyed by its `rollReference.id`.
   - **When the field is gone but the id matches**, the Rust app trusts the record and writes the field back:
     Swift dropped it on a write.
   - **When Swift prepared the roll again**, the id is new and the roll is matrix. That is correct, since it
     was measured on Apple's decode.
5. **The base pick** must outlive the draft: frame, unit point and radius, kept with the record. For a roll
   Swift prepared, it is found again as here, in the rebate band, against `rollReference.base`. That landed
   within 0.0008 to 0.0022 of window on both rolls.
6. **Caches.** The decode's colours join every key a decode feeds: the measuring copy's `Reading`, previews,
   thumbnails, the full-resolution cache.
7. **The transition module (#175) must skip sensor rolls.**
   - **Skip any frame whose roll is sensor**, by field or by record. Its shift is LibRaw-*matrix* against
     Apple, and laid on a sensor roll it adds a cast.
   - **Decode LibRaw with `Colours::Matrix` explicitly** in its Apple-against-LibRaw measure, never the
     Settings default.
   - **Mark a Swift roll switched to sensor before the transition reached it** as settled in the ledger. The
     re-measure supersedes the correction, and the time rule alone would also skip it, since its sidecars are
     newer.

## Tests it would need

- **Decode on a real NEF and RW2.** `Sensor` must equal `diag · conversion⁻¹ ·` the matrix decode to 1e-6.
  Twin: the identity in place of the inverse fails. The matrix path must stay byte for byte as before.
- **Sidecar.** An absent field reads as matrix, and an unknown value reads as matrix. Twin: `"sensor"` reads
  as sensor. A sensor roll's record survives a Swift-style rewrite that drops the field.
- **Switching** keeps the base pick: the black lands on the same rebate disc within 1e-6. Twin: a pick moved
  into the picture lands elsewhere.
- **Transition.** A sensor roll is left untouched. Twin: the same roll in matrix colours is corrected.

## Reproduce

```
scripts/sensor-colours/run <work> <strips> <zf roll copy> <frames,> <s1r roll copy> <frames,>
```

- **What it runs.** The Swift `--render` per face, `apple-decode` blocks,
  `cargo run -p oni-measure --example sensor_colours` per roll, then `analyse.py`. About 2 min per roll.
- **The roll copies** hold clones of the RAWs and the Swift sidecars. Nothing in the originals is written.
