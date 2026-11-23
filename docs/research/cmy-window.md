# Prepare's window on the dye layers (CMY), before the paper

The owner's question: replace the enlarger rest and the black anchor by ONI's classic triple window, Prepare the Roll's per-channel black, white and mid, read on the dye layers (C, M, Y amounts after the dye separation, or a digital photo's virtual film layers) and laid on the negative before the paper. Measured on throwaway prototypes (task #3570); nothing of the app changed.

## What Prepare already measures, and what the paper reads of it today

- **Prepare the Roll** (`oni-measure`): each frame is rendered up to `Stage::Levels` (`truncate.rs`: print bypassed, windows on the graduation, **separation at 0**, so it reads the scan's densities), its gains brought onto green's median (`auto_levels::recentred`), each channel's tails placed (`auto_levels::placement`, Classic); the roll combines the frames (`roll::combined`: gains on the median, each end's chroma on the trimmed median, its depth at the 20 % / 80 % frames, the picked base as every channel's black). Each frame then stores `levels` (black, white, mid, shadows, highlights per channel, on scan density) and `rollReference`. On roll #8 every frame carries the roll's window (spans 1.18 / 1.36 / 1.78 of density): the window is per roll, never per frame.
- **ONI's print** reads the whole window (stage 5, `apply_window`), then its print curve at rest; a dye-separated scan reaches it through `separation::toned`, which already carries the RGB window onto the dyes (each neutral grey the windows print, unmixed).
- **The paper** reads three things of it (`paper::anchors`): the black as the zero of the amounts (the bare film), the reference grey (where the windows read the share a plain RAW's grey prints at, so black, white and mid all shape it), and the white only as the span Lighting's points work in. The enlarger rest (time, M, Y) prints that grey neutral at a plain RAW's grey; the black anchor lays the printed bare film on pure black, in light.

So today's paper path is already "Prepare's window, its grey turned into the enlarger's filtration"; what the window's ends do not set is the grade per layer, which stays the paper's.

## Prototypes

The per-layer window was laid in the dye separation's own per-layer tone table (`DyeSeparation::tone`, GPU and CPU alike, no new kernel), between the separation and the paper; the RGB window was carried to the dyes (black → 0, densest → its amounts, grey → its amounts), as a migration would. Targets on each paper: its own black b* (97 % of each layer's density range), its neutral grey g* (a plain RAW's grey, neutral), its own white w* (3 %). Measured with `examples/tone_response` on roll #8 (DSC_6440, 6452, 6464, Portra 800, the roll's window) and the digital ramp (virtual Portra 800), Lighting at rest.

| Variant | What the window does |
|---|---|
| **T** today | enlarger rest on the grey, bare film anchored on pure black in light (Paper blacks off) |
| **TB** today, Paper blacks on | the same, the paper's own ends |
| **Wn** classic ends | each layer: bare film → paper black, grey → grey, densest read → paper white, along the paper's neutral axis (the amounts that print neutral, by mean amount) |
| **Wbg** black in exposure | each layer scaled so bare film → paper black and grey → grey, straight |
| **Wr** window at the enlarger's rest | each layer read about its own grey, scaled so the bare film, the grey and the densest land on the same mean amount on every layer (unit grade on the mean), then the paper's neutral axis |

Royal Digital, scan ramp (slopes in L* a stop; spread = largest channel departure of the grey ramp, −4 to +3 stops):

| | L* −4 | −3 | −2 | −1 | 0 | +1 | +2 | +3 | +5 | slope@0 | shadows | highlights | spread |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| T | 0.9 | 4.6 | 19.6 | 44.7 | 69.2 | 84.2 | 89.2 | 90.0 | 90.7 | 20.3 | 20.1 | 2.9 | 0.183 |
| TB | 4.0 | 7.7 | 20.9 | 45.1 | 69.4 | 84.2 | 89.2 | 90.0 | 90.7 | 20.2 | 18.7 | 2.9 | 0.115 |
| **Wr** | 4.3 | 7.6 | 20.2 | 44.3 | 69.3 | 84.6 | 89.5 | 90.1 | 90.6 | 20.9 | 18.4 | 2.8 | **0.025** |
| Wn | 3.1 | 3.9 | 9.5 | 34.5 | 68.4 | 73.5 | 77.1 | 80.3 | 85.1 | 18.7 | 15.3 | 3.4 | 0.034 |
| Wbg | 2.3 | 2.9 | 7.1 | 31.2 | 69.3 | 87.5 | 90.0 | 90.6 | 90.7 | 30.2 | 14.2 | 1.6 | 1.055 |

Other papers, scan ramp slope@0 / L* at +2 / spread, and the digital ramp's spread:

| Paper | TB | Wr | Wn | digital spread TB → Wr |
|---|---|---|---|---|
| Endura Premier | 23.2 / 89.0 / 0.315 | 24.0 / 89.1 / **0.052** | 15.1 / 76.6 / 0.031 | 0.521 → 0.317 |
| Frontier SP-3000 | 18.4 / 93.3 / 0.487 | 19.7 / 93.5 / **0.028** | 14.7 / 81.3 / 0.029 | 0.538 → 0.249 |
| Royal Digital | 20.2 / 89.2 / 0.115 | 20.9 / 89.5 / **0.025** | 18.7 / 77.1 / 0.034 | 0.312 → 0.239 |

Photos (median L*, Royal): DSC_6440 T 32.6, TB 33.3, Wr 32.3, Wn 20.9, Wbg 17.4; DSC_6452 11.8 / 13.9 / 13.1 / 5.5 / 4.1.

What the numbers say:

1. **Prepare's ends laid on the paper's ends (Wn) is not the classic window's freedom, it is another print.** The roll's densest read sits about +5 stops over the grey, the paper whitens at +2 to +3.5: laying one on the other squeezes the highlights (L* at +2: 77 against 89 on Royal, 81 against 93 on the Frontier) and drops the mids 8 to 35 % (Endura 23.2 → 15.1); laying the bare film on the paper's black takes 10 L* off the −2 stop. The photos print 6 to 12 L* darker in their middle. The paper's shoulder and clip, its character, are gone: the measure decides the tone, not the paper.
2. **The black on the paper's black in exposure (Wbg) costs 49 % of mid contrast** (slope 30.2 against 20.3), clips at +1.75 and tints the greys (spread 1.06). The black anchor in light costs under 1 % of mid slope (T 20.3, TB 20.2): laying black is a matter of the display, not of the negative. The anchor is no hack; it is in the right place.
3. **The window at the enlarger's rest (Wr) keeps today's tone within 1 L* at every stop** (against TB, the paper's own ends) **and makes the greys 4 to 17 times more neutral** on a scan (spread 0.025 to 0.052 against 0.115 to 0.487), a quarter to a half less tinted on a digital photo. This is what the per-layer window brings that the enlarger cannot: three neutral points per layer (bare film, grey, densest) instead of one (the grey), so a negative's crossover no longer tints its shadows and highlights. On the Frontier it removes the teal cast its shadows and road take today (its greys drifted by 0.49), which may be part of what the owner calls its look: to judge by eye.

Images (roll #8, not publishable, kept outside the repository): `/Volumes/T7/oni-build/scratch/window-design/out/m-royal.jpg` (T, TB, Wn, Wbg on Royal Digital) and `m-rest.jpg` (TB and Wr on Royal Digital, then on the Frontier).

## Verdict

Yes to the CMY window as the data and the controls; no to its classic rest. Read on the dye layers, Prepare's window is the right model for every pipe (scan, digital, any paper) and replaces the per-frame filtration with three neutral points per layer, editable. Its rest must be the enlarger's (Wr), not the measured ends on the paper's ends (Wn): the ends set each layer's balance, the paper keeps the grade. The black anchor is not replaced on the negative: it becomes Print lift's presentation step.

## Design

**Data.** `levels` keep their shape (`Levels` per layer: black, white, mid, shadows, highlights) on a new axis: dye amount above the bare film, per layer C, M, Y. The roll reference keeps its fields on that axis. A black and white negative keeps one layer.

**Prepare, what carries over unchanged.** `roll::measured`, `roll::combined` (gains median, TRIM and DEPTH, the base pick as the black), `auto_levels::placement` and `suggest` (Classic tails), `recentred` (the gains step stays on the scan: the camera gains are how the scan is read, and the base pick sets the bare film every amount lies above), the pipette, the sheet reads, `measure_roll`'s flow and its writing.

**Prepare, what changes.** `truncate::truncated(Stage::Levels)` reads the dyes: the separation at the stock's dose instead of 0, the histogram on the amount axis instead of the density graduation (one more `graduation` for amounts, its span set by the stocks' densest amounts). A digital photo's window is its stock reference read the same way, deterministic. The base pick reads 0 on every layer by construction (the amounts lie above it), so the black is no longer a measured value but the bare film itself; the white is the roll's densest read per layer; the mid is where the reference grey lies.

**Where it sits.** After Lighting on the negative, before the paper: the `Negative`'s own last per-layer table (`Negative::printed`, today straight on a paper) carries the window and the paper's neutral axis, so Lighting's moves act on the film in log exposure first, then the window reads the moved negative as a print does. No new kernel: both tables already exist on GPU and CPU; the neutral axis is baked per paper and frame like the enlarger rest is today (a 96-node solve). ONI's print reads the same window: `separation::toned`'s carriage of the RGB window onto the dyes is no longer needed, the window being on the dyes already.

**The window's rest.** Each layer read about its grey: under it scaled so the bare film lands on the mean grey's distance, over it so the densest read lands on the mean span, all three on the paper's neutral axis; the grey on a plain RAW's grey. Black, grey and densest print neutral on every paper, the grade is one on the mean, so the paper's toe, shoulder and clip stay its own.

**What the person sees.** In the colour settings, per layer C, M, Y: black, white and mid handles on the negative's histogram (amounts), at Prepare's measure. Moving a layer's black or white turns that layer's grade about its grey (a crossover correction); moving its mid moves its grey (what the filtration did per frame). Global black and white points move the three together. These are today's More color settings on the negative (`channel_black`, `channel_white`, `channel_mid`, `black_point`, `white_point`) given Prepare's measure as their rest instead of zero.

**The enlarger rest and the black anchor become.** The enlarger keeps a rest per paper, solved once on the paper's neutral grey: a constant the paper block shows, no longer a per-frame filtration; Temperature and Tint stay Lighting's moves. The black anchor leaves the paper's ends: Print lift off lays the paper's own black on pure black and its white on pure white in light after the print (measured cost under 1 % of mid slope), Print lift on shows the paper's own ends.

**Migration.** A sidecar's RGB window carries to the dyes as the prototype did: black → 0, densest → its amounts, the grey → its amounts, exactly by the neutral greys `separation::toned` already traces. A frame's render moves by the gained neutrality (its tone within 1 L*), so a photo edited before keeps the enlarger rest on a legacy flag unless the owner wants every photo to move, as for the spektrafilm paper (new stages off for old photos). Format bump; an older build opens it read-only.

**The datasheet unmix** (`datasheet-unmix`, the Status M/A curves read as amounts) softens every datasheet paper (Royal Digital's slope@0 20.3 → about 17.7, clip +2.5 → +3.0 in the diagnosis runs). The window re-solves its targets through whatever curves the paper holds, so the design stands; Wr keeps tracking the paper's own tone, and Wn's squeeze would ease a little as the paper's white moves out. The spreads above were measured before the unmix and should be measured again after it.

**Not measured.** A slide through its internegative, a black and white negative, Lighting moved, a real amount histogram (the window was carried from the RGB one, not measured on amounts), the app's own Prepare flow.
