# Parity with the Swift app

The goal is a strictly identical clone of the Swift app (`5e1y/open-negative-initiative`): every
feature in the same place, every hidden rule and constant, every serialised field. Only the RAW decode
(LibRaw) and 16-bit rendering may differ. Each row cites the Swift code it was read from, which wins
over any note, and the Rust test that proves it.

## Statuses

- **done**: works end to end in the app as in Swift. For a model constant or rule, the value or rule
  is ported and read where Swift reads it.
- **partial**: the core exists without its interface, the interface exists unwired, or a value
  differs. The Gap column says which.
- **missing**: neither the core nor the interface exists.
- **port-only**: the port has it and Swift does not (the phone link). Not counted below.

The **Check** column names the Rust test that proves the row, or "no check". The end state is every
row done with a check. A test marked "crate only" proves the core, not the feature in the app.

## Counts

| Surface | File | Done | Partial | Missing | Rows with a check |
|---|---|---|---|---|---|
| Editor: canvas, panel, bar, menus, keys, history | [editor.md](editor.md) | 26 | 50 | 75 | 46 of 151 |
| Archive, gallery, rolls, Prepare the Roll, projects | [archive.md](archive.md) | 14 | 32 | 39 | 33 of 85 |
| Export and colour management | [export-colour.md](export-colour.md) | 4 | 31 | 3 | 34 of 38 |
| Presets, windows, help, updates, life cycle | [other.md](other.md) | 7 | 17 | 24 | 15 of 48 |
| Hidden behaviour: models, measurement, interaction, codec | [hidden.md](hidden.md) | 75 | 17 | 33 | 74 of 125 |
| **All** | | **126** | **147** | **174** | **202 of 447** |

Within the editor: canvas 5 / 6 / 8, main band 0 / 1 / 4, panel tabs 1 / 2 / 5, Roll tab 4 / 7 / 8, Crop tab 0 / 5 / 3, Color tab 4 / 4 / 5, Details tab 3 / 0 / 1, De-dust 0 / 1 / 6, Scanner face 0 / 3 / 4, bar 3 / 6 / 3, menus 3 / 12 / 21, keys 1 / 0 / 6, history 2 / 3 / 1 (done / partial / missing).

Nine rows are done with no check: ⌘G, bare keys yielding to a text field, ⌘ and ⇧
selection (twice), double-click to open, the offline `--check`, the export sample sizes, the
histogram overshoot and the bare medal keys.

### Swift self-checks

[checks.md](checks.md) lists the 162 checks `--check` runs: **63 have a Rust twin, 18 a partial one,
81 none.**

### Pipeline branches against Swift

One test compares a Rust render with Swift's (`oni-pipeline/tests/roll8.rs`), on roll #8's own
sidecars and with detail stripped. Of 33 pipeline branches listed in [hidden.md](hidden.md), **10 are
reached by that comparison and 23 are not**; every one of the 23 with a Rust path has a GPU-vs-CPU
check only.

## Values that differ from Swift

| What | Swift | Rust |
|---|---|---|
| Sidecar write delay | 400 ms, `Pipeline/Sidecar.swift:7` | 500 ms, `oni-app/src/app.rs:34` |
| Thumbnail side | 320 px, disk cache, `Pipeline/Thumbnails.swift:15` | 480 px, memory, `oni-app/src/app.rs:29` |
| Block reset target | the mode's rest, `Pipeline/Parameters.swift:246` | Negative's rest, `oni-app/src/store.rs:92` |
| Dye Separation reset | dose, family and by-hand, `UI/SettingsPane.swift:74` | dose only |
| Straighten | re-fits the crop, `Pipeline/Geometry.swift:234` | raw angle, `oni-core/src/knob.rs:242` |
| Double-click interval | the system's | 500 ms, `oni-app/src/track.rs:12` |
| Roll tab block order | Measured Roll … Per-Channel Levels, Dye Separation, Balance Strength | Dye Separation and Balance Strength before Density Ceiling |
| Menu titles | "Compare Before/After", "Hide the Library", "Remove Dust & Scratches", "Mirror Horizontally", "Show What This Screen Cannot Show", "Filter: All", "New Roll from Selection", "New Contact Sheet", "Paste to Roll", "Export…" | "Compare", "Hide Library", "De-dust", "Mirror", "Screen Gamut", "Cycle Filter", "New Roll", "Make a Contact Sheet", "Paste to the Roll", "Export" |

## The ten largest gaps, in port order

1. **Opening an archive.** The app opens only `argv[1]`: no Open Folder, no roots, no index, no
   remembered folder, no thumbnail cache.
2. **The canvas.** Zoom, pan, the full-resolution region and Compare work; no screen gamut
   stripes; dust patches and custom flat-field masks never reach the render.
3. **Wiring what the core already does.** Copy, paste, paste to roll, framing, film type, trash,
   restore, reset to default and the gallery history are tested in `oni-core` but answer "not in this
   build"; most menu shortcuts are unmapped.
4. **The measurement behind the automatic keys.** `balance::Measure` has no implementation: no Auto
   White Balance, auto levels, pipette, measuring blocks, Classic placement or roll reference.
5. **The curve, levels, spectrogram and saturation editors.** The kernels exist; no editor, and none
   of their point rules.
6. **Geometry.** No crop overlay, ratio, rotate, mirror, straighten re-fit or measuring window.
7. **Prepare the Roll.** The whole surface and its steps are absent.
8. **Export.** `oni-export` and `oni-colour` are not dependencies of the app; no export card, no
   profile picker, no paper sheet.
9. **Presets.** Core files only: no deck, editor, share code, import or menu list.
10. **The Scanner face and the application shell.** No Lab keypad or face switch; no help surfaces,
    updates, quit guard, console or key sound.

## Port order

Editor first (canvas, panel, bar, history), then archive and rolls, then export and colour
management, then the rest. The keycap look comes last, through the design system.
