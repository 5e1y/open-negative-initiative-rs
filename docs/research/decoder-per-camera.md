# Apple against LibRaw, camera by camera

Measured on 2026-10-20 (#180) on every RAW this Mac holds, read only. Nothing in the app changed. It extends
`decoder-apple-vs-libraw.md` (#165) from one camera to three, and reads the gap in the rendered image.

## Short answer

- **The shift is not universal: it follows the camera.** On the Nikon Z f the Rust render leans
  **red-orange** (Oklab hue 40°, 1.5 to 2.0 ΔE×100 on the greys). On the Lumix S1R it leans **pure yellow**
  (hue 92°, 2.6 to 3.1), with almost no red.
- **In the owner's controls.** The Z f needs **Red ↔ Cyan +0.085 EV** (toward cyan) and
  **Blue ↔ Yellow −0.087 EV** (toward blue). The S1R needs +0.04 and **−0.29**.
- **Within one camera it is a constant.** The Z f varies by 0.001 to 0.002 EV across 38 frames. One S1R
  matrix serves two different rolls within 0.005 stop.
- **The owner's −8 matches the Z f's red exactly** (0.08 against 0.085), read as eight hundredths of an EV
  toward cyan. His −2 in blue removes a quarter of the blue part. That leaves 0.5 ΔE of yellow on the
  greys and 11 % more chroma on skin.
- **The crushed blacks and the extra chroma both come from the decode, not the pipeline.** Rendering
  LibRaw through the fitted 3×3 matrix removes both, every other stage unchanged.
  - **Blacks.** Pro face: 3.4 % of pixels black in Swift against 6.1 % in Rust.
  - **Chroma.** Once the cast is cancelled, chroma is equal at 8 px (×1.00). What reads as saturation is
    the cast left over, plus LibRaw's pixel-scale colour noise, which Apple's colour NR removes
    (`libraw-knobs.md`).

## 1. Inventory (mounted disks: the internal one only; the external T7 is unplugged)

| Camera | Files | Kind | Swift sidecars |
|---|---|---|---|
| Nikon Z f | roll #8, 38 NEF (the original, its test copy and temporary copies) | colour negative | 38: 24 Pro, 14 Scanner (ONI machine) |
| Panasonic DC-S1R | a Kodak ColorPlus roll shot on a Leica M6, 38 RW2, a copy in an earlier session's scratchpad | colour negative | 38: 25 Pro, 13 Scanner (Frontier), 1 monochrome |
| Panasonic DC-S1R | `P1072392` (calibration frame, copies in `oni-adversarial`), `P1073044` (B&W negative), `P1072740` (flat-field preset) | negatives | adversarial or monochrome only |
| Fujifilm X-E4 | `DSCF0410.RAF` | digital photo, positive | none |
| unknown (Adobe tutorial `masking.dng`) | one DNG in Camera Raw's bundle | digital | none; LibRaw refuses it (error −2), and DNGs stay out of the table |

The Lumix calibration folder is absent, the Swift repository holds no RAW fixture, and Spotlight finds none in
iCloud Drive or Pictures. That makes **three camera models**, two of them on film with Swift sidecars.

## 2. The decode alone: Apple over LibRaw, stops (median of 8-px blocks, relative to green)

| Camera, frames | R − G | B − G | Frame sd | Exposure (G) |
|---|---|---|---|---|
| Z f, roll #8, 38 | **+0.089** | **−0.097** | 0.005 / 0.003 | −0.028 |
| S1R, ColorPlus roll, 38 | **+0.043** | **−0.274** | 0.009 / 0.034 | −0.025 |
| S1R, `P1072392`, other roll | +0.049 | −0.313 | one frame | −0.013 |
| S1R, `P1073044`, B&W negative | −0.007 | −0.066 | one frame | **−1.15** |
| X-E4, positive scene, midtone greys | +0.025 | +0.144 | one frame | **+1.54** |

- **The matrix belongs to the camera, the gains to the camera and the film.**
  - The S1R roll's matrix applied to `P1072392`, another roll, leaves 0.002 / 0.005 stop.
  - Swapped between the two cameras, the matrix leaves 0.20 stop in blue either way.
  - The B&W negative's gains differ from the colour roll's: same camera and matrix, different spectrum.
- **Two exposure gaps, both from `BaselineExposure`.**
  - On the S1R B&W frame, Apple reads −0.6, while LibRaw has no value and Rust falls back on 0.48.
  - On the X-E4, Apple reads 2.02.
  - Both gaps dwarf the colour lean.
- **The X-E4's gap depends on the level.** In its greys it is +0.22 / +0.35 in the darks against
  +0.03 / +0.14 in the midtones. Only one positive frame exists, so its colour stays unmeasured on film.

## 3. What the eye sees: the Rust render minus the Swift render, Oklab ×100, greys of L 0.3–0.85

Each frame's two decodes go through the same Rust pipeline with the same sidecar, the one the Swift app
wrote. The render on Apple's decode stands for Swift's. Step 4 of the Frontier lab measured the real
`--render` against Rust at 0.019–0.027 mean worst-channel gap, the size of the decode's alone (0.018).

| Camera, face | Frames | Δa (red+) | Δb (yellow+) | Cast | Red ↔ Cyan | Blue ↔ Yellow | C / Y clicks |
|---|---|---|---|---|---|---|---|
| Z f, Pro | 24 | +1.15 | +0.96 | 1.5 at 40° | **+0.083** | **−0.086** | |
| Z f, Scanner (ONI machine) | 14 | +1.49 | +1.31 | 2.0 at 41° | +0.086 | −0.087 | **C +1.1, Y −1.1** |
| Z f, every frame on the Frontier | 38 | +0.70 | +1.48 | 1.6 at 65° | +0.087 | −0.095 | C +1.5, Y −1.3 |
| Z f, roll prepared again in Rust | 38 | +1.20 / +1.55 | +0.91 / +1.19 | ~1.6 at 37° | +0.087 | −0.083 | C +1.2, Y −1.0 |
| S1R, Pro | 23 | −0.03 | +2.63 | 2.6 at 91° | **+0.040** | **−0.292** | |
| S1R, Scanner (Frontier) | 12 | −0.20 | +3.05 | 3.1 at 94° | +0.059 | −0.242 | **C +0.9, Y −4.4** |

- **The correction columns.** Each column gives the setting, added in the Rust app, that brings the greys
  to zero. It is solved from probe renders, and the Z f column, rendered, leaves under 0.05.
  - **Spread across frames**: 0.001 to 0.002 EV on the Z f, 0.005 / 0.06 on the S1R in Pro.
  - **The roll prepared again in Rust** keeps the Swift sidecar with the test copy's own gains, levels
    and roll reference, so a new roll balance does not remove the lean.
- **Colours, uncorrected.**
  - Z f: skin −4.6° and ×1.34 chroma, sky +13° and ×0.61.
  - S1R: skin +8.4° and ×1.37, sky ×0.41.
  - The cast feeds the warm colours and drains the cool ones.
- **Gains alone are enough on the Z f, not on the S1R.**
  - Z f: once corrected, chroma is ×1.00 and every hue within 1.4°.
  - S1R: chroma ×0.94, sky −8.6° on Pro, skin −8.8° on the Scanner face.
  - The fitted matrix closes both: hue within 1.7°, chroma ×1.00.

## 4. The owner's roll #8 correction, checked

The sign convention, explicit:

- **The labels.** In negative mode the Source Balance rows read `Red ↔ Cyan` and `Blue ↔ Yellow`, low end
  first (`roll_tab.slint`, Swift's `GainPanel` `invertedSense`).
- **Raising a gain moves the print toward the complement.** A higher value is more of that channel's
  light through the negative, so less density, so less of that colour on the print: right is cyan, right
  is yellow. `senses.rs` checks it.
- **On the negative.** LibRaw reads the red 0.089 stop darker against green, a red density 0.027 higher,
  which is "more cyan dye" on the negative. After inversion that is more red on the print. The blue is
  read 0.097 stop brighter, a lower yellow-dye density, so more yellow on the print.
- **The cure is therefore toward cyan and toward blue.** In Rust: Red ↔ Cyan up, Blue ↔ Yellow down,
  Scanner C+ and Y−. In Swift, the same amounts with the opposite signs.

| Correction on the Z f (Rust app, Pro) | Greys left (Δa, Δb) | Cast | Skin chroma | Sky chroma |
|---|---|---|---|---|
| none | +1.27, +1.09 | 1.7 | ×1.34 | ×0.61 |
| owner: Red ↔ Cyan +0.08 (toward cyan), Blue ↔ Yellow −0.02 | −0.09, +0.52 | 0.5, yellow | ×1.11 | ×0.88 |
| owner's numbers in the other sense (−0.08 toward red) | +2.71, +1.28 | 3.0 | | |
| measured: +0.085, −0.087 | −0.01, −0.01 | 0.0 | ×1.00 | ×1.01 |

The owner's red is the measured red. His blue leaves half a ΔE of yellow, below the usual
just-noticeable step, and that yellow is the "hair more saturated" in warm colours (skin ×1.11). On an S1R
the same eye would need about +0.04 and −0.29, close to nothing in red and three times the blue.

## 5. Blacks and chroma: decode or pipeline?

| Arm (Z f, Pro face) | Darkest 5 %, Oklab L ×100 | Pixels under L 0.03 |
|---|---|---|
| Swift (Apple decode) | 5.4 | 3.4 % |
| Rust (LibRaw) | 3.7 | 6.1 % (18 of 24 frames worse) |
| Rust, owner's correction | 3.2 | 6.5 % |
| LibRaw × the decoder's three gains | 4.8 | 4.0 % |
| LibRaw × the fitted matrix | 5.4 | 3.4 % |

- **The blacks come from the decode.**
  - The black points were placed by Swift on Apple's densities. LibRaw's differ per channel and per colour,
    so more pixels pass under them.
  - The R/B balance does not touch it (3.2 after it). The decoder's own three gains, green's 0.03 stop
    included, take two thirds of it, and the matrix all of it.
  - Scanner face (ONI machine): unchanged (13.5 against 13.4). S1R: the darkest 5 % lose 1.3 points (Pro)
    without clipping, and the matrix restores them.
- **The chroma comes from the decode too.**
  - At 8 px the two decodes agree in chroma once the cast is cancelled: ×1.00 on the Z f, ×0.94–0.99 on the
    S1R.
  - The extra saturation is the cast left over (warm colours ×1.11 to ×1.37), plus pixel-scale colour
    noise. These block renders average that noise out, and `libraw-knobs.md` measures it: Apple's colour NR
    removes 75 % of it.
  - Every arm runs the same pipeline with the same sidecar, so none of it is the pipeline's.

## 6. Verdict

- **Hardware-dependent.**
  - The cast's hue: 40° (Z f) against 92° (S1R).
  - Its size: 1.5–2.0 against 2.6–3.1.
  - The Blue ↔ Yellow cure: −0.09 against −0.29 EV, 3.4 times.
  - The Red ↔ Cyan cure: +0.085 against +0.04, half.
- **Stable within a camera**: frame spread 0.001–0.06 EV, one matrix across rolls. It also moves a little
  with the face (Pro 40°, Frontier 65° on the same Z f) and the film.
- **A single global slider offset fits one camera only.** This confirms #165's per-camera matrix table as
  the cure: it alone also removes the crushed blacks and the S1R's skin and sky hue errors that gains
  leave behind.
- **Before a Fuji entry.** It needs a Fuji negative and the `BaselineExposure` gap settled first.

## Reproduce

```
scripts/decoder-gap/per-camera <out> <sidecar folder> <RAW>...
```

- **`per-camera`** chains three steps:
  - `run`: the decodes and the fit.
  - `render-pairs`: `render_pairs.rs`, compiled with `rustc` against the workspace's release libraries,
    so no crate changes. It renders Apple's and LibRaw's decodes, gain and click probes, and the
    decoder-gain and matrix arms.
  - `analyse.py`: Oklab per frame, then a second pass with the median correction applied.
- **Sidecars and other runs.**
  - The sidecar folder holds `<name>.json` copies, so the originals are only read.
  - `render-pairs` takes a separate Rust sidecar folder for a roll prepared again.
  - X-Trans frames need `ONI_BLOCKS_AT=6,7` for `blocks`.
- **Time.** About 2 min for 38 Z f frames and 4 min for 38 S1R frames.
