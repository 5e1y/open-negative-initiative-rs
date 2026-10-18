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
| Editor: canvas, panel, bar, menus, keys, history | [editor.md](editor.md) | 102 | 22 | 28 | 108 of 152 |
| Archive, gallery, rolls, Prepare the Roll, projects | [archive.md](archive.md) | 27 | 27 | 31 | 43 of 85 |
| Export and colour management | [export-colour.md](export-colour.md) | 37 | 1 | 0 | 38 of 38 |
| Presets, windows, help, updates, life cycle | [other.md](other.md) | 10 | 15 | 23 | 18 of 48 |
| Hidden behaviour: models, measurement, interaction, codec | [hidden.md](hidden.md) | 98 | 13 | 19 | 101 of 130 |
| **All** | | **274** | **78** | **101** | **308 of 453** |

Within the editor: canvas 18 / 1 / 0, main band 2 / 1 / 2, panel tabs 5 / 0 / 3, Roll tab 7 / 5 / 7,
Crop tab 8 / 0 / 0, Color tab 10 / 3 / 0, Details tab 3 / 0 / 1, De-dust 7 / 0 / 0, Scanner face
6 / 1 / 0, bar 5 / 4 / 3, menus 23 / 5 / 8, keys 3 / 1 / 3, history 5 / 1 / 1 (done / partial /
missing).

Twelve rows are done with no check, all in the app: Recheck Originals and Read the Folders Again
(three rows), bare keys yielding to a text field, ⌘ and ⇧ selection (twice), double-click to open,
the thumbnails' proof, the ground behind the frame, Space held for the hand tool, the offline
`--check` and the bare medal keys.

### Swift self-checks

[checks.md](checks.md) lists the 162 checks `--check` runs: **78 have a Rust twin, 20 a partial one,
64 none.**

### Pipeline branches against Swift

One test compares a Rust render with Swift's (`oni-pipeline/tests/roll8.rs`), on roll #8's own
sidecars and with detail stripped. Of 33 pipeline branches listed in [hidden.md](hidden.md), **10 are
reached by that comparison and 23 are not**; every one of the 23 with a Rust path has a GPU-vs-CPU
check only.

## Values that differ from Swift

None of the values listed against Swift remains apart: the sidecar delay, the thumbnail side and
cache, the block reset's rest, the Dye Separation reset, the slider's double-click interval, the Roll
tab's block order and the menu titles now read as Swift's.

## The ten largest gaps, in port order

1. **Opening an archive.** Open Folder, the index, the remembered folder and the thumbnail cache
   work; several roots at once, the navigator and the ONI Trash view do not.
2. **The canvas.** Zoom, pan, the full-resolution region, Compare and the screen gamut stripes
   work, dust patches and custom flat-field masks reach the render, the crop frame and the brush draw on it.
3. **Wiring what the core already does.** Copy, paste, paste to roll, framing, film type, trash,
   New Roll and the gallery history are wired with Swift's titles and shortcuts; Restore waits on the
   ONI Trash view, Ungroup on a film gesture in the core.
4. **The measurement's surfaces.** `oni-measure` reads Auto White Balance, the blocks, Classic's two
   steps, the pipette and the roll's reference, and the key is wired; no pipette gesture, Each or
   Average dialog, measuring window overlay or Prepare the Roll step calls the rest.
5. **The editors' histograms.** The curves, levels, spectrogram and saturation editors work with
   their point rules; the levels plot has no input histogram yet and the curves draw over the output.
   Only the editors record a step 150 ms after the edit rests; the sliders still record at release.
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
