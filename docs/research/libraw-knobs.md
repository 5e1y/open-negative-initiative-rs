# LibRaw's knobs against the Swift app's `CIRAWFilter` settings

Measured on 2026-10-20 (#176) on two frames of the reference roll's test copy (Nikon Z f, ISO 100,
`DSC_6432`, `DSC_6468`), read only. Nothing in the app changed.

## Short answer

- **The Swift decode is much smoother than LibRaw's.** Over the whole crop, LibRaw's pixel-scale colour
  detail is **4.9 to 6.7 times** Swift's and its pixel-scale green detail **2.0 to 2.2 times**, both in
  density, the axis the inversion amplifies.
- **Apple's colour noise reduction at 1 accounts for most of it.** It removes 75 % of the pixel-scale
  colour detail, 59 % at 2 px, 33 % at 4 px and 13 % at 8 px, and 33 % of the pixel-scale green. The
  rest is the demosaic: at 2 px and coarser, LibRaw's AHD matches Apple's decode without its colour NR
  within 3 % on `DSC_6432` and 10 % on `DSC_6468`.
- **Sharpness is not a cause.** The Swift app turns Apple's sharpening off, and LibRaw has none. LibRaw's
  AHD already carries more pixel-scale green energy than Apple's decode with sharpening 1 (0.0199
  against 0.0161).
- **No LibRaw knob reproduces Apple's colour NR.** The best ones remove 20 % (FBDD) or 12 % (three median
  passes) of the colour detail, against Apple's 75 %. A chroma blur of about 1.25 px, in log space and at
  full force, matches Apple's colour NR within the spread of the two frames.
- **One automatic knob is still on**: `adjust_maximum_thr` (0.75). It never fires on roll #8 but would
  rescale a brightly exposed scan frame by frame.

## 1. What the Swift app sets (`RawDecode.configured`)

| `CIRAWFilter` option | Swift value | Apple default (Z f, ISO 100) | Reason given in the code |
|---|---|---|---|
| `decoderVersion` | highest ≤ 8 (8) | 8 | a newer default would move every render unasked |
| `boostAmount`, `boostShadowAmount` | 0 | — | the default is a tone curve, different per channel |
| `contrastAmount`, `detailAmount`, `localToneMapAmount` | 0 | 0, 0, — | no tone decision upstream |
| `isGamutMappingEnabled`, `isHighlightRecoveryEnabled`, `extendedDynamicRangeAmount` | off, off, 0 | — | no tone or gamut decision upstream |
| `neutralTemperature`, `neutralTint` | 5000 K, 0 | as shot | one neutral, film-agnostic balance |
| `shadowBias` | 0 | per camera | Apple's floor drives the densest blue under zero |
| `colorNoiseReductionAmount` | **1** | 0.5 | halves the colour noise the inversion amplifies, luma unmoved (`--demosaic`) |
| `luminanceNoiseReductionAmount` | 0 | 0.363 | no fine-detail filter before the inversion |
| `sharpnessAmount` | **0** | 1 | carves the demosaic's patterns into worm-like grain; Details sharpens instead |
| `moireReductionAmount` | 0 | 0.55 | a chroma filter of fine detail before the inversion |
| `isLensCorrectionEnabled` | off, on when the sidecar says `lensCorrected` | on | at 1:1 the profile straightens nothing and resamples away 15–20 % of fine detail; dust spots placed on the corrected geometry keep it (up to 24 px of shift) |
| `scaleFactor` | to a 2000 px side for the measuring copy | 1 | Apple's own reduced demosaic |
| `baselineExposure` | left to Apple | file's | a uniform gain no balance can see |

`DemosaicBench` (`--demosaic`) in the Swift app is the bench those choices came from.

## 2. LibRaw 0.22.2 as vendored, and what `pinned.c` sets

`build.rs` compiles every demosaic in the copy: `ahd`, `aahd`, `dcb` (with FBDD), `dht`, `misc` (linear,
VNG, PPG) and `xtrans`. Neither RawSpeed nor the DNG SDK is built, so the `DNG_STAGE2/3` options do
nothing. The GPL demosaic packs (AMaZE, LMMSE, VCD) left LibRaw in 0.20 and are absent. LibRaw computes
in 16-bit integers internally, whatever `output_bps` says.

`dcraw_process` runs, in order: black subtraction, `adjust_maximum`, `user_sat`, `green_matching`, the
scaling (with wavelet denoise and `aber`), `exp_correc`, FBDD, the demosaic, the median passes, the
highlight modes 2 and up, and the colour conversion.

| Knob | What it does | `pinned.c` | Bears on the comparison with Apple |
|---|---|---|---|
| `user_qual` | demosaic: 0 linear, 1 VNG, 2 PPG, 3 AHD, 4 DCB, 11 DHT, 12 AAHD | 3 (AHD) | yes, the pixel-scale texture (§3) |
| `dcb_iterations`, `dcb_enhance_fl` | DCB's refinement passes and colour enhancement | default (−1) | only with DCB |
| `fbdd_noiserd` | FBDD denoise on the mosaic before any demosaic: 1 light, 2 full | 0 | partly: −20 % colour detail |
| `med_passes` | 3×3 median on R−G and B−G after the demosaic, n passes | 0 | partly: −12 % at 3 passes |
| `threshold` | wavelet denoise, luma and chroma, on the 4 planes before the demosaic | 0 (default) | luma denoise, which Swift refuses |
| `green_matching` | evens G1 against G2 before the demosaic | 0 (default) | no measurable effect on the Z f |
| `four_color_rgb` | demosaics G1 and G2 apart (for maze artefacts) | 0 (default) | no |
| `aber[0]`, `aber[2]` | lateral CA: R and B planes scaled about the centre, bilinear | 1 (default) | Apple's lens correction is off too |
| `highlight` | 0 clip, 1 unclip, 2 blend, 3–9 rebuild | 1 | matches Apple's recovery off |
| `exp_correc`, `exp_shift`, `exp_preser` | linear gain before the demosaic, 0.25 to 8, clipped in 16 bits | gain 1 in the app (2 in the linearity test only) | no; the baseline lives in Rust's float matrix |
| `half_size` | one pixel per 2×2 block, no demosaic | on when the long side ≥ 2 × requested | the preview's texture differs from Apple's `scaleFactor` |
| `use_camera_wb`, `use_auto_wb`, `user_mul` | the multipliers | 0, 0, Rust's 5000 K Planckian | matches `neutralTemperature` |
| `use_camera_matrix`, `output_color` | the profile LibRaw would apply | 0, 0 (camera space) | no; Rust converts with the Adobe matrix (`decoder-apple-vs-libraw.md`) |
| `output_bps`, `gamm`, `no_auto_bright`, `bright`, `auto_bright_thr` | output depth and curve | 16, linear, on, 1, unused | matches Apple's boost and contrast at 0 |
| `user_black`, `user_cblack`, `user_sat` | override the file's black and white levels | file's (−1) | matches `shadowBias` 0 |
| **`adjust_maximum_thr`** | **lowers the white level to the frame's own maximum when that exceeds 75 % of it** | **default 0.75, not pinned** | **a per-frame decision Apple does not make** |
| `no_auto_scale`, `no_interpolation` | skip the scaling or the demosaic | 0 | no |
| `use_fuji_rotate` | turns Fuji's 45° sensors | 1 (default) | Fuji only |
| `rawparams.options` | DNG stages, JPEG-XL previews, float-to-int | `CONVERTFLOAT_TO_INT` | DNG SDK not built |
| `coolscan_nef_gamma` | gamma of a Coolscan film scanner's NEF | 1.0 | film scans from a Coolscan only |
| `bad_pixels`, `dark_frame` | dcraw's map and dark-frame files | none | a flat-field could use them, not Apple |

**`adjust_maximum_thr` on roll #8.** Across the 38 NEFs, the frame's maximum peaks at 0.56 of the white
level (`DSC_6469`), under the 0.75 trigger, so it never fired. A scan exposed to the right, with the film
base above 75 % of white, would have its white lowered to its own peak: up to +0.41 stop, different on
every frame of the roll.

## 3. Measurements

**Method.**
- **The two decodes.** `CIRAWFilter` is set exactly as `RawDecode.configured`, then one knob moves per
  arm. LibRaw is the vendored 0.22.2, built apart with the pinned options and `output_color` Rec. 2020,
  one knob per arm.
- **The crop.** Both decodes give the same 2048 px centre crop in linear Rec. 2020 floats; the shift
  between them, searched over ±12 px, is 0.
- **The readings.** All are taken in density (−log10), where a per-channel gain changes nothing:
  - `DemosaicBench.read`, unchanged: chroma and luma are the spread in the flattest tenth of 32 px
    blocks, detail and colour edge the |Laplacian| in the busiest tenth;
  - `hf-G` and `hf-C`: the mean |Laplacian| of G, and of R−G and B−G, over every pixel;
  - for the bands below, the same after k×k binning.

**Arms, `DSC_6432` (`DSC_6468` in brackets for the two whole-crop columns).** "Swift" is the app's
decode. LibRaw's time is `dcraw_process` alone, single-threaded, on a 24 MP frame.

| Arm | flat chroma | flat luma | busy detail | colour edge | hf-G | hf-C | time |
|---|---|---|---|---|---|---|---|
| Apple, Swift's settings | 0.0133 | 0.0086 | 0.0154 | 0.0074 | 0.0091 (0.0079) | 0.0050 (0.0041) | |
| Apple, colour NR 0 | 0.0166 | 0.0107 | 0.0200 | 0.0231 | 0.0137 (0.0109) | 0.0197 (0.0151) | |
| Apple, colour NR 0.5 (Apple's default) | 0.0165 | 0.0106 | 0.0200 | 0.0231 | 0.0136 (0.0108) | 0.0197 (0.0151) | |
| Apple, sharpness 1 | 0.0128 | 0.0107 | 0.0259 | 0.0074 | 0.0161 (0.0144) | 0.0050 (0.0041) | |
| Apple, luminance NR 1 | 0.0136 | 0.0072 | 0.0137 | 0.0074 | 0.0062 | 0.0050 | |
| Apple, moiré 1 | 0.0027 | 0.0076 | 0.0153 | 0.0072 | 0.0090 | 0.0043 | |
| Apple, every default | 0.0041 | 0.0089 | 0.0242 | 0.0076 | 0.0139 (0.0121) | 0.0050 (0.0045) | |
| LibRaw AHD, as pinned | 0.0152 | 0.0112 | 0.0277 | 0.0702 | 0.0200 (0.0155) | 0.0334 (0.0200) | 0.8 s |
| AHD + 1 median pass | 0.0141 | 0.0110 | 0.0271 | 0.0632 | 0.0196 | 0.0297 | 3.5 s |
| AHD + 3 median passes | 0.0135 | 0.0108 | 0.0270 | 0.0623 | 0.0195 (0.0151) | 0.0294 (0.0166) | 8.4 s |
| AHD + FBDD 1 | 0.0136 | 0.0103 | 0.0238 | 0.0463 | 0.0172 (0.0130) | 0.0272 (0.0160) | 2.5 s |
| AHD + FBDD 2 | 0.0136 | 0.0103 | 0.0239 | 0.0459 | 0.0172 | 0.0272 | 3.1 s |
| AHD + wavelet 100 | 0.0153 | 0.0061 | 0.0147 | 0.0218 | 0.0081 | 0.0137 | 1.1 s |
| AHD + green matching | 0.0153 | 0.0120 | 0.0283 | 0.0643 | 0.0205 | 0.0330 | 0.8 s |
| linear | 0.0164 | 0.0114 | 0.0232 | 0.0435 | 0.0163 | 0.0277 | 0.3 s |
| VNG | 0.0146 | 0.0113 | 0.0317 | 0.0756 | 0.0230 | 0.0358 | 2.0 s |
| PPG | 0.0156 | 0.0109 | 0.0272 | 0.0678 | 0.0196 | 0.0341 | 0.3 s |
| DCB | 0.0144 | 0.0112 | 0.0274 | 0.0546 | 0.0189 (0.0150) | 0.0261 (0.0163) | 3.5 s |
| DHT | 0.0137 | 0.0118 | 0.0343 | 0.0299 | 0.0231 (0.0186) | 0.0173 (0.0126) | 3.1 s |
| AAHD | 0.0146 | 0.0119 | 0.0401 | 0.1014 | 0.0273 (0.0198) | 0.0414 (0.0222) | 3.4 s |
| AHD + log chroma blur 1.5 px, force 1 | | | | | 0.0191 (0.0151) | 0.0036 (0.0025) | |

**Bands, `DSC_6432`: mean |Laplacian| of G and of colour after k×k binning.**

| Arm | G/1 | C/1 | G/2 | C/2 | G/4 | C/4 | G/8 | C/8 |
|---|---|---|---|---|---|---|---|---|
| Apple, Swift's settings | 0.0091 | 0.0050 | 0.0189 | 0.0142 | 0.0388 | 0.0288 | 0.0619 | 0.0394 |
| Apple, colour NR 0 | 0.0136 | 0.0196 | 0.0241 | 0.0347 | 0.0420 | 0.0432 | 0.0631 | 0.0452 |
| Apple, sharpness 1 | 0.0161 | 0.0050 | 0.0261 | 0.0142 | 0.0442 | 0.0288 | 0.0650 | 0.0394 |
| LibRaw AHD | 0.0199 | 0.0332 | 0.0249 | 0.0357 | 0.0428 | 0.0419 | 0.0640 | 0.0443 |
| AHD + FBDD 1 | 0.0171 | 0.0270 | 0.0228 | 0.0294 | 0.0412 | 0.0373 | 0.0633 | 0.0426 |
| DHT | 0.0230 | 0.0173 | 0.0270 | 0.0248 | 0.0439 | 0.0369 | 0.0646 | 0.0425 |
| AHD + log chroma blur 1 px | 0.0192 | 0.0067 | 0.0238 | 0.0183 | 0.0419 | 0.0336 | 0.0637 | 0.0413 |
| AHD + log chroma blur 1.5 px | 0.0191 | 0.0036 | 0.0233 | 0.0117 | 0.0411 | 0.0270 | 0.0633 | 0.0381 |
| AHD + log chroma blur 2 px | 0.0190 | 0.0022 | 0.0229 | 0.0079 | 0.0404 | 0.0211 | 0.0628 | 0.0343 |

**Readings.**
- **Apple's colour NR acts between 0.5 and 1.** At 0.5, Apple's default, it is within 1 % of 0 on these
  ISO 100 frames. The Swift value of 1 is where all of its effect lies.
- **The log chroma blur is a model, not Apple's filter.** It blurs log R, G, B minus their Rec. 2020
  luma by a Gaussian, at full force. At 1.5 px it sits slightly under Apple at every band on
  `DSC_6432` and further under on `DSC_6468`. Between 1 and 1.5 px brackets Apple's curve, so 1.25 px
  is the starting constant. The same blur in linear RGB raises the colour detail measured in density,
  across luma edges: the space matters.
- **The pixel-scale green gap is the demosaic.** A chroma filter leaves it: 0.0191 against Apple's
  0.0091. Part of it is Apple's colour NR, which also lowers its own G by a third. The rest is AHD's
  Nyquist-scale energy, noise and zipper, that Apple's demosaic does not make. At 2 px and coarser the
  green agrees within 3 %.
- **Moiré reduction is the strongest chroma filter Apple has.** Its default, 0.55, takes the flat
  chroma from 0.0133 to 0.0041. It is the main reason a default `CIRAWFilter` looks cleaner still. The
  Swift app leaves it at 0, so parity needs nothing here.
- **The pipeline's own chroma denoise does not close the gap.** Details' chroma denoise (radius 4,
  force 0.2 at rest) runs after the inversion in both apps. On LibRaw's decode it removes at most 20 % of
  the remaining chroma, so the Swift render has gone through two chroma filters and the Rust render through
  one, the weaker.

## 4. Swift option against its nearest LibRaw or pipeline equivalent

| Swift option | Nearest equivalent | Parity today |
|---|---|---|
| decoder version 8 | the vendored LibRaw 0.22.2, never the system's | equivalent |
| boost, contrast, local tone map, EDR, gamut mapping at 0 | linear gamma, `no_auto_bright` | equivalent |
| highlight recovery off | `highlight` 1 | equivalent |
| 5000 K / tint 0 | `user_mul` from Rust's 5000 K Planckian | equivalent; matrices differ (`decoder-apple-vs-libraw.md`) |
| `shadowBias` 0 | the file's black (`user_black` −1) | equivalent |
| baseline exposure | Rust's matrix: BaselineExposure, else 0.48 | equivalent (Fuji aside) |
| **colour NR 1** | **none in LibRaw**: FBDD −20 %, median ×3 −12 %; an ONI chroma stage at decode would match | **missing; the main texture gap** |
| luminance NR 0 | `threshold` 0, `fbdd_noiserd` 0 | equivalent |
| sharpness 0, detail 0 | LibRaw has no sharpening; Details sharpens after | equivalent; AHD is crisper at Nyquist anyway |
| moiré 0 | nothing | equivalent |
| lens correction off | nothing | equivalent |
| lens correction on for `lensCorrected` frames | **none**: LibRaw reads no lens profile and applies no DNG warp opcode | **missing**: dust spots shift up to 24 px |
| `scaleFactor` for the measuring copy | `half_size`, 2×2 binning without demosaic | different texture; levels agree within 0.006 stop |

## 5. Recommendations

Each one keeps LibRaw as the only demosaicer and decides nothing on its own: they are constants.

1. **Pin `adjust_maximum_thr = 0`.**
   - **Cost:** one line in `pinned.c`, and a test that it is pinned. The twin is a decode with the
     threshold at 0.75 on a frame whose maximum is lifted above it.
   - **Risk:** none on roll #8, where it never fires.
   - **Gain:** it removes a per-frame exposure decision before it bites a scan exposed to the right.
2. **A constant chroma denoise at decode, the twin of Swift's colour NR 1.**
   - **What:** a Gaussian of about 1.25 px on log R, G, B minus their luma, at full force, on the mosaic
     branch only. Linear TIFF and DNG renders must not move.
   - **Calibration:** against `CIRAWFilter` at colour NR 0 and 1, on roll #8 and the S1R, by band. The
     result is proof by difference against Apple, not taste.
   - **Cost:** one separable kernel run once per decode, with a scalar CPU reference and a twin (blur
     off must fail the band bound).
   - **Risk:** every RAW render moves once more, towards Swift. Keep it in ONI code, not in LibRaw's
     callbacks, so it stays floating point.
   - **Slider:** none until use proves the constant wrong.
3. **Keep AHD.**
   - DCB takes 4.6 times as long for −22 % colour detail.
   - DHT takes 4 times as long for −37 to −48 % colour detail and +16 to +20 % green detail.
   - AAHD is worse on both.
   - None reaches Apple's −75 %, and each change moves every render. Revisit only if (2) leaves
     visible zipper.
4. **Leave FBDD, median passes and wavelet denoise off.** The first two are slow for a fraction of the
   effect. The wavelet one is a luma denoise that eats grain, which the Swift app refused.
5. **Measure the remaining green gap after (2), in the real pipeline**, before any luma filter.
   - **The gap:** about 2 times Apple at pixel scale, within 3 % from 2 px.
   - **The fallback, should it show:** the Details sharpen at rest, which runs on a crisper base in
     Rust. Its rest is a Swift value, so this is the owner's call, not a constant to change silently.
6. **Record the lens gap in `docs/parity/`.**
   - **The gap:** frames with `lensCorrected` cannot get Apple's lens profile from LibRaw.
   - **The choices:** a lens library (lensfun, LGPL, outside LibRaw) or a Mac-only path through Apple.
     Both are larger decisions.

## Reproduce

- **The harness.** It lives outside the repository: `apple.swift`, `lr.c` and two readers. It builds
  with the toolchain `swiftc` and `clang` as `scripts/decoder-gap/run` does. LibRaw comes from
  `crates/oni-decode/libraw`, compiled apart with `-O2`, with no cargo involved.
- **If it should stay,** `scripts/decoder-gap/` is the natural home. One frame takes about 45 s for
  every arm.
