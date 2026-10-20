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
| Editor: canvas, panel, bar, menus, keys, history | [editor.md](editor.md) | 142 | 9 | 0 | 142 of 151 |
| Archive, gallery, rolls, Prepare the Roll, projects | [archive.md](archive.md) | 79 | 9 | 2 | 86 of 90 |
| Export and colour management | [export-colour.md](export-colour.md) | 37 | 1 | 0 | 38 of 38 |
| Presets, windows, help, updates, life cycle | [other.md](other.md) | 35 | 11 | 2 | 40 of 48 |
| Hidden behaviour: models, measurement, interaction, codec | [hidden.md](hidden.md) | 122 | 3 | 5 | 121 of 130 |
| **All** | | **415** | **33** | **9** | **427 of 457** |

Within the editor: canvas 18 / 1 / 0, main band 2 / 3 / 0, panel tabs 7 / 0 / 0, Roll tab 19 / 0 / 0,
Crop tab 8 / 0 / 0, Color tab 13 / 0 / 0, Details tab 4 / 0 / 0, De-dust 7 / 0 / 0, Scanner face
7 / 0 / 0, bar 11 / 1 / 0, menus 33 / 3 / 0, keys 7 / 0 / 0, history 6 / 1 / 0 (done / partial /
missing).

Fourteen rows are done with no check: the traffic lights' inset, Recheck Originals and Read the Folders Again (three rows), bare
keys yielding to a text field, the exclusive key surfaces, the thumbnails' proof, the ground behind
the frame, Space held for the hand tool, the offline `--check`, the bare medal keys, the Temperature
and Tint track ends, Show Presets Folder and Roll of the N.

### Swift self-checks

[checks.md](checks.md) lists the 162 checks `--check` runs: **91 have a Rust twin, 23 a partial one,
48 none.**

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

1. **A frame not yet converted.** Its thumbnail and column picture are the pipeline's render where
   Swift shows the file's embedded preview (#61).
2. **Windows' own surfaces.** The settings on the system clipboard under the app's types, a signed
   build and its feed; ⌥⌘ shortcuts read as Ctrl+Alt, which European layouts take for AltGr.
3. **Sound and touch.** No key click plays and no haptic answers; the choice to mute is kept.
4. **The watchdog's stack.** Blocks are written and counted; the blocked thread's stack is not read.
5. **Prepare the Roll's leftovers.** The plate's settle animation, a custom flat-field mask on the
   roll's tiles, the tutorial videos.
6. **Pickers.** The Dyes picker and Select Roll Type open nothing; a roll row shows no stock.
7. **The archive tree's hand.** No context menu, no ⌘/⇧ selection, no drop on the project tree.
8. **The design system's interactions.** The pager, slots, the column resizer, nested swipes and
   the hover fallback.
9. **Sidecar batches.** Read and written side by side, but the interface thread still waits for them.
10. **The instruments.** Swift's command-line tools and 48 of its 162 self-checks have no twin.

## Port order

Editor first (canvas, panel, bar, history), then archive and rolls, then export and colour
management, then the rest. The keycap look comes last, through the design system.
