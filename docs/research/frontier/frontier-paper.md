# The Frontier SP-3000 as a paper

The Frontier becomes one more print in the Paper list, beside spektrafilm's five. It takes what every paper takes (the negative's dye amounts above its bare film, after the stock's dye separation) and answers Lighting as they do since #3010: true density before its curve, no ONI window, no colour compensation. No Hunt, no window, no resting offset: the D+0.5 M+2 Y-2 rest belongs to the cube path of the Scanner face, which stays as it is until that face is retired.

## The model

Read from Fuji's patents (`recherche-frontier.md`), kept to what a table of uniform patches can show:

1. **Scanner read.** The table's input `x` is the synthetic negatives' own density above base, per channel, with no crosstalk: the quantity the paper's dye amounts are. The print reads the amounts as `x`, at scale 1. A Status M read of the real dyes would put back the crosstalk the separation removed and the table never saw. Today's cube path stretches the scan by 1.3 / the green window (0.96 on roll #8, 1.36 of density) and unmixes with the Swift generic rows at 0.6; the paper reads true density and the stock's own dyes at 1.
2. **CMYD offsets.** D is the enlarger's time: Exposure +1 is a stop less light, log10 2 of log exposure on every layer. C, M, Y are the filtration, a CC a hundredth of density on its own layer alone (Temperature and Tint, 30.1 CC a stop).
3. **Negative density to paper exposure (Matrix2).** A 3 × 3 with rows summing to one, so a neutral density step is a time step: `[0.963 0.035 0.001] [-0.041 1.090 -0.048] [-0.010 0.120 0.891]`.
4. **Sensitometric curve (LUT4).** Each layer's density a sum of three normal integrals of its log exposure, laid on the kernel's 256 nodes over −2.5 to 2.5; Contrast turns it about the reference grey (4^C), as on every paper. Dmax 2.62 / 3.18 / 2.99.
5. **Output colour stage.** Nine virtual dyes, each a density per layer plus a base and a linear Rec. 2020 light: `light = Σ view_b · 10^−(dye_b · D + base_b)`, a matrix in density, then a power, then a matrix in light. It carries the hue turns and the gamut edge the table demands: fitted unclipped against the table's clipped code, the print keeps inside sRGB as the machine renders (0.2 % of the constrained nodes under −0.005 of light, none over 1). The least light any mix of the three layers prints is −1.6e-5; the paper's white is 0.98.

It runs **in the existing paper kernel**, with no new WGSL: the kernel's spectral sums run over three bands of the negative (one a layer, each a row of the matrix, each dimmed by its own filter only) and nine bands of the print instead of the wavelengths. Nothing of the kernel changed; its scalar reference is the CPU reference. The fit is `frontier_print_fit.py` (numpy, deterministic, 20 s), writing the "frontier" block of `crates/oni-settings/stocks/fujifilm-frontier-sp-3000.json`.

Hypertone (local range compression) and Hyper-sharpness are spatial: no table of patches holds them, and the print leaves them out.

## Fit

ΔE is Oklab distance × 100 of display-clipped code, on `frontier.cube`'s nodes inside its reach. Constrained nodes are those every measurement reads (channel spread under 0.6).

| Against the measured table | Nodes | Median | p90 | p99 | Max |
|---|---|---|---|---|---|
| **Frontier paper, constrained** | 1 440 | **1.04** | **2.41** | 4.46 | 7.08 |
| Frontier paper, mostly read (spread < 1.0) | 3 584 | 1.44 | 3.20 | 5.77 | 10.76 |
| Frontier paper, the whole reach | 7 581 | 2.41 | 7.30 | 19.89 | 34.69 |
| Look Lab model (`FITTED.frontier`, sectors, grey cast), constrained | 1 440 | 1.19 | 2.99 | 5.40 | |
| Twin: the paper's curves linearised, constrained | 1 440 | 33.4 | 51.8 | | |
| The cube path itself | its nodes | 0 by construction | | | |

The paper's kernel, in f32 on its 256 nodes, reads 1.04 / 2.41 (`tests/frontier_paper.rs`). The 63 real renders (`Scripts/scanner/data/measurements.json.gz`, Swift repository) are not on this machine, so the print is not measured against them: the cube reads them at 0.27 / 0.81 / 2.53, so the print stands within about 1.3 median of them by the triangle, unmeasured. The separable floor the first analysis found (about 1.6 on the renders) holds here: nine output dyes bring the constrained median from 1.29 (three) to 1.04, more bands extrapolate wildly past the reach; the rest of the table is hue at mid chroma, as before.

Variants tried and left: two negative bands a layer (no gain), non-negative output lights (1.28 median), a penalty holding light above none on every density mix (1.21 to 1.39).

## Blues and skins

A scene shot as a digital photo on the virtual Portra 800, printed at rest (`examples/frontier_print_traits.rs`): 96 blues (Oklab h 230 to 265°, L 0.4 to 0.7, C 0.06 to 0.12) and 96 skins (h 40 to 75°, L 0.5 to 0.8, C 0.04 to 0.10); hue turns against the scene, the film's own turn included.

| Print | Blues Δh | Blues chroma | Past 270° | 247° blue → | Skins Δh | Skins chroma | Skin red (a) | 58° skin → |
|---|---|---|---|---|---|---|---|---|
| **Frontier paper** | −2.3° | ×0.92 | 5 % | 237.7° | **+14.2°** | ×1.21 | **×0.62** | 74.6° |
| Today's Scanner-face Frontier | −8.2° | ×1.11 | 9 % | 226.2° | +10.1° | ×1.35 | ×0.72 | 69.2° |
| Portra Endura | −11.3° | ×0.92 | 0 % | 230.1° | +6.8° | ×1.18 | ×1.04 | 66.8° |
| ONI's print | −11.7° | ×0.97 | 2 % | 233.5° | +4.7° | ×1.31 | ×1.00 | 64.6° |

The machine's traits come through, from the table itself: against ONI's print its blues sit 9.4° toward violet, its skins 9.5° toward yellow with 38 % less red. The film turns blues toward cyan first (−11° on ONI's print), so few cross 270°. Today's look keeps ONI's tone and the owner's rest, which take part of the drift back. The grey at rest prints neutral by construction; a scene grey a little off the reference (L 0.6) prints at C 0.012, 49°, as the machine's greys drift.

## Rest, Lighting, ends

The enlarger rests as on every paper (#3010): time, magenta and yellow so the reference grey prints at a plain RAW's grey (0.398), neutral; black and white by the time alone. The least filter now lies at none at rest too, so the Frontier, whose solve lands under none (M −9.5, Y −14.6 on the checks' negative), reads C 14.6, M 5.1, Y 0. The print's ends are its own: white 0.98, Dmax at no light (2e-5), so True blacks moves it by about 2e-5 of light: the Frontier's blacks are already black. On the Frontier paper the Scanner face's Frontier machine lays no resting offset: the rest is the cube's.

## Renders

`examples/frontier_compare.rs` on roll #8 (DSC_6438, 6444, 6452, 6456, Portra 800) and P1073076: today's Scanner-face Frontier, the Frontier paper and Portra Endura at rest, and the Frontier paper at Exposure −1 / 0 / +1. Roll #8's photos are not published: the JPEGs stay outside the repository. The paper prints darker than today's look (its grey at the reference grey, today's lifted by ONI's look's tone), cooler in the shadows than Endura, with yellows more lemon than gold; at +1 its sky goes cyan-green as the curve's blue shoulders first.
