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

- **Every GPU kernel has a scalar CPU reference** beside it in `oni-render/src/kernel/<name>.rs`, and a test comparing
  them on settings that reach every branch.
- **A check that cannot see the bug is worse than none**: every numeric test carries a twin, a
  deliberately wrong variant asserted to fail.
- **Measure before theorising**, on real files: roll #8 (Nikon NEF) is the reference roll, the Lumix
  RW2 calibration files the second. `ONI_RAW_DIR` points the tests at a folder of RAWs.

## Decisions that look like mistakes and are not

- **LibRaw replaces Apple's `CIRAWFilter`**, and a per-camera 3×3 measured once against Apple keeps RAW
  colours as in the Swift app (owner's decision, 2026-10-20). Linear TIFF and DNG renders must not move.
- **The balance is the Swift app's 5000 K / tint 0** (Planckian), derived from the camera's own matrix
  and adapted to D65 by Bradford; never the as-shot one or `pre_mul`. LibRaw's highlight mode 1 keeps
  every channel under the 16-bit ceiling. The exposure is 2^BaselineExposure, 0.48 stop when absent.
- **`wgpu` with WGSL** runs on Metal (macOS) and DirectX 12 (Windows). Kernels read with
  `textureLoad` or storage buffers and interpolate themselves: `rgba32float` filtering is optional.
- **Sidecars keep the Swift JSON format and field names**, so a roll opens in either app.

## Build

`scripts/check` runs every check: one build, then every test binary at once (about 30 s);
`--release` is the shipped build and the benches' only. LibRaw is vendored in `crates/oni-decode/libraw` (0.22.2) and
built by `build.rs`, so a new camera arrives by updating that copy, never from the system.

- **The loop stays under a minute, build and checks included.** The dev profile is incremental, the
  workspace at opt-level 1 and every dependency at 3. The Slint interface is its own crate (`oni-ui`),
  so a Rust change never rebuilds it.
- **One feature set for every command**: every crate depends on `oni-hack` (cargo-hakari), so `cargo run`,
  `scripts/check` and a one-crate test never rebuild Slint for another feature set. After any dependency
  change, run `cargo hakari generate && cargo hakari manage-deps`. The phone-link crates stay outside it.
- **An interface edit needs no build in `scripts/live`**: the app reads the `.slint` files at launch and
  reloads them on each save (Slint's live preview), in `target/live`.
- **The app the owner runs is `--profile app`** (release's numbers, incremental, 256 units): an edit
  relinks in seconds. `--release` stays the shipped build. Debug objects are not kept (`split-debuginfo
  = "off"`), so `target/` holds what is linked and nothing older. The window checks are one binary (`tests/window`) that runs each
  check in a process of its own, as Slint takes one platform a process. Never wait on a build: work on
  the next package while one builds.

- **Where the Xcode licence is not accepted**, the `/usr/bin` shims (`cc`, `clang`, `ar`, `git`) refuse to
  run. Point `CC`, `CXX`, `AR` and `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER` at the binaries in
  `$(xcode-select -p)/Toolchains/XcodeDefault.xctoolchain/usr/bin` before any `cargo` command.
- **One target directory per checkout.** Worktrees sharing a `CARGO_TARGET_DIR` get the same artifact hash
  for workspace crates, so cargo links another worktree's build and calls it fresh. Run the final
  verification in one clean checkout.
- **`wgpu`'s major version matches Slint's** (`unstable-wgpu-30`): the canvas texture crosses between them.

---

## Goal and definition of done

The port is a **strict behavioural copy** of the Swift app: the same functioning, including the hidden
rules no panel shows (the hue compensation of the print curve in inverted saturation, the saturation
compensation of the black and white points, dye separation, double-click behaviours, the exact placement
of points). Only two differences are allowed: the LibRaw decode and 16-bit rendering. Port the logic from
the Swift code, never from the interface, and prove it against Swift's outputs.

**Done** means the whole clone, every feature present and working, each proven by a check. Features come
first on the simplified design system; the real interface work comes after.

- **The parity inventory is `docs/parity/`**: every function and hidden rule of the Swift app, with its
  status (done, partial, missing), the Swift code it was read from and the Rust test that proves it. Its
  `README.md` carries the counts; update them with every row that changes.
- **Proof by difference**: the pipeline against Swift on Swift's own decode with adversarial settings;
  every Swift `Checks/` self-check gets a Rust twin; gestures are replayed through the command layer.
- **Green tests are not proof that the app works.** Before saying a feature works, drive the real app
  through the user's flows on a Mac: open the gallery, select, double-click to edit, edit, go back, and
  watch for crashes. A full set of green test targets once shipped an app that crashed on every edit,
  ignored double-click and let the gallery selection drift from the sidebar.
- **Real files**: the automated runs use a test copy of the reference roll. Original RAW and image files
  are never deleted or rewritten; sidecars and the library index may be, since a roll can be re-prepared.

## Performance targets

- **While a parameter slides**, the image follows each render's landing (one in flight, the latest
  waiting, at most one a 120 Hz frame) and the histogram counts at 30 fps (Swift's
  `DS.Motion.recomputedInterval`), so fast-renewing caches never flood RAM or GPU (owner's call).
- **The rest of the interface** (knob, labels, hovers, scrolling) runs as fast as it can, 120 fps at
  least on Apple silicon.
- **Responsiveness beats frame rate**: a constant 60 fps with very low input-to-photon latency is
  preferred over a 120 fps that feels loose. Measure input-to-photon latency and frame-time consistency
  (no spikes, no deep buffering), never the average frame rate alone.
- **Benchmarks lift the frame cap and vsync**, so they measure the real rate, not the screen's.
- How: `docs/research/performance.md`. In short, the settings store stays in Rust with one Slint property
  per displayed value and one write per notch; a latest-value slot keeps one render in flight and one
  pending; full resolution renders only the visible region plus the filter margin, a preview pyramid
  serves gestures. The display path starts 8-bit like Swift's canvas, with the screen-profile conversion
  isolated in one kernel; 16-bit and EDR come later through a presentation ONI owns.

## Interface rules

- **Every screen is built from the design system** (`crates/oni-ui/ui/ds/`, tokens generated from
  `DSTokens.swift`): no literal size, colour or radius outside it, so a change of look is one edit. Simple
  buttons until the design system defines the final components; the exact ONI look comes back last.
- **Drag is reserved for parameters and for the image** (pan, zoom). Menus, lists, sidebars and every
  other surface scroll by wheel or trackpad only, never by grab-dragging (`mouse-drag-pan-enabled: false`);
  the one exception is the scroll knob on the edge, which drags and pages as AppKit's scroller.
- `docs/ui-spec.md` maps every Swift interface item to its Slint construct.

## Working loop

- **Local first.** Build and test on a Mac; GitHub Actions only for Windows, started at the milestones
  that need it, and never something to wait on between steps.
- **Fast by default**: tests on two frames of the reference roll, the whole roll on demand. An
  adversarial review only for code that touches the user's files (settings, sidecars, library).
- **A cloud container's GPU is software** (lavapipe, 20 to 50 times slower): GPU-heavy checks and every
  window run on the Mac.
- **No release or publication** without the owner's word. Commits, pushes and merges of this repository
  need no confirmation.
- **Nothing private goes into this public repository**: no email address, no machine or account detail.
  A `User-Agent` sent to crates.io or any other service is generic.

## Roadmap

Desktop milestones, each closed by a measure or by the owner's judgement in hand:

1. First window on the reference roll (reached).
2. A usable editor, at the performance targets above.
3. Archive and rolls.
4. Parity with the Swift app (Scanner face, De-dust, flat-field, contact sheet, paper proof, full export).
5. Windows.
6. Public beta on both systems.

Mobile milestones (`5e1y/open-negative-initiative-mobile`, Android first): M1 the Android remote against
the demo host, M2 import from the phone (needs milestone 3), M3 the remote on the Rust app (needs the
Scanner face, milestone 4), M4 Bluetooth. Beyond Swift: the **Frontier Look Lab** (the measured SP-3000
table unbaked into parameters, the paper curve edited as a graph, each parameter showing a "neutral" and
a "Frontier" value; research in `docs/research/frontier/`) and **frame detection on a click** (the frame
edge in Crop, the measure window in Prepare the Roll). Later: tablets (iPad, Android), sending images to
the phone, a physical keyboard speaking the same commands, and a way to support the project.

The Swift app is frozen except for bug fixes. The phone link crates (`oni-link`, `oni-localsend`,
`oni-link-ffi`, `oni-link-demo`) live here; the desktop drives them through `oni_link::Host`,
`HostEvents::next()` and `Host::publish(State)`, with relative commands (step, reset, machine, walk, undo,
redo).
