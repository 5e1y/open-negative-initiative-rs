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
- **departed**: Swift has it and the owner chose to leave it. Not counted below.

The **Check** column names the Rust test that proves the row, or "no check". The end state is every
row done with a check. A test marked "crate only" proves the core, not the feature in the app.

## Counts

| Surface | File | Done | Partial | Missing | Rows with a check |
|---|---|---|---|---|---|
| Editor: canvas, panel, bar, menus, keys, history | [editor.md](editor.md) | 146 | 7 | 0 | 145 of 153 |
| Archive, gallery, rolls, Prepare the Roll, projects | [archive.md](archive.md) | 95 | 5 | 0 | 98 of 100 |
| Export and colour management | [export-colour.md](export-colour.md) | 37 | 1 | 0 | 38 of 38 |
| Presets, windows, help, updates, life cycle | [other.md](other.md) | 37 | 8 | 2 | 39 of 47 |
| Hidden behaviour: models, measurement, interaction, codec | [hidden.md](hidden.md) | 123 | 3 | 5 | 122 of 131 |
| **All** | | **438** | **23** | **7** | **442 of 469** |

Within the editor: canvas 19 / 1 / 0, main band 2 / 3 / 0, panel tabs 7 / 0 / 0, Roll tab 19 / 0 / 0,
Crop tab 8 / 0 / 0, Color tab 13 / 0 / 0, Details tab 4 / 0 / 0, De-dust 7 / 0 / 0, Scanner face
7 / 0 / 0, bar 11 / 1 / 0, menus 35 / 1 / 0, keys 8 / 0 / 0, history 6 / 1 / 0 (done / partial /
missing).

Fourteen rows are done with no check: the traffic lights' inset, Recheck Originals and Read the Folders Again (three rows), bare
keys yielding to a text field, the exclusive key surfaces, the thumbnails' proof, the ground behind
the frame, Space held for the hand tool, the offline `--check`, the bare medal keys, the Temperature
and Tint track ends, Show Presets Folder and Roll of the N.

### Swift self-checks

[checks.md](checks.md) lists the 162 checks `--check` runs: **106 have a Rust twin, 35 a partial one,
9 none, and 12 are blocked on a behaviour the port lacks.**

### Pipeline branches against Swift

One test compares a Rust render with Swift's (`oni-pipeline/tests/roll8.rs`), on roll #8's own
sidecars and with detail stripped. Of 33 pipeline branches listed in [hidden.md](hidden.md), **10 are
reached by that comparison and 23 are not**; every one of the 23 with a Rust path has a GPU-vs-CPU
check only.

## Values that differ from Swift

None of the values listed against Swift remains apart: the sidecar delay, the thumbnail side and
cache, the block reset's rest, the Dye Separation reset, the slider's double-click interval, the Roll
tab's block order and the menu titles now read as Swift's.

## The eight largest gaps, in port order

1. **Windows' own surfaces.** A signed build and its feed; ⌥⌘ shortcuts read as Ctrl+Alt, which
   European layouts take for AltGr.
2. **Touch.** No haptic answers; the key clicks play, silent until turned on (the owner's default).
3. **The watchdog's stack.** Blocks are written and counted; the blocked thread's stack is not read.
4. **Prepare the Roll's leftovers.** The plate's settle animation.
5. **Lens correction.** LibRaw reads no lens profile: a frame Swift corrected (`lensCorrected`) keeps
   its distortion, and its dust patches can sit up to 24 px off (`docs/research/libraw-knobs.md`).
6. **The design system's interactions.** The pager, slots, the column resizer, nested swipes and
   the hover fallback.
7. **Sidecar batches.** Read and written side by side, but the interface thread still waits for them.
8. **The instruments.** Swift's command-line tools and 10 of its 162 self-checks have no twin, 13 more
   blocked on a behaviour the port lacks.

## Port order

Editor first (canvas, panel, bar, history), then archive and rolls, then export and colour
management, then the rest. The keycap look comes last, through the design system.
