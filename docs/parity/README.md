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
| Editor: canvas, panel, bar, menus, keys, history | [editor.md](editor.md) | 129 | 16 | 6 | 134 of 151 |
| Archive, gallery, rolls, Prepare the Roll, projects | [archive.md](archive.md) | 67 | 16 | 4 | 80 of 87 |
| Export and colour management | [export-colour.md](export-colour.md) | 37 | 1 | 0 | 38 of 38 |
| Presets, windows, help, updates, life cycle | [other.md](other.md) | 27 | 13 | 8 | 32 of 48 |
| Hidden behaviour: models, measurement, interaction, codec | [hidden.md](hidden.md) | 118 | 6 | 6 | 117 of 130 |
| **All** | | **378** | **52** | **24** | **401 of 454** |

Within the editor: canvas 18 / 1 / 0, main band 2 / 2 / 1, panel tabs 5 / 2 / 0, Roll tab 19 / 0 / 0,
Crop tab 8 / 0 / 0, Color tab 13 / 0 / 0, Details tab 3 / 0 / 1, De-dust 7 / 0 / 0, Scanner face
6 / 1 / 0, bar 7 / 3 / 2, menus 29 / 5 / 2, keys 6 / 1 / 0, history 6 / 1 / 0 (done / partial /
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

1. **The contact sheet.** Composing a roll on its sheet, its window, its sidecar and reference,
   detail off on a sheet, and Prepare the Roll writing one.
2. **RGB passes.** Files grouped by pass order, Shift by One File, and the registration measured.
3. **Updates.** The automatic check, Check for Updates…, the UPDATE key and the signed feed.
4. **The year on film.** The journal written on each gesture and Show My Year on Film.
5. **The pipette and the Details loupe.** The readings exist; no canvas gesture or loupe calls them.
6. **The settings pasteboard.** Copy Settings stays in memory instead of the system pasteboard.
7. **The editors' leftovers.** Help texts, fold memory, the histogram's morph, the 150 ms step.
8. **The shell's leftovers.** The traffic lights' inset, the watchdog, key sounds and haptics.
9. **Stocks.** The stock picker by brand with its icons, a roll row showing its stock.
10. **The design system's interactions.** The pager, slots, the column resizer, hover fallback.

## Port order

Editor first (canvas, panel, bar, history), then archive and rolls, then export and colour
management, then the rest. The keycap look comes last, through the design system.
