# Open Negative Initiative in Rust — working notes

The Rust rewrite of Open Negative Initiative (ONI), the film negative converter and editor, native on
macOS and Windows. The Swift app (`5e1y/open-negative-initiative`) is the reference: until this port
reaches parity, every behaviour, constant and serialised field is copied from it, and the Swift code
wins over any note.

## Hard rules

1. **Nothing decides on its own.** No film-base detection, no smart inversion, no preset on import.
   The automatic keys run only on a click and write only what they show.
2. **Floating point end to end.** Images are `rgba32float` between every stage; the render graph
   accepts no other texture format.
3. **Never a home-made demosaicer.** LibRaw demosaics, with every automatic knob pinned
   (`crates/oni-decode/csrc/pinned.c`). Changing one moves every render.
4. **Zero anticipation.** A constant before a slider; a slider once use proves the constant wrong.
5. **Every change is logged in `docs/tasks.json`** before its commit, cited as `(#id)`.
6. **Every comment follows the comment rules**, in every file.

## Comment rules

English; two lines at most per comment block; person-agnostic; say what and why, the why being a goal
or a constraint, never history, dates, tickets or rejected alternatives; delete a comment that restates
the code; a costly trap keeps its constraint and its number in two lines.

## Checks

- **Every GPU kernel has a scalar CPU reference** in `oni-render/src/cpu.rs`, and a test comparing
  them on settings that reach every branch.
- **A check that cannot see the bug is worse than none**: every numeric test carries a twin, a
  deliberately wrong variant asserted to fail.
- **Measure before theorising**, on real files: roll #8 (Nikon NEF) is the reference roll, the Lumix
  RW2 calibration files the second. `ONI_RAW_DIR` points the tests at a folder of RAWs.

## Decisions that look like mistakes and are not

- **LibRaw replaces Apple's `CIRAWFilter`**, so every RAW render moves once against the Swift app.
  Linear TIFF and DNG renders must not move.
- **The daylight balance is the camera's fixed `pre_mul`**, never the as-shot one, with LibRaw's
  highlight mode 1 so no channel is scaled past the 16-bit ceiling.
- **`wgpu` with WGSL** runs on Metal (macOS) and DirectX 12 (Windows). Kernels read with
  `textureLoad` or storage buffers and interpolate themselves: `rgba32float` filtering is optional.
- **Sidecars keep the Swift JSON format and field names**, so a roll opens in either app.

## Build

`cargo test --release` runs every check. LibRaw is vendored in `crates/oni-decode/libraw` (0.22.2) and
built by `build.rs`, so a new camera arrives by updating that copy, never from the system.
