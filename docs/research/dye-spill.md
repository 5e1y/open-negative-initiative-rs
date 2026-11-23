# The spill a masked negative leaves on a camera scan, and the small matrix that mends it

The owner's question (task #3650): with dye separation removed from both pipes, can the data of every stock we hold make "a small something" against the spill left after the per-channel windows, without spectra at run time? Measured on the 48 data-sheet profiles, spektrafilm's former profiles (git history, `44fcc040`), and real scans; nothing of the app changed. Scripts in `docs/research/dye-spill/`, the renders by `crates/oni-pipeline/examples/dye_spill.rs`.

## What "spill" is once each channel has its window

About a mid-grey, each layer adds density to each scan channel. A window per channel divides each channel by its own neutral slope, so greys never show spill; what is left is a 3x3 of shares, rows summing to 1: how much of a channel's windowed density each layer brings (`spill.py`, `Stock.spill`). Its inverse hands each channel back to its own layer and leaves every grey where it was.

## What the sheets and spektrafilm say (modelled: `study.py`)

Shares on a Rec. 2020 scan under D50 with the 1931 observer as the camera, ONI's own scan model; rows R G B, columns cyan, magenta, yellow layer:

| Source | R | G | B |
|---|---|---|---|
| 48 data sheets, mean | 0.69 0.30 0.02 | 0.23 0.68 0.09 | 0.06 0.09 0.85 |
| spektrafilm, 21 negatives, mean | 0.67 0.31 0.02 | 0.14 0.81 0.05 | 0.02 0.07 0.91 |
| the same dyes read on Status M (sheets / spektrafilm) | 0.91 0.08 0.01 / 0.96 0.03 0.01 | 0.13 0.78 0.10 / 0.04 0.93 0.03 | 0.07 0.05 0.89 / 0.01 0.03 0.97 |

1. **The mask does its work for a paper, not for a camera.** On Status M, close to what a paper reads, spektrafilm's dyes leave 3 to 7 % of spill: the mask removes nearly all of it, as the owner says. On a camera's broad channels the same dyes leave 19 to 33 %: the red channel reads the magenta dye's long flank (580 to 620 nm), the green reads cyan's short flank. This is channel overlap, which no mask touches. Nordiclab's own figure for 100 nm bands is 36 to 41 % (no mask in its model), 10 to 20 % for 20 nm bands.
2. **The two sources agree on red (magenta brings 30 %) and disagree on green** (cyan brings 23 % or 14 %). The Vision3 sheets print a cyan with 0.10 to 0.20 of its peak at 550 to 565 nm; spektrafilm reshaped it to about zero there. A sheet's mid-scale neutral cannot settle this: a cyan tail in green and a little more magenta fit it equally. This is the inferred part of the 44 sheets that print no dyes, and why the data-sheet separation over-corrected.
3. **The camera's red edge moves the largest term more than the stock does**: a drawn Bayer red opening at 585 nm gives magenta 21 to 23 % of red instead of 30 to 33 %; a drawn white LED instead of D50 adds 4 points. Per family the sheets differ by less (Kodak 0.32, Fujifilm 0.29, Vision3 0.28, standard deviation 0.05 to 0.08) than the sources do. A per-stock spill from the sheets would be noise.

## What real scans say (measured: `hull.py`)

Every pixel's density over the bare film is a sum of the three layers' columns, none negative, so a scan can never show a purer channel than a layer alone gives: the smallest G/R ever seen is at least the cyan layer's own G/R. Five rolls were read (86 frames: Gold 200 twice, UltraMax 400, Portra 800 on the Lumix, roll #8 on the Nikon). Four hold no colour saturated enough to bind. The UltraMax 400 roll holds two aquarium frames that do: a column lit by red LEDs (P1072365, nearly the red layer alone, 3 300 blocks clustered at G/R 0.18 to 0.22 and B/R 0.12 to 0.17), and a blue-green tank (P1072362, no red layer, R/G down to 0.11 to 0.15).

| One layer alone, other channels per unit of its own | sheet model | spektrafilm model | real scan, at most |
|---|---|---|---|
| cyan: G/R | 0.34 | 0.12 | **0.20** |
| cyan: B/R | 0.09 | 0.02 | 0.12 |
| magenta: R/G | 0.28 | 0.25 | **0.13** |
| yellow: G/B | 0.08 | 0.02 | 0.11 (not binding) |

Both models overstate the largest term, magenta in red, by two: the real camera's red sits further from the magenta dye than the observer's. The sheets overstate cyan in green by 1.7; spektrafilm understates it. A second check on the same frames: a mended share under zero means less dye than the bare film, which no layer can hold. Share of well-exposed pixels pushed under zero:

| Matrix | red column, green | tank, red |
|---|---|---|
| inverse of the sheets' mean | 89 % | 65 % |
| inverse of spektrafilm's mean | 60 % | 55 % |
| the proposal below | 24 % (median +0.011) | 1 % |

## The proposal

One generic spill for every colour negative, as shares a stock file can carry and a person can read, with a dose; off (dose 0) by default until the owner judges it:

```
"spill": { "red": [0.88, 0.12, 0.00], "green": [0.16, 0.80, 0.04], "blue": [0.09, 0.06, 0.85] }
```

The pipe lays `I + dose x (inverse(spill) - I)` on the three window shares (density above the window's black over its span, before the window's own tone): at dose 1, `[1.167 -0.176 0.008] [-0.228 1.289 -0.061] [-0.108 -0.072 1.180]`. Rows sum to 1, so every grey, the black and the white stay where the windows put them; no spectrum, no solve, nine numbers. It raises the red-green difference of densities by 1.43 and blue-yellow by 1.21; the former spektrafilm separation's first order is 1.94 and 1.13, the data-sheet one 2.4 and 1.25.

Where the numbers come from: cyan in green and blue, and magenta in red, are the real-scan bounds above (upper bounds, the red column's a tight cluster); magenta in blue (0.07) and yellow in green (0.05) are not bound by any frame and take the lower of the two models. Every stock file would carry the same values today; a stock gets its own only from a measurement.

Honest strength: dose 1 is the upper bound the pictures allow, measured on one stock and one camera. Dose 0.5 is the safe rest if one is wanted; I would ship it off.

## On pictures

`dye_spill` renders each frame on ONI's own print at rest on its sidecar's windows, CPU reference only: `off` (no separation, the pipes' future), `half` and `on` (the proposal at 0.5 and 1), `spektra` (inverse of spektrafilm's mean spill), `old` (the separation kernel on the stock's former spektrafilm dyes), `now` (the kernel on today's data-sheet dyes). Roll #8 (Portra 800): DSC_6440, 6452, 6464; Gold 200: P1072457, 484, 469; UltraMax 400: P1072365, 362, 353. Images kept outside the repository, in `/Volumes/T7/oni-build/scratch/dye-spill/out/spill-<frame>.jpg`.

- `spektra` and `old` are the same picture to the eye on every frame: the former separation was, to first order, one 3x3. Nothing is lost by dropping the spectra.
- `off` is the flattest: foliage olive, reds toward orange, skies pale. `on` deepens greens and reds without moving a hue; skin gains a little red, asphalt and walls stay put. `half` is hard to tell from `off` without the pair side by side.
- `spektra`, `old` and `now` turn the orange banner of P1072457 red and its pavement pink, skies cyan-teal (P1072484), and flatten the red column of P1072365 to one red, its orange highlights gone; `now` also tints roll #8's asphalt teal. That is the over-correction the owner saw, and the pictures' own evidence that both models are too strong.

## Limits

- The measured terms rest on two frames of one roll (UltraMax 400, Lumix, the owner's light). The Nikon's and other stocks' are assumed alike; the sheets say stocks differ less than cameras do.
- The bounds are upper bounds: a scene is never one layer alone. The bare-film pick moves a ratio read at 0.3 to 0.6 of density by about 0.05.
- First order only: on the models a fixed matrix leaves 0.04 to 0.13 stop rms of the 0.4 to 0.5 it mends, more on colours near a channel's zero.
- The matrix was laid before the window's own tone; with unequal mids per channel a grey would need it after. Where it sits is the density pipe's call.
- What would settle it: one frame per stock of red, green and blue LED light (a phone screen filling the frame, one colour a frame), or one negative scanned once in white and once in narrowband light. `hull.py` reads either.
- Not looked at: the paper print, slides, black and white (no spill to mend), glare, which the Nordiclab study ranks above the separation model.

## Run again

```
python3 docs/research/dye-spill/study.py [folder of former spektrafilm profiles]
cargo run -p oni-pipeline --example dye_spill -- <out> 1200 <former profile | -> on=1.1675,-0.1757,0.0083,-0.2281,1.2888,-0.0606,-0.1075,-0.0724,1.1799 -- <raw>...
python3 docs/research/dye-spill/hull.py <out>
python3 docs/research/dye-spill/view.py <out> <sheet.jpg> <frame> off,on
```
