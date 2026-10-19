# Extreme optimisation: the plan

The goal: decode in a blink, zoom, pan and reach full resolution without a hitch, and an interface that
never waits. One feature at a time, measured in the real window before and after, each step a bench
line in `docs/tasks.json`.

## Where the time goes today (24 MP NEF, M2 Max, release)

| Step | Now | Note |
|---|---|---|
| LibRaw full decode (AHD) | 980 ms | one thread; the f32 conversion after it also runs on one |
| LibRaw half-size decode | 345 ms | no demosaic; used by Prepare the Roll and the measuring copies (#96) |
| Open a frame to first render | ~1.15 s | full decode, then 2000 px preview built on the GPU |
| Notch to screen | 4-9 ms p50 | the 30 Hz cap holds the cadence (#89) |
| Frame to histogram | 6-7 ms p50, 20-36 ms p99 | the p99 is the levels/curves inputs' blocking readback (#93) |
| Dye Separation release | ~20 ms | the roll's sidecars read and written on the interface thread |

## 1. Decode

1. **Half-size reads for every copy under half the sensor** (done for the roll and measures, #96). The
   editor's 2000 px preview and the gallery's 320 px thumbnails would gain 650 ms an open and a pass
   2.8x faster, but their pixels move by a hair (channel means within 0.01 %): the owner's call, as
   rule 3 says a pinned knob moves every render.
2. **LibRaw's demosaic on every core.** LibRaw is built without OpenMP; with it, or by demosaicing
   bands in parallel, the full decode should fall from ~1 s to ~0.3 s. Measure first.
3. **The camera-to-linear conversion on every core**: 24 M pixels in one loop today.
4. **A preview cache on disk**: the 2000 px preview kept per fingerprint (f16, ~24 MB), so a frame
   opened before opens in tens of milliseconds. Bounded by size, evicted oldest first.
5. **Prefetch**: once the open frame has rendered and the hand rests, decode the previews of the
   neighbours the walk keys reach next, through the same latest-wins gate (#93), so a walk is instant.
6. **Full resolution in the background**: start it when the frame is open and idle, not at the first
   zoom, so 100 % lands at once; dropped when the frame is left (#97).

## 2. Zoom, pan, full resolution

1. **No allocation per dispatch**: the graph creates two or three uniform buffers and a bind group per
   stage per render. A ring of uniform buffers written with `queue.write_buffer`, bind groups cached
   per texture pair.
2. **The texture pool rounds sizes up** (256 px steps) so a pan reuses its textures instead of
   allocating a new set per pixel of movement; idle textures past the canvas size purged after a view.
3. **No `purge` inside the inputs reading**: it empties the pool every image interval during a slide.
4. **A preview pyramid** (2000, 1000, 500) for gestures, so a pinch renders from the level its scale
   needs, then the region at full resolution once it settles.
5. **One canvas target at the canvas's size**, the region written into it, instead of a new texture
   and a Slint import each time the region's size changes.
6. **Staging uploads in bands with a submit between them**, so a full upload never holds a second copy.

## 3. The histograms

1. **The levels and curves inputs read without blocking**: their readback is mapped asynchronously like
   the main histogram's, and they come from the 1400 px measuring copy the measure already holds. That
   removes the p99 of both the notch and the histogram.
2. **Traces drawn straight into a `SharedPixelBuffer`**, one copy fewer; the saturation plot only when
   its tab shows.

## 4. The interface thread

1. **Nothing cloned per notch that a notch does not change**: today a notch clones the settings about
   four times (menus, lab, dedust, render request) and rebuilds the dust rows on every tab.
2. **Stable models updated in place** (`set_row_data`) for the gallery tiles, the roll cells and the
   column, instead of a new `VecModel` per publish.
3. **Sidecar writes off the interface thread**: a single writer thread, reads of a path with a write
   pending answered from memory; removes the 20 ms at the end of a roll-wide gesture. Touches the
   user's files: adversarial review first.

## 5. Memory

Done: the editor's full frame and copies let go in the gallery, Compare's before with Compare off,
decodes gated one at a time (#93, #97). Next:

1. The thumbnail memory bounded in bytes (~300 MB) with recency refreshed on use.
2. The roll's 360 px sources capped or stored as 8-bit.
3. The contact sheet composed tile by tile instead of holding every decoded tile, then a clone.
4. Export: the frame taken by value (one 390 MB copy fewer), strips streamed to the file.
5. RGB passes composed one pass at a time (1.75 GB peak today on export).
6. Undo companions of roll-wide gestures stored as field diffs, not whole settings per frame.

## 6. Measuring all of it

1. The bench report gains resident memory (start, peak, after leaving the editor) and the GPU pool's
   live bytes.
2. A walk bench: N frames walked at a hand's pace, time to first render and to full resolution each.
3. A zoom bench at 100 % and 400 % with a pan, frame-time centiles and the worst gap.
4. Each bench run before and after every change above; a change that does not move its number is
   reverted.
