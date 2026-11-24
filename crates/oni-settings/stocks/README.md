# Film stocks and papers

Everything ONI knows of a film stock or a paper is in one JSON file here, named by its key: who it is, the canister it draws, the stock curve a roll on it is read back through, and the data sheet's profile a digital photo is shot on. Every file here is built into the app; nothing else in the code names a stock, except the defaults (Portra 800 for a digital photo, ONI Standard for a paper), which are the app's constants since a photo storing no stock reads them.

## Your own stocks and papers, without building the app

ONI reads a second folder, `Stocks` in its support folder:

- macOS: `~/Library/Application Support/Open Negative Initiative/Stocks`
- Windows: `%APPDATA%\Open Negative Initiative\Stocks`

Drop a `.json` file there to add a stock or a paper, and its canister's `.svg` beside it. A file whose `key` is a shipped file's stands in for it: copy a file from here, edit it there. The folder is watched while the app runs: adding, editing or removing a file updates the menus at once, and the open photo renders again on what changed. Remove your file and the shipped one is back.

A file that does not read is refused with a notice saying why, and the version of it ONI last took stays in use. A file is refused when JSON does not parse, a field is missing or unknown (a misspelt one), its `format` is later than the app's, a key holds more than letters, digits and hyphens, a key or a retired key is another file's, it names dyes no file defines, its profile does not read or bake, or it stands for a shipped file yet drops a key that file defined (other stocks name it).

## A look from a 3D LUT

Drop a `.cube` file in the same folder and it is a film of its own for digital photos, listed under the brand LUT, named by its `TITLE` (else its file name) and stored as `lut-` and its file name in lower case. A photo on it is read through the cube in place of a negative: the film's exposure first, then the scene in display sRGB code, through the cube, back to linear light. Every print being made on a paper, that positive prints through the internegative. Its grain is the default film's. A cube is refused when it gives no `LUT_3D_SIZE` from 2 to 128, is a 1D table, has a domain other than 0 to 1, or holds a node more or less than its size needs. ONI ships no look.

A photo keeps the key it was edited on even while no file defines it: it renders on the default stock or paper meanwhile, its key shown marked missing, and finds its stock again when the file comes back.

## A film stock

```json
{
 "oni": "film",
 "format": 1,
 "key": "kodak-gold200",
 "name": "Kodak Gold 200",
 "brand": "Kodak",
 "type": "colour-negative",
 "iso": 200,
 "canister": "kodak-gold.svg",
 "dyes": "kodak-gold-200",
 "film": {
  "key": "kodak-gold-200",
  "aliases": ["kodak-gold-200-datasheet"],
  "balance": "daylight",
  "dyes": "kodak-gold-200",
  "reference": {"stops": [-0.830452, 0.0, 1.0133026], "black": [0.2786753, 0.27860653, 0.27844664], "white": [0.6040813, 0.6759063, 0.73893756]},
  "profile": {"info": {}, "provenance": {}, "data": {}}
 }
}
```

- `key`: what a roll stores; never change a shipped one. `aliases`: retired keys that read as this one.
- `name`, `brand`: shown as written, trade names being proper nouns. `generic: true` marks a description ("Generic Colour Negative"), shown translated where the app's catalogue holds it.
- `type`: `colour-negative`, `cine-negative`, `colour-slide` or `black-and-white`.
- `iso`: the speed the roll is exposed at, left out for a line or a generic stock.
- `canister`: an SVG beside the file (48 × 48, its root `<svg width="48" height="48"`); left out or missing, the type's generic canister is drawn.
- `offered: false`: the picker no longer lists it, a roll on it still reads. `line: true`: a whole line, its stock unnamed.
- `dyes`: the scanned stock a roll on it is read as, whose curve Lighting reads its negative back through; left out on a slide or in black and white.
- `film`: the virtual film a digital photo is shot on, where the data sheet is profiled. Its `key` is what a digital photo stores; `dyes` naming its own key makes its profile the scanned stock of that key; `balance` is `daylight` (D55) or `tungsten`; `reference` is derived from the profile when left out, on the scan a colour negative's dyes and D-min give under a white lamp; `couplers`, when given, is the film's own DIR coupler matrix (row the donor layer's density, column the receiving layer's log exposure), the common one otherwise: it moves colours and leaves the grey on the sheet's curve, and a matrix that folds a layer's curve back (inhibiting it faster than its exposure rises) is refused, and a colour film is offered to digital photos only once it holds its own (`examples/coupler_fit.rs` fits one on a reference chart of the stock: the scanned negative printed by the enlarger against that chart, grey matched), a photo already on another keeping it; `profile` is what `scripts/datasheet-profiles.py` writes, its `provenance` naming the sheet, its URL, its SHA-256 and what is measured and what inferred.
- `developments`: the stock developed otherwise, a push: each with its own `key`, `name`, `iso`, `dyes` and `film`, on this file's brand, type and canister.
- `dye_set`: a set of no stock's own, `{"key": …, "mean_of": [film keys]}`, their curves averaged (the generic C-41 stock, in `generic-color-negative.json`).

A `colour-slide` stock's film is a reversal film: its profile's curves fall with exposure (Status A, the log H origin where green transmits an 18 % grey), its couplers act on the first developer's negative image, its reference is the windows' rest and its dyes `generic-c41`; a digital photo on it is projected rather than printed. A curve its sheet hides under another's stroke takes that one's points past its own last, listed as inferred (Provia 100F's green and blue under red's).

## A paper

```json
{
 "oni": "paper",
 "format": 1,
 "key": "kodak-royal-digital",
 "aliases": ["kodak-portra-endura", "kodak-ektachrome-radiance-iii"],
 "name": "Kodak Royal Digital",
 "brand": "Kodak",
 "profile": {"info": {}, "provenance": {}, "data": {}}
}
```

A paper holds a data sheet's `profile`, or, for the Frontier SP-3000, `frontier`: the fit of its measured table that `docs/research/frontier/frontier_print_fit.py` writes (`neutral`, `matrix` and each layer's `curves`), with its provenance. A paper is its curves: its profile's `data` holds `log_exposure`, 256 evenly spaced log H, and `density_curves`, each node's Status A red, green and blue densities as its sheet prints them, D-min included; each layer's density then absorbs its own channel alone. Prepare the Roll's window covers the paper's useful range, D-min + 0.04 to 0.9 Dmax on its neutral, unless `data` states it as `useful_range`, the log H of its white end then its black end: ONI Standard's, written by `scripts/oni-standard-paper.py`, is [0, 1], its curve laid on the window's share. The paper menu lists the papers by brand and name.

## Writing the files

`scripts/datasheet-profiles.py <emulsion-dataset folder>` writes each profile into the file holding the profile of its name, every other field kept, and derives again the reference of a film whose profile moved; a profile no file holds gets a file of its own, its name, brand, speed and canister to check. `SOURCES.txt` says where the profiles come from and under which terms.
