# Apple's decoder against LibRaw, and the rolls prepared in the Swift app

Measured on 2026-10-20 (#165) on the reference roll's test copy (Nikon Z f, 38 NEFs, ISO 100, colour
negative), one Lumix S1R negative (`P1072392.RW2`) and one Fujifilm RAF positive scene. Every number
below comes from `scripts/decoder-gap/run`; nothing in the app changed.

## Short answer

- **The lean is real and it is red plus yellow.** On roll #8, opened in the Rust app with the
  sidecars the Swift app wrote, the positive comes out **+0.11 stop in red, -0.01 in green, -0.10 in
  blue** against the Swift render (mean of 38 frames, linear output). On the S1R frame it is
  +0.06 / -0.02 / **-0.31** stop: a much stronger yellow.
- **Cause.** Both decodes are linear and agree on exposure within 0.03 stop. They differ by camera
  profile: LibRaw converts with Adobe's matrix, `CIRAWFilter` with Apple's own. On the negative, LibRaw
  reads red 0.063 stop lower and blue 0.126 stop higher, so the red density is 0.019 higher and the
  blue density 0.038 lower. After inversion that gives more red and less blue.
- **"A simple shift of gains"?** Mostly. A per-channel gain removes about 3/4 of the gap. The rest
  depends on colour (one frame, DSC_6468, gets worse under gains alone), and the gain differs from
  camera to camera (the Z f's blue is -0.13 stop, the S1R's -0.33), so no single constant serves every
  user.
- **A 3×3 matrix per camera closes the gap** to the decoders' demosaic noise: 0.0016 mean output gap
  against 0.018 today, with residuals flat across density.

## Method

- `scripts/decoder-gap/apple-decode.swift` runs `CIRAWFilter` with exactly the Swift app's
  `RawDecode.configured`:
  - decoder version 8, boost 0, no gamut mapping, 5000 K / tint 0;
  - shadow bias 0, chroma noise 1, sharpness 0;
  - lens correction, highlight recovery and EDR off.
  
  The context is `extendedLinearITUR_2020` in `RGBAf`, the space the pipeline receives.
- `oni-decode/examples/blocks.rs` dumps `oni_decode::decode` on the same grid.
- Both decodes are averaged over 8×8 pixel squares. On the NEFs the grids align at (0, 0): correlation
  0.9998, both frames 6048×4032. On the RAF LibRaw's frame sits at (6, 7). A square counts when it is off
  the edge and every channel lies between 2e-4 and 0.9 in both decodes: about 378 k squares a frame.
- `oni-decode/examples/decoder_fit.rs` fits LibRaw to Apple in two ways:
  - per-channel gains, the median of log2 ratios;
  - a 3×3 matrix, least squares on the relative error.
- `oni-pipeline/examples/decoder_gap.rs` renders each frame through the real pipeline (GPU, every
  stage) with the sidecar the Swift app wrote, a read-only copy of the original roll's. It does this
  on Apple's decode and on four LibRaw variants.
- Apple's preview decode (`scaleFactor` to a 2000-pixel side, the copy the Swift app measures on)
  matches its full-size decode within 0.006 stop per channel. Full-size dumps therefore stand for the
  measured copy.

## Measurements

### Apple over LibRaw, per channel (stops, median over the squares)

| Camera | R | G | B | Spread over frames (sd) |
|---|---|---|---|---|
| Nikon Z f, roll #8, 38 frames | +0.063 | -0.026 | -0.126 | 0.008 / 0.009 / 0.006 |
| Lumix S1R, 1 frame | +0.036 | -0.013 | -0.326 | one frame |
| Fujifilm RAF, 1 positive scene | +1.97 | +1.74 | +2.26 | one frame |

37 of the 38 Z f frames sit within 0.012 / 0.018 / 0.015 stop of the mean. DSC_6468 reads
+0.028 / -0.055 / -0.144, because its colours differ from the others'.

### Residual of each model (density, log10; Apple minus model)

| Camera | Model | Mean R / G / B | RMS R / G / B |
|---|---|---|---|
| Z f | none | +0.018 / -0.008 / -0.036 | 0.019 / 0.010 / 0.037 |
| Z f | gains | -0.001 / 0.000 / +0.002 | 0.006 / 0.006 / 0.009 |
| Z f | 3×3 matrix | 0.000 / 0.000 / 0.000 | 0.0007 / 0.0008 / 0.004 |
| S1R | none | +0.011 / -0.004 / -0.108 | 0.014 / 0.007 / 0.113 |
| S1R | gains | 0.000 / 0.000 / -0.010 | 0.009 / 0.006 / 0.034 |
| S1R | 3×3 matrix | 0.000 / 0.000 / +0.002 | 0.003 / 0.003 / 0.022 |

- **Cross-validation.** A Z f matrix fitted on the even frames and tested on the odd ones gives the
  same 0.0007 / 0.0008 / 0.004. Across single-frame fits, each matrix coefficient varies by at most
  0.006 (sd).
- **The model is per camera.** The Z f matrix applied to the S1R leaves 0.086 in blue.
- **Not a white balance alone.** Gains in the camera's own space (Apple's neutral read differently, the
  same matrix otherwise) fit no better than output gains: 0.006 / 0.007 / 0.012 on the Z f. Apple's
  matrix itself differs.

The fitted matrices, LibRaw's Rec. 2020 to Apple's:

```
Z f   1.1252 -0.1698 -0.0064      S1R   1.1065 -0.1704 -0.0194
      0.0259  0.9537 -0.0640            0.0097  0.9946 -0.0704
      0.0092 -0.0109  0.8969           -0.0043 -0.0155  0.8632
```

### Linearity: residual after the matrix, by Apple's density (stops)

| Density | 0.4 | 0.8 | 1.2 | 1.6 | 2.0 | 2.4 | 2.8 |
|---|---|---|---|---|---|---|---|
| Z f R | -0.001 | 0.000 | +0.001 | +0.001 | | | |
| Z f G | 0.000 | -0.001 | 0.000 | +0.001 | +0.003 | | |
| Z f B | +0.002 | -0.001 | 0.000 | 0.000 | -0.001 | +0.002 | +0.021 |
| S1R B | | +0.019 | -0.003 | -0.008 | +0.018 | +0.034 | |

- **The Z f is linear in both decoders** over the whole negative. Only the densest blue, near Apple's
  floor, moves.
- **The S1R's blue keeps ±0.03 stop of curvature**, which a matrix cannot take.
- **On the gains model, the Z f residual grows with density**: -0.03 to +0.14 stop in blue from
  density 0.4 to 3.0. That is the colour dependence that gains leave behind.

### What the user sees: output of the real pipeline, minus Apple's render

The pipeline renders with the Swift app's own sidecars. The output is linear, with means of R 0.19,
G 0.19 and B 0.20 on roll #8. The last column is the mean of each pixel's worst channel.

| Decode fed to the pipeline | R | G | B | Worst channel |
|---|---|---|---|---|
| Z f, LibRaw as today | +0.0151 (+0.11 stop) | -0.0014 | -0.0137 (-0.10 stop) | 0.0182 |
| Z f, gains at decode | +0.0007 | +0.0018 | +0.0024 | 0.0051 |
| Z f, gains folded into `gains.stops` | +0.0007 | +0.0018 | +0.0028 | 0.0052 |
| Z f, matrix at decode | 0.0000 | 0.0000 | +0.0001 | 0.0016 |
| S1R, LibRaw as today | +0.0067 | -0.0024 | -0.0307 (-0.31 stop) | 0.0311 |
| S1R, gains at decode | +0.0008 | +0.0009 | -0.0051 | 0.0093 |
| S1R, matrix at decode | +0.0001 | +0.0001 | +0.0002 | 0.0054 |

- **Faces.** The 14 Scanner frames lean more than the 24 Pro frames: +0.018 / -0.017 against
  +0.014 / -0.012.
- **Roll #8 was prepared twice.** The test copy was prepared again in the Rust app, and its roll gains
  differ from the Swift app's by R +0.092 and B -0.094 stop. The decode gap above, set on green,
  predicts +0.089 and -0.099. Preparing the roll again therefore recovers the balance within 0.006
  stop.

### Fujifilm: an exposure gap, not a colour one

- **Apple's view of this RAF.** `CIRAWFilter` reports a baseline exposure of 2.02 for it, and LibRaw
  reads none. The Rust decode therefore falls back on its constant 0.48, a 1.54 stop gap. The midtones
  measure 1.53 to 1.58 stops.
- **Effect on a Swift-prepared roll.** On a Fuji roll the levels windows would sit about 0.46 density
  off. That is far larger than the colour lean.
- **Status.** This is one positive frame from an X-Trans sensor, with diverging shadows (density above
  2.4). It needs a Fuji negative before anything is decided.

## Which stored values depend on the decode

The stage-4-to-7 kernel reads `t = (s + pedestal) · 2^gains`, then `d = -log10 t`, and every balance
number lives on that axis (`oni-render/src/wgsl/stage47.wgsl:106-108`). The pedestal is added before
the gains, so a decoder gain folded into `gains.stops` is exact only to within the pedestal,
10^-3.5 (the 0.0004 difference between the two gain rows above).

| Sidecar field | Space | Under a decoder gain | Under a decoder matrix |
|---|---|---|---|
| `gains.stops`, `rollReference.stops` | linear gain before the log | shifts exactly | not representable |
| `levels.*.black/white`, `rollReference.black/white/base/median` | density at the roll's gains | valid once the gains shift | not representable |
| `densityCeiling` (pedestal) | linear offset | nearly valid | nearly valid |
| `print.red/green/blue.exposure` (filter key), `balanceFilters` | after inversion | valid once the gains shift | drifts with colour |
| Scanner clicks, separation, decompression, geometry, `captureStops` | doses, counts, EXIF | independent | independent |

No field records the decoder. `version` (19) is the format's. The precedent is
`legacy::neutral_decode_gains` (format 6), which folded an earlier decoder gain into `gains.stops`
and nothing else.

## Recovery strategies

| | (a) Matrix at decode, per camera | (b) One-time sidecar migration | (c) Marker of the decoder that prepared it |
|---|---|---|---|
| Gap left (Z f / S1R) | 0.0016 / 0.0054 | 0.0052 / 0.009 (gains only) | as (a) or (b) behind it |
| Sidecar | untouched | `gains.stops` and `rollReference.stops` rewritten | new field |
| Same roll in Swift afterwards | same look in both apps | Swift renders the inverse cast, blue/cyan | marker lost on Swift's first write |
| Unknown camera | LibRaw as today | needs that camera's gains anyway | needs them anyway |
| Hard rules | a constant per camera, no decision | rewrites settings without the user | — |

- **(c) does not survive the Swift app.** Swift re-encodes typed settings on every write, so an
  unknown key disappears (`Sidecar.write`). Bumping `version` instead makes the Swift app open the
  roll read-only.
- **(b) breaks the promise that a roll opens in either app.** It also recovers only the gain part, and
  it still needs a per-camera gain.
- **(a) is the only option that leaves the user's files alone and agrees in both directions**, old
  rolls and new.

**Recommendation: (a).**

- **The table.** A table of "Swift rendition" matrices keyed by make and model, multiplied into
  `conversion`'s matrix in `oni-decode` for every RAW of that camera, prepared in either app. The table
  would start with the Z f and the S1R.
- **Where it comes from.** Each entry is measured once with `scripts/decoder-gap/run` on a Mac, on at
  least one negative and one wide-gamut frame per camera.
- **Cameras without an entry** decode as today. Their users can **prepare the roll again**, which this
  roll shows recovers the gains within 0.006 stop, at the cost of any handles placed by hand after
  the roll's balance.
- **A later step, only if users ask.** On a Mac, a click could run the same fit against `CIRAWFilter`
  on one of the roll's frames for an unknown camera. Rolls prepared in Swift exist only on Macs.

**This reverses a decision in CLAUDE.md**: "every RAW render moves once against the Swift app" would
become "matches it for the cameras in the table". The owner decides.

## Risks

- **The fit covers only the negative's gamut.** Roll #8's log-chroma spread is 0.03 and 0.09 decades
  against 0.45 in luminance. Saturated colours (slides, positives, colour-chart patches) are an
  extrapolation of the matrix until a wide-gamut frame confirms it.
- **Apple's profile is not always a matrix.** The S1R keeps ±0.03 stop of curvature in blue, and other
  cameras may keep more.
- **DNGs and linear scans must stay out of the table.** Their renders must not move, and Apple reads a
  DNG's own profile.
- **The table is frozen at decoder version 8.** That is the version the Swift app pins.
- **Fuji's exposure gap is separate** and is not fixed by this.

## Tests it would need

- **Lookup.** Make and model to matrix, an unknown camera to identity. Twin: a near-miss model name
  gets identity.
- **Decode against Apple, per table camera.** Runs on `ONI_REFERENCE` block dumps. After the
  correction the density RMS must be at most 0.002 on the Z f and 0.025 in the S1R's blue. Twin: the
  uncorrected decode must fail, at 0.019 and 0.11.
- **Pipeline.** `roll8_lands_on_the_swift_render` would bound LibRaw's render at about 0.003 mean
  instead of 0.1. Twin: without the matrix it must fail, at 0.018.
- **Unchanged decodes.** Linear TIFF, DNG and plain images decode byte for byte as before.
- **Sidecars untouched.** A Swift-prepared sidecar opened and closed without an edit keeps its bytes.

## Transition

The owner chose strategy (a), once a 3×3 proved quick to measure in the app: `crates/oni-transition`,
behind a blocking sheet on macOS (#175). No sidecar is read to decide anything, and none is written.

- **Per camera.** LibRaw's make and model. `CIRAWFilter` runs in the process through `objc2`, set as
  `RawDecode.configured`. Both decodes are averaged over 8-pixel squares and aligned to the pixel (the
  best correlation of log luminance within 8 pixels of the frames' size difference). The squares off
  floor and clip of three frames spread over the camera's RAWs are pooled, and the 3×3 is fitted on
  their relative error, as `decoder_fit` does.
- **The table.** `<support>/Transition/cameras.json` holds, per camera, the matrix, its frames and its
  residual, written atomically. `oni-decode/src/rendition.rs` folds the camera's matrix after
  `conversion()`'s, before `linear::converted`, in one place that `decode_native` skips. DNGs, linear
  scans and cameras absent from the table decode as before.
- **When.** At launch and whenever a folder opens, the library's RAWs are named off the interface thread
  (`seen.json` remembers each file by size and date). A camera the table lacks is measured behind the
  sheet, from any three reachable frames, while the thumbnails' pass waits. Its frames' previews and
  thumbnails are then dropped and drawn again, and the open frame is read again.
- **Windows.** There is no Apple decoder, so no table: RAWs decode in LibRaw's own colours.
- **Rolls prepared in Rust test builds** were balanced on LibRaw's colours and shift once. The owner
  accepted this.

Measured on the test copy of roll #8 and on the S1R ColorPlus roll, read only:

| | Z f | S1R |
|---|---|---|
| Measure, three frames (both decodes) | 4.2 s | 7.0 s |
| Fitting the matrix | 32 ms | 63 ms |
| Density residual R / G / B | 0.0007 / 0.0007 / 0.0036 | 0.0044 / 0.0036 / 0.0194 |
| Render gap to Swift: none / gains / matrix | 0.0182 / 0.0051 / 0.0016 | 0.0462 / 0.0119 / 0.0095 |
| Pro face, pixels under L 0.03: Swift / LibRaw × gains / matrix | 6.64 / 7.51 / 6.62 % | |
| Pro face, darkest 5 % L ×100: Swift / gains / matrix | 3.4 / 2.9 / 3.4 | 23.1 / 22.9 / 23.2 |

- **Blacks.** The matrix brings them back to Swift's, where gains alone deepen them on the Z f.
- **S1R.** The blue curvature stays (0.0095): a matrix cannot carry it.
- **Fit on one frame.** A matrix fitted on a single frame holds on the Z f (0.0018). On the S1R one frame
  of three gave an outlier, so the three frames are pooled.
- **Blacks against #180.** These percentages come from `render-pairs` and `analyse.py` on this branch.
  Their absolute level differs from `decoder-per-camera.md`'s table, though the order of the arms is
  the measure's.

## Reproduce

```
scripts/decoder-gap/run <out> <sidecar folder or -> <RAW>...
```

The script works on Macs whose Xcode licence is unaccepted, once the toolchain environment is sourced
(`CLAUDE.md`, Build). It compiles the Swift tool with the
toolchain's `swiftc`, writes `<out>/dumps`, then prints the fit to `<out>/fit.txt` and the renders to
`<out>/gap.txt`. The sidecar folder holds `<name>.json` copies, so the originals are only read. The
whole roll takes about 70 s.

The transition's own measure, a camera's matrix from its frames with each step timed:
`cargo run --release -p oni-transition --example measure -- <raw>...`.
