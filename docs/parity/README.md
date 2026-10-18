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
| Editor: canvas, panel, bar, menus, keys, history | [editor.md](editor.md) | 122 | 16 | 13 | 128 of 151 |
| Archive, gallery, rolls, Prepare the Roll, projects | [archive.md](archive.md) | 51 | 22 | 13 | 68 of 86 |
| Export and colour management | [export-colour.md](export-colour.md) | 37 | 1 | 0 | 38 of 38 |
| Presets, windows, help, updates, life cycle | [other.md](other.md) | 24 | 11 | 13 | 26 of 48 |
| Hidden behaviour: models, measurement, interaction, codec | [hidden.md](hidden.md) | 114 | 9 | 7 | 116 of 130 |
| **All** | | **348** | **59** | **46** | **376 of 453** |

Within the editor: canvas 18 / 1 / 0, main band 2 / 2 / 1, panel tabs 5 / 2 / 0, Roll tab 17 / 0 / 2,
Crop tab 8 / 0 / 0, Color tab 13 / 0 / 0, Details tab 2 / 1 / 1, De-dust 7 / 0 / 0, Scanner face
6 / 1 / 0, bar 6 / 3 / 3, menus 28 / 4 / 4, keys 5 / 1 / 1, history 5 / 1 / 1 (done / partial /
missing).

Thirteen rows are done with no check: Recheck Originals and Read the Folders Again (three rows), bare
keys yielding to a text field, the exclusive key surfaces, the thumbnails' proof, the ground behind
the frame, Space held for the hand tool, the offline `--check`, the bare medal keys, the Temperature
and Tint track ends, Show Presets Folder and Roll of the N.

### Swift self-checks

[checks.md](checks.md) lists the 162 checks `--check` runs: **82 have a Rust twin, 19 a partial one,
61 none.**

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

1. **Prepare the Roll.** The measure, the reach, RollDraft, the capture stops and the base pipette
   are in the crates; the surface, its steps, its keys and its menu item are not.
2. **The pipette.** `oni-measure` reads it at Swift's radius; no canvas gesture calls it.
3. **The flat-field mask library.** Use This Photo as Flat-Field, Load…, Rename, Folder, moving
   the frame out of the library, and the sliders remembered per Mac.
4. **The contact sheet.** Composing a roll on its sheet, its window and its detail note.
5. **Opening an archive.** Several roots, the navigator, favourites, the folder menu and what is
   remembered across launches.
6. **RGB passes.** Files grouped by pass order and the registration measured.
7. **The application shell.** Help surfaces, updates, the quit guard, the console, the key sound.
8. **The editors' leftovers.** The Details loupe, help texts, fold memory, the histogram's morph.
9. **Presets.** Everything but the deck's fade at its ends.
10. **The canvas by hand.** Wheel, pinch and ⌘ scroll, tried on the Mac.

## Port order

Editor first (canvas, panel, bar, history), then archive and rolls, then export and colour
management, then the rest. The keycap look comes last, through the design system.
