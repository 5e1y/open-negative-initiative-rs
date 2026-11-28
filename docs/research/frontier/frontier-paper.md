# The Frontier SP-3000 as a paper

The Frontier is one more paper in the list, a look at the chain's end like every paper (`crates/oni-settings/stocks/README.md`): three curves from the window's share to each layer's dye amount, a 3 × 3 crosstalk, the light, then its ends, the bare film on black and the window's white on white. It runs in the same kernel as the data sheets' papers (`oni-render/src/kernel/paper.rs`) and is baked out of the app by `scripts/paper-looks.py`; nothing about it is solved at run time.

## What the bake reads

The measured table, `crates/oni-pipeline/resources/scanner/frontier.cube`: 27³ nodes of display sRGB code over each channel's density above the synthetic negatives' base, −0.15 to 2.6 (`frontier.json`). It was fitted on 63 machine renders of 7 synthetic negatives; only the 1,440 nodes inside its reach whose channels lie within 0.6 of one another are read by every measurement (`analyse-table-frontier.md`).

## The look

- **Greys.** The table's neutral line, the red and blue densities where it prints red, green and blue alike along green's, read between 0.15 D (under it the table prints black) and its reach, held outside. The window's share 0 is its black, density 0; share 1 its white, the green density where its grey prints lightest, 1.62 D. Each channel's share is laid on its own branch of that line, so an equal share on the three prints the table's own grey, exactly, and a grey never prints darker on a thinner share (the table's noise under code 0.005 near its black is held).
- **Colours.** A 3 × 3 crosstalk, the curves unmixed through it so the greys stay the table's, fitted by Levenberg-Marquardt on the 1,440 constrained nodes, Oklab distance × 100 to the table laid between the same ends. Nothing is extrapolated past what the table measures: the curves hold at either end of the window.
- **Light.** The table's light is display sRGB's: the look's `primaries` take it to the working Rec. 2020.

## Figures

On the constrained nodes, against the table (`frontier_paper::the_look_stays_on_the_measured_table_where_the_measurements_constrain_it`): ΔE median 1.99, p90 4.73, max 9.7. With no crosstalk, the greys alone: median 2.72, p90 5.81. No separable look goes much under 1.6 of median (`analyse-table-frontier.md`): what is left is hue, the turns the machine lays by sector, which three curves and a 3 × 3 cannot carry.

On the owner's billiard session (six scans, `planche-3` of 2026-11-28): ΔE median 1.5 to the table, p90 2.4 to 5.1; a saturated blue table leaves the constrained zone on 19 % of its pixels, where the table is itself its former fit's extrapolation.

## What it replaced

The former Frontier paper fitted a matrix and three normal integrals per layer, then laid the window on a "useful range" re-solved in the app, which discarded the table's density scale and neutral; its curves reached 4.7 to 5.3 of density and its black sat at 0.9 of that, crushing the shadows. The legacy print of the Scanner face keeps its own baked cube (`frontier_look.rs`), unchanged, for the photos a released version edited there.
