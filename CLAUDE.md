# Open Negative Initiative in Rust — working notes

The Rust rewrite of Open Negative Initiative (ONI), the film negative converter and editor, native on
macOS and Windows. The Swift app (`5e1y/open-negative-initiative`) was the reference while the port was
rebuilt. It is now deprecated and far behind in flows, UX and UI, which answer to the UX and UI benches
(`docs/ux-bench.md`, `docs/ui-bench.md`) instead. What stays from it is the result: the conversion's
maths and constants, and the sidecar format it reads.

## Hard rules

1. **Nothing is hidden from the person.** The app may act on its own, but every automatic step and every
   value of the conversion stays visible and reachable: the levels and gains stay shown and editable even
   while the automatic mode sets them. An automatic action writes only what it shows.
2. **Floating point end to end.** Images are `rgba32float` between every stage; a decode and its
   reductions (preview, halvings) are held in `rgba16float` (owner's call, half the memory); the render
   graph accepts no other texture format.
3. **Never a home-made demosaicer.** LibRaw demosaics, with every automatic knob pinned
   (`crates/oni-libraw/csrc/pinned.c`). Changing one moves every render.
4. **Zero anticipation.** A constant before a slider; a slider once use proves the constant wrong.
5. **Every change is logged in `docs/tasks.json`** before its commit, cited as `(#id)`.
6. **Every comment follows the comment rules**, in every file.
7. **The log says nothing of the person using the app.** No file, folder, disk, account or phone name
   reaches the log file, stderr or a sealed report: a photo is `anonymity::token`, a name inside a notice
   is `oni_i18n::named` (the screen shows it, the log masks it), and every line takes `anonymity::scrubbed`
   on its way out. A template naming something bare fails its check until it marks the name or is listed as
   never logged. A shipped binary names no folder or account of the machine that built it (`scripts/release-build`).
8. **Never a patch.** A problem is rectified at its source, never at its symptom: find what produces the
   wrong state and change that, even far from where it shows. Simplify and improve rather than add: no timeout,
   retry, clamp, flag or special case laid over a cause. A cause outside the app (a driver, a system quirk) is
   named where the code answers it.

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

- **LibRaw replaces Apple's `CIRAWFilter`**, in its own colours on every camera and both systems: no
  per-camera matrix (owner's call, 2026-11-02). A frame edited under the former Mac matrices keeps its look
  through its gains (`retired_matrices`, sidecar format 20) until its roll is measured again. Linear TIFF
  and DNG renders must not move.
- **The balance is the Swift app's 5000 K / tint 0** (Planckian), derived from the camera's own matrix
  and adapted to D65 by Bradford; never the as-shot one or `pre_mul`. LibRaw's highlight mode 1 keeps
  every channel under the 16-bit ceiling. The exposure is 2^BaselineExposure, 0.48 stop when absent.
- **`wgpu` with WGSL** runs on Metal (macOS) and DirectX 12 (Windows). Kernels read with
  `textureLoad` or storage buffers and interpolate themselves: `rgba32float` filtering is optional.
- **The Frontier is the ONI machine's render with its own baked look** (owner's call), not Swift's
  density table: ONI's window and print, then the Look Lab's chain on the ONI look's tone, resting on
  D+0.5 M+2 Y-2 in the gains while the keypad reads zero.
- **Sidecars keep the Swift JSON format and field names**, so a roll opens in either app.
- **A dust stroke is kept on the scan, not on the frame** (sidecar format 21): the corrections are laid before the crop,
  turns and angle, so no framing moves them, and a patch's file names the exposure and flat-field it was cut under.
- **The editor reads a RAW at LibRaw's half size** (owner's call, 2026-10-22): 3 to 5 times faster to open, no
  difference on screen; the whole decode is a Settings choice, and Details, De-dust and every export read it.
- **femtovg is vendored** (`vendor/femtovg`, 0.27.0, `[patch.crates-io]`) to keep its wgpu pipelines across
  flushes: upstream drops them and an interface frame cost 8.7 ms instead of 1.9 ms; a frame is submitted every 16 passes, as
  Metal holds about 2.6 MB of driver memory per pass until its command buffer completes (590 MB on a first frame). Slint's femtovg renderer is
  vendored too (`vendor/i-slint-renderer-femtovg`, 1.18.1) for no vsync wait on present and three drawables
  (`ONI:` comments in `wgpu.rs`): the display link still paces the draws; with two, the interface waited on drawables.
- **wgpu-hal is vendored** (`vendor/wgpu-hal`, 30.0.1, `[patch.crates-io]`) for upstream's fix, in no release yet: a window's first
  frame owes nothing to AppKit's occlusion state. Without it Slint holds a window at alpha 0 until a frame wgpu refuses until that
  state says visible: where it never does, nothing shows and nothing is logged. The copy goes at the release that carries the fix.
- **femtovg redraws the whole window at every frame**: the columns and the bar carry `cache-rendering-hint`, or a
  pan redraws every row of the tree. A gesture renders the view lighter, sharp once the hand rests (`SETTLE`).

## Build

`scripts/check` runs every check: one build, then every test binary at once (about 30 s);
`--release` is the shipped build and the benches' only. LibRaw is vendored in `crates/oni-libraw/libraw` (0.22.2) and
built by `build.rs`, so a new camera arrives by updating that copy, never from the system.

- **`scripts/lint`** runs clippy on every target with warnings denied (`[workspace.lints.clippy]`); it is outside `scripts/check`. A lint is
  allowed once at workspace level with its reason, never per site, and no fix may rewrite arithmetic in the render, pipeline, colour or measure crates.
- **The loop stays under a minute, build and checks included.** The dev profile is incremental, the
  workspace at opt-level 1 and every dependency at 3. The Slint interface is its own crate (`oni-ui`),
  so a Rust change never rebuilds it.
- **One feature set for every command**: every crate depends on `oni-hack` (cargo-hakari), so `cargo run`,
  `scripts/check` and a one-crate test never rebuild Slint for another feature set. After any dependency
  change, run `cargo hakari generate && cargo hakari manage-deps`. The phone-link crates stay outside it.
- **Dev and test builds read the interface at run time** (Slint's live preview, set by `oni-ui/build.rs` for
  the `debug` profile): `oni-ui` is a thin layer compiled in seconds, a `.slint` edit reaches a window
  check in about 15 s, the full check runs in about 2 min. Its `@tr` texts stay English, so a French
  look is judged on the app profile. `--profile app` and `--release` compile the interface (about 12 min).
- **An interface edit needs no build in `scripts/live`**: the app reads the `.slint` files at launch and
  reloads them on each save (Slint's live preview), in `target/live`.
- **The design system is seen in the 2.5D lab** (`scripts/ds-lab`, `crates/oni-ui/lab`), never in the app:
  it reloads on each save and tunes `Depth` live; the app holds no showcase of its own.
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

The conversion is a **strict behavioural copy** of the Swift app's result: the same pipeline, including
the hidden rules no panel shows (the hue compensation of the print curve in inverted saturation, the
saturation compensation of the black and white points, dye separation, the exact placement of points).
Only two differences are allowed there: the LibRaw decode and 16-bit rendering. Port that logic from the
Swift code, never from its interface, and prove it against Swift's outputs.

**Flows, UX and UI owe Swift nothing** (owner's call): they are judged on the benches and may be rethought
whole, a sidebar or a flow included, as long as the result stays as good. A step that secures a good
conversion, such as the dark pick in Prepare the Roll, is never cut to save time.

**Done** means every feature present and working, each proven by a check.

- **The parity inventory is `docs/parity/`**: every rule of the Swift app's result (conversion, hidden
  rules, data), with its status, the Swift code it was read from and the Rust test that proves it. Its
  `README.md` carries the counts; update them when a result row changes. Its interface rows are history.
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

- **Every screen is built from the design system** (`crates/oni-ui/ui/ds/`): no literal size, colour or
  radius outside it, so a change of look is one edit. The design system is ONI's own; its tokens began as
  Swift's `DSTokens.swift` and owe it nothing now. Its look follows the art direction and the UI bench.
- **Any screen, flow or interaction may be rethought whole** when the benches show a better way. Today drag
  adjusts parameters and the image, and lists scroll by wheel or trackpad (`mouse-drag-pan-enabled:
  false`). That is the current state, not a rule. Only the conversion's result is untouchable.
- `docs/ui-spec.md` and the interface rows of `docs/parity/` are history from the rebuild: no interface
  change needs to update them.
- **Every text the app shows is translated**: English in the source, French and Spanish in
  `crates/oni-i18n/lang/<lang>/LC_MESSAGES/oni-ui.po`, which Slint bundles and the Rust side reads. Slint
  writes `@tr("…")` (arguments as `{}`), Rust `tr("…")`, `tr!("… {name}", name = …)`, `trn(one, many, n)` and
  `marked("…")` for a constant. `scripts/i18n` adds new texts to the catalogues; the catalogue check fails
  while one lacks a translation, loses a placeholder, or a `.slint` word sits outside `@tr`. Logs, keys and
  what files or the Swift app read stay English. The language is Settings > General's, else the system's
  (`ONI_LANGUAGE` for the checks, which read English).

## Working loop

- **The app is always launched in the background, without the focus** (owner's rule), so the owner keeps
  working: a bench does it by itself, any other launch sets `ONI_BACKGROUND=1`, the owner's app included.
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
