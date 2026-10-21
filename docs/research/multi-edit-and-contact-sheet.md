# Editing several frames together, and the contact sheet's next step

Two ways to answer the same need: look at a roll's frames side by side and act on them at once. **A** edits
the frames themselves, live, on the canvas. **B** keeps the contact sheet as a picture and makes it a place to
mark, circle and export. They can coexist; §4 compares them and recommends an order.

## 1. Where things stand

- **The contact sheet today** (`oni-measure/src/sheet.rs`, `oni-app/src/app/sheets.rs`, Swift's
  `ContactSheet`): New Contact Sheet lays the selection six to a row, each frame decoded at the roll's
  tile and framed on its own settings, and writes **one new 16-bit linear TIFF** beside the first frame. That
  file is adopted as a photograph of its own: editing it edits the picture, never the frames it came from.
  Its sidecar keeps a `ContactSheetGrid` (columns, rows, tile, margin, each frame's window).
- **Pieces already there for both options:**
  - Prepare the Roll already decodes reduced tiles of a whole roll and redraws them as a look changes
    (`roll_previews.rs`, `TILE_SIDE`, the `retile` queue).
  - The store already writes a gesture to several frames as one undo step (roll settings, paste to roll).
  - The canvas already draws vector strokes over the picture (De-dust's `SourceTrace`, `LiveStroke`,
    `CorrectionMark`).
  - The gallery already labels frames (`Medal`) in their sidecars.

## 2. Option A — Edit together

### 2.1 What the hand does
1. Select frames in the gallery or the sidebar (2 to 36; a roll is ≤ 40).
2. **Edit Together** (bar key, menu, ⌘⇧E): the canvas shows them **six to a row**, each frame its own
   render, in gallery order. The panel is the usual one.
3. A slider, a key or a curve moved sets **the same absolute value on every frame shown** (the owner's
   call): Exposure +0.3 puts every frame at +0.3, whatever each had.
   - A slider whose frames disagree shows the **lead frame's value** (the first selected) and a "mixed"
     mark beside its readout; the first notch makes them all equal.
   - Only the field moved is written: a frame's own crop, window, levels, flat-field, dust and labels never
     move unless that very control is used.
4. A click on a tile toggles it in or out of the edit (a dimmed tile keeps its settings); ⌥-click keeps
   only that tile. A double click opens that frame alone in the editor; Esc or the key leaves.
5. One gesture is one undo step over all the frames; the sidecars are written on release, never during a
   drag.

### 2.2 How it renders without lag
- **Sources:** each frame decoded at half size (LibRaw's `half_size`, no demosaic, ~4× fewer pixels) and
  reduced by Lanczos to **300 px on its longest side**, kept as half-float linear RGBA in the caches folder
  (`Caches/Grid/<fingerprint>.f16`, ~0.5 MB a frame) so a roll decodes once; later openings read the cache.
- **Budget for a roll of 36:** sources ~17 MB, drawn tiles ~9 MB, 2.2 Mpx a notch, about one 1800 px
  picture: the editor's 2000 px preview renders in ~9 ms today.
- **Phase 1, per tile:** each tile rendered by the existing `Pipeline` on its own settings, at scale
  300/full width, through the same latest-value slot (one notch in flight, the latest waiting). Measure:
  `ONI_BENCH=grid` drags Exposure over 36 tiles and prints the notch p50/p99 and frame cadence.
- **Phase 2, only if phase 1 misses 16 ms p99:** one atlas texture holding every tile and one pass of the
  per-pixel stages with each tile's arguments read from a per-tile table (the kernels index their
  uniforms by tile); neighbourhood stages stay per tile.
- **Neighbourhood stages** (Texture, Sharpening, Chroma Denoise, Grain) run at the tile's scale as they do
  on any reduced preview; dust patches and custom flat-field masks are laid as on the canvas.
- **RAM:** under 64 MB for 36 frames, GPU included; nothing kept at full resolution.

### 2.3 Checks
- Window check: select 6 frames, Edit Together, drag Exposure, every frame's sidecar holds the value;
  one undo restores each frame's own; a dimmed tile is untouched (twin).
- Store check: a mixed field shows the lead's value; the first notch writes all; fields not moved are
  byte-identical in every sidecar.
- Bench `ONI_BENCH=grid`: notch p99 ≤ 16 ms on roll #8's 38 frames; RAM delta ≤ 64 MB.

### 2.4 What it replaces
The need to make a contact sheet to compare or match frames. The sheet stays for printing a roll as one
picture (Option B) or goes, the owner's call; removing it departs from the Swift app and frees the code
whose decodes crowd the slots (task #211).

## 3. Option B — The contact sheet as a marked-up proof

### 3.1 What the hand does
1. New Contact Sheet as today (the composed picture, opened in the editor).
2. A **Mark** tab on the sheet (beside De-dust): a grease-pencil red by default, three tools:
   - **Pen:** freehand strokes, as on a paper proof.
   - **Circle:** an ellipse dragged around a frame; circling a frame **gives it a favourite label** in its
     own sidecar (the sheet knows each frame's window, `ContactSheetGrid.windows`); removing the circle
     takes the label back.
   - **Cross:** a cross over a frame marks it rejected the same way.
3. Marks are drawn over the sheet on the canvas, never baked into its pixels; undo, erase, clear.
4. **Export as it is:** the sheet's export burns the marks in at the export's resolution (stroke width
   scaled), so the file looks like the marked proof; an export option leaves them out.

### 3.2 Storage and rendering
- Marks live in the sheet's sidecar as vector paths in the sheet's own unit square (`marks: [{tool, points,
  colour, width}]`), a field the Swift app ignores (it reads the sheet, without marks).
- On the canvas: the De-dust overlay's path drawing (`SourceTrace`); at export: rasterised by a small CPU
  stroker (anti-aliased, round caps) over the rendered sheet, before the output profile.
- Labels set by circling go through the gallery's own label command (one undo step for the circle and the
  label together).

### 3.3 Checks
- A circle around tile 3 labels frame 3 favourite; deleting it restores its label (twin: a stroke outside
  every window labels nothing).
- An export with marks differs from one without only along the strokes.

## 4. Comparison and order

| | A — Edit together | B — Marked-up sheet |
|---|---|---|
| Answers | matching and editing frames at once | choosing, showing, printing a roll |
| Touches the frames | yes, live | only their labels (circle, cross) |
| Main work | tile sources + grid canvas + absolute multi-write; atlas only if needed | Mark tab, three tools, sidecar field, export burn-in |
| Risk | the no-lag target (bench decides phase 2) | low; reuses De-dust's drawing |
| Leaves the sheet? | it can, or keep it for B | keeps and extends it |

**Recommendation:** build **A first**, phase 1, with its bench on roll #8 as the gate; it removes the main
reason to make a sheet. Then decide on the sheet: keep it and add **B** if the proof is wanted, or remove it.

## 5. Open questions for the owner
1. A: should the grid also offer **relative** editing later (⌥ while dragging), or absolute only?
2. A: the lead frame — first selected, or the one the pointer last hovered?
3. B: label semantics — circle = favourite and cross = reject, or a free colour per mark?
4. The sheet: keep for B, or remove once A ships?
