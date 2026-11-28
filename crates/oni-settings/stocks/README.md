# Film stocks and papers

Everything ONI knows of a film stock or a paper is in one JSON file here, named by its key: who it is, the canister it draws, the stock curve a roll on it is read back through, and the data sheet's profile a digital photo is shot on. Every file here is built into the app; nothing else in the code names a stock, except the neutral film, which a new digital photo is shot on, and the default film (Portra 800), the app's constant since a photo storing no film reads it; a photo storing no paper prints on none.

## Your own stocks and papers, without building the app

ONI reads a second folder, `Stocks` in its support folder:

- macOS: `~/Library/Application Support/Open Negative Initiative/Stocks`
- Windows: `%APPDATA%\Open Negative Initiative\Stocks`

Drop a `.json` file there to add a stock or a paper, and its canister's `.svg` beside it. A file whose `key` is a shipped file's stands in for it: copy a file from here, edit it there. The folder is watched while the app runs: adding, editing or removing a file updates the menus at once, and the open photo renders again on what changed. Remove your file and the shipped one is back.

A file that does not read is refused with a notice saying why, and the version of it ONI last took stays in use. A file is refused when JSON does not parse, a field is missing or unknown (a misspelt one), its `format` is later than the app's, a key holds more than letters, digits and hyphens, a key or a retired key is another file's, it names dyes no file defines, its profile does not read or bake, or it stands for a shipped file yet drops a key that file defined (other stocks name it).

A photo keeps the key it was edited on even while no file defines it: it renders on the default stock, or on no paper, meanwhile, its key shown marked missing, and finds its stock again when the file comes back.

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
- `film`: the virtual film a digital photo is shot on, where the data sheet is profiled. Its `key` is what a digital photo stores; `dyes` naming its own key makes its profile the scanned stock of that key; `balance` is `daylight` (D55) or `tungsten`; `reference` is derived from the profile when left out, on the scan ONI's scanner makes of a colour negative's dyes and D-min; `couplers`, when given, is the film's own DIR coupler matrix (row the donor layer's density, column the receiving layer's log exposure; a negative term strengthens the receiving layer), the common one otherwise: it moves colours and leaves the grey on the film's curves, and a matrix that folds a layer's curve back (inhibiting it faster than its exposure rises) is refused, and a colour film is offered to digital photos only once it holds its own, a photo already on another keeping it; `curves`, when given, is each layer's density on the profile's `log_exposure`, fitted with `couplers` on a reference, which the film develops on in place of the sheet's printed curves, and `fit` says where both come from; `profile` is what `scripts/datasheet-profiles.py` writes, its `provenance` naming the sheet, its URL, its SHA-256 and what is measured and what inferred.
- `developments`: the stock developed otherwise, a push: each with its own `key`, `name`, `iso`, `dyes` and `film`, on this file's brand, type and canister.
- `dye_set`: a set of no stock's own, `{"key": …, "mean_of": [film keys]}`, their curves averaged (the generic C-41 stock, in `generic-color-negative.json`).

A colour negative is read as Status M densities, channel by channel, each channel its own layer: its layers develop on its `curves`, or on the sheet's printed ones (`sheet_density_curves`) where its file fits none, above their least density, inhibiting one another through `couplers`, and its bare film is the sheet's D-min read through Status M (ISO 5-3). No dye spectrum, lamp or sensor reads it: a neutral lands on its curves exactly, and its `reference` holds two points a channel, its black on the bare film and its white where a neutral at the sensor's saturation lands, derived from the film when left out, never measured on a picture. This reading has nothing to do with the paper a photo may be printed on.

The shipped colour negatives' couplers and curves are fitted, the sheet staying the source of their sensitivities and D-min. The films fitted on a reference the owner chose hold the result as data, the references kept out of this repository; the others take the mean couplers of their family's (photo or cine, by `type`) and their sheet's curves made neutral along the grey and brought to those films' contrast, which `scripts/stocks-from-family.py` redoes from `scripts/stocks-from-family.json` and the fitted files. `scripts/derive-couplers.py` derives a matrix from a sheet alone, a start for a new colour negative with no reference.

A film with `neutral: true` is a film of no character, as the shipped `generic-neutral.json`, which a new digital photo is shot on: each layer's density above a clear bare film is its tone curve at its own channel's light, laid so the sensor's saturation reaches `range`, the density from the bare film to that saturation (over 0, up to 3). Its `profile`'s `data` holds `range` and the curve, `density` above the bare film on `log_exposure` about a grey, nought at its foot and rising past the saturation: the mean of six colour negatives' sheet curves, their three layers each, which `scripts/datasheet-profiles.py` writes (Portra 160, 400 and 800, Ektar 100, Gold 200, UltraMax 400), so its window reads with a colour negative's contrast and no colour of its own. It reads no spectrum, has no coupler and its dyes are pure; `balance: scene` leaves the scene the light its decode balanced, where `daylight` or `tungsten` would light it first. The range moves no render at rest, the window being laid on it: it sets how far a gain or a filter, laid in density, moves the picture.

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

A paper is a look at the chain's end, every number of it baked in its file's `look` by `scripts/paper-looks.py`, the app solving nothing: `axis`, the window's shares its 256 nodes span (−0.5 to 1.5); `curves`, each layer's dye amount over the D-min at each node; `crosstalk`, what each channel reads of each dye (Status A); `floor`, the D-min; `primaries`, each density's light in the working space; `latitude`, the log exposure the window spans, Brightness's unit; and, on a data sheet's paper, `filtration`, the enlarger's CC the paper's block shows. The window's share 0, the bare film, prints black and its share 1, the window's white, prints white.

A data sheet's paper keeps its `profile`, the bake's source: `data` holds `log_exposure`, 256 evenly spaced log H, `density_curves`, each node's Status A red, green and blue densities as its sheet prints them, D-min included, and `crosstalk`. The bake lays the window on the paper's useful range, the bare film on 0.9 Dmax of its neutral and the window's white on D-min + 0.04, each layer offset by the filtration that prints a grey neutral along it. The Frontier SP-3000 holds no profile: its look is baked from its measured table (`crates/oni-pipeline/resources/scanner/frontier.cube`), its greys the table's neutral line from its black to its white, its crosstalk fitted on the nodes its measurements constrain, its provenance in the look. The paper menu lists the papers by brand and name.

## Writing the files

`scripts/datasheet-profiles.py <emulsion-dataset folder>` writes each profile into the file holding the profile of its name, every other field kept, and derives again the reference of a film whose profile moved; a profile no file holds gets a file of its own, its name, brand, speed and canister to check. A paper whose profile moved is baked again by `scripts/paper-looks.py`. `SOURCES.txt` says where the profiles come from and under which terms.
