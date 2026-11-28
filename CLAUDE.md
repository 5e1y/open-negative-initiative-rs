# Open Negative Initiative in Rust — working notes

The Rust rewrite of Open Negative Initiative (ONI), the film negative converter and editor, native on
macOS and Windows. The Swift app (`5e1y/open-negative-initiative`) is dead (owner's call, 2026-11-12): no
rule, constant, flow or behaviour is kept because Swift had it, and no check compares with it. The
conversion is ONI's own; flows, UX and UI answer to the benches (`docs/ux-bench.md`, `docs/ui-bench.md`).
Everything may change, the sidecar format included, as long as an update migrates what a person already
has on its own, so no one loses an edit.

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
  through its gains (`migrate::matrices`, sidecar format 20) until its roll is measured again. Linear TIFF
  and DNG renders must not move.
- **The balance is 5000 K / tint 0** (Planckian), derived from the camera's own matrix
  and adapted to D65 by Bradford; never the as-shot one or `pre_mul`. LibRaw's highlight mode 1 keeps
  every channel under the 16-bit ceiling. The exposure is 2^BaselineExposure, 0.48 stop when absent.
- **`wgpu` with WGSL** runs on Metal (macOS) and DirectX 12 (Windows). Kernels read with
  `textureLoad` or storage buffers and interpolate themselves: `rgba32float` filtering is optional.
- **A digital photo is shot on a virtual film** (owner's call, 2026-11-21): no Digital pipeline of its own. Its scene, in the light its decode balanced
  (5000 K on a RAW), reaches layers balanced for the stock's own light (Portra 800: D55, Vision3 500T: tungsten), so a tungsten film turns a daylight scene
  blue as a real one does and the Source tab's Filtration takes the cast away (owner's call, 2026-11-26); the Film block names both lights. It is exposed
  through Otsu 2018's spectra of real objects (`irradiance_xy_otsu2018.npy`; slides and black and white films keep spektrafilm's table) and developed on
  its `curves`, its layers inhibiting one another through `couplers` (a negative term strengthens a layer), both fitted on a reference the owner chose for
  16 films (kept out of the repository, the files holding the result) and the family's mean for the 30 others (`scripts/stocks-from-family.py`), the sheet
  staying the source of the sensitivities and the D-min (owner's call, 2026-11-27), over its bare film's Status M D-min (`film::status`): its negative is
  those densities, each channel its own layer, written less the veil its reading adds, no dye spectrum, lamp, sensor or matrix between (model S, owner's
  call, 2026-11-26). That negative is read by the negative pipeline unchanged, its reference derived on it: each channel's black on the bare film, no
  light, and its white on the sensor's saturation, the decode's 5000 K neutral at the white level, `2^0.48`; a neutral exposes the film's three layers at
  the same log exposure and the scene's chromaticity is read as it is, no layer offset and no gamut compressed (owner's call, 2026-11-26): each channel's
  two points are its black on the bare film and its white where a neutral at the sensor's saturation reads, nothing measured on the photos. A paper is a
  print laid over it, never its reader. The film of no character is the neutral one. spektrafilm's full chain, on its own profiles, stays a benchmark
  outside the app (`examples/film_compare.rs`, `docs/research/colour-diagnosis`).
- **A digital photo's virtual negative is a scan made once** (owner's call, 2026-11-26): its scene shot on its film (shooting
  exposure, halation, bloom, curves, grain) is rendered on the scan's grid before any crop or turn (`Pipeline::scan`) and kept in
  `rgba16float` as a decode is, at the editor's resolution, at full definition only once the view reads past it (100 %, export),
  made in 4 MP bands (`Graph::materialise_half`); the frame is read off it as a scanned negative's (`Pipeline::on_scan`),
  Lighting included. A film's setting under a hand renders on the preview's scan, sharp once the hand rests. The halation is red
  alone (`HALATION_COLOUR`): the red layer, the deepest, takes the light the base sends back.
- **A new digital photo is shot on the neutral film** (owner's calls, 2026-11-24): a film of no character, first in the list
  (`stocks/generic-neutral.json`, `VirtualStock::NEUTRAL`), so a photo opens as its decode holds it, its white on the sensor's
  saturation. Each layer's density above its clear bare film is its `range` times the sRGB code of its own working-space channel's
  light over the saturation (`neutral_negative`): no spectrum, no coupler, pure dyes, the scene's own light (`balance: scene`), no
  light exactly on the bare film; its scan is written less the pedestal its reading adds (0.2 % of light at mid-tone otherwise), so
  the window's share is that code and the pure inversion gives the light back. Its range, 1.0, is a real colour negative's window
  (Portra 800's green spans 0.99), so the gains and filters, laid in density, move it as they move a scan. Portra 800 stays what a
  photo storing no film, or a key no file defines, reads.
- **The grain lies on the developed film** (owner's call, 2026-11-24): a digital photo's alone, uniform in the film's density after its
  curve (`grain`), so the inversion weakens it toward the white and strengthens it in the shadows as on film; each layer has its own
  grains, a colour negative's channel its own layer's, carrying 0.739 of its film's grain Amount (the Amount is the grain a scan shows,
  which a scanner's channels averaged over three layers: `grain::READ_ALONE`), a slide's mixed on the three channels by the projector's
  gaze. Halation has no threshold and a long tail (owner's calls, 2026-11-25 and 27): each layer's whole exposure, spread as exp(−r / λ)
  over the width its film's file gives (`HALATION_TAIL`, three Gaussians on the film's plate), a dose of it laid before the film's curve,
  red alone, so a bright frame takes a light pink veil and its lights a wide glow; the dose is `halation::DOSE_PER_AMOUNT` (0.05 / 0.31) of
  the amount the files and the slider hold; each film's file gives its rests (halation, Bloom from its MTF, grain amount and size), the
  sliders free on top. A slide's lies on its positive; a scanned negative takes none.
- **A digital photo's film effects are notched** (owner's call, 2026-11-27): grain, halation, bloom and the shooting exposure step through
  named levels (`oni_settings::effect_levels`), each film's rests among them, ticked on the rail; a key, an arrow or the value editor moves
  one notch, the knob standing on it in the same turn, no spring; under a drag the knob rests on each notch it crosses, the readout with
  it; a number typed lands on the nearest. At rest the render thread makes the preview scans one notch either side of each of the six
  fields, the one the hand moved last first, while no job waits (`render::ahead`, twelve kept, about 255 MB on a 47 Mpx photo), so the
  notch beside lands in the next frame. Format 82 lays an older photo's values on their nearest levels, the person's own stock files
  read first.
- **Every film stock and paper is its maker's data sheet** (owner's call, 2026-11-22): `scripts/datasheet-profiles.py` writes each profile from the
  official sheet's curves into its stock's file; no spektrafilm profile ships. A slide's bake follows spektrafilm's model (its dyes, couplers); a
  colour negative's is its fitted curves and couplers over its sheet's sensitivities and D-min, the sheet's printed curves kept in its profile; a
  paper's profile is its sheet's Status A curves, D-min included, and its dyes' Status A crosstalk, the source of the look
  `scripts/paper-looks.py` bakes into its file.
- **Every film stock and paper is one file** (owner's call, 2026-11-23): `crates/oni-settings/stocks/<key>.json` holds its identity,
  canister, dyes, virtual film, reference and profile (a paper's profile and its baked look), built into the binary; the person's own
  in the support folder's `Stocks` add stocks or stand in for one by key, watched while the app runs (`stocks/README.md`). The code
  names no stock but the defaults (the neutral film, Portra 800, the generic C-41 set, Ektachrome E100 for slides)
  and, in a migration, the papers that left (`migrate::papers_redone`); a key no file defines is kept as stored and renders on the
  default until its file is back, a paper's on no paper.
- **A digital photo on a colour reversal film is projected** (owner's call, 2026-11-23): its scene exposes the slide on the film's
  sheet (curves falling, its couplers on the first developer's negative image), and the slide's dyes filter a 3200 K projector lamp
  onto a white screen, seen adapted to the bare film's light with a 0.5 % veil (`film::projection`), a band the sheet leaves
  unmeasured holding its neighbours'; it never prints on a paper, and shows as a scanned slide does (`projector`).
  Velvia 50, Velvia 100, Provia 100F and Ektachrome E100 hold their sheets; Provia's green and blue, hidden under red's stroke
  past log H -1.50 and -2.16, take red's points there.
- **A black and white virtual film is the colour negative's kernel on one layer**: its sensitivity on the three channels, no
  coupler, its silver neutral in one scanned band; its photo is then read as a monochrome roll's (`PipelineSettings::is_monochrome`).
- **The Frontier is a paper** (owner's calls, 2026-11-22 and 28): the Fujifilm Frontier SP-3000 prints in the paper list, its look baked
  from its measured table (`frontier.cube`): its greys the table's neutral line from its black to its white, exactly, and a crosstalk
  fitted on the 1,440 nodes its measurements constrain, nothing extrapolated past them (`docs/research/frontier/frontier-paper.md`); a
  photo once edited on the removed Scanner face keeps its render on a legacy print (its keys and D+0.5 M+2 Y-2 rest).
- **A paper is its curves** (owner's calls, 2026-11-23 and 24): a paper is a look at the chain's end (owner's call, 2026-11-28), three
  curves from the window's share to each layer's dye amount, the dyes' crosstalk, the light, then its ends, every number baked in its
  file by `scripts/paper-looks.py` and nothing solved in the app. Each layer develops its dye on its own curve (contrast, shape, the
  layers' drift, D-min and Dmax), each channel reads the dyes through a broad-band crosstalk (owner's call, 2026-11-28): the 3 × 3 that
  best stands for the print through its sheet's dye spectra under a D50 viewing light, read by the 1931 observer, its curves unmixed
  through it so a grey keeps the sheet's Status A densities; each density absorbs its own Rec. 2020 channel alone. One constant of the
  bake, `BROAD_BAND` (1 the broad band as fitted, 0 pure dyes), sets its strength (`scripts/paper-looks.py`, which reads the dye
  spectra in the emulsion dataset; no spectrum ships).
- **The render is the pure inversion, a paper a choice laid over it** (owner's call, 2026-11-24): a negative already holds the log
  of the light, so each channel's density, read between Prepare's black on the bare film and its white on the roll's lights, is
  shown as it is, as a black and white negative's: the window's share the sRGB code of its light in the working space, no curve,
  and no bound but the output clip's, a share past either end carried on along that code (`kernel::inversion`). `print_paper` absent is no paper, the rest of every new photo and every Prepare, a digital
  photo's included; Brightness and Print lift are a paper's alone. A photo of an earlier format keeps a paper only where
  it chose one, from format 73, and that paper still ships (`migrate::papers_redone`, format 84).
- **A paper named lays Prepare's window on its useful range** (owner's calls, 2026-11-22 to 24): no ONI print, no enlarger switch
  and no colour compensation but on a legacy print; the bare film exposes the paper's black end (0.9 Dmax on its
  neutral) and the window's white its white end (D-min + 0.04, ISO 6846),
  whatever the frame: no grey placed, no time solved. Each layer's offset, the paper's own filtration, is baked
  once by least squares so equal shares print neutral along the whole range (`scripts/paper-looks.py`). The picked black never lifts and lies on pure black,
  the window's white on pure white (Print lift keeps the paper's own); Lighting but Exposure moves the whites freely. A legacy print
  laid keeps its own curve and takes no paper; laying it takes the paper away. Brightness moves every share by its stops over the paper's
  `latitude`, the log exposure the window spans, as the enlarger's light did.
- **Lighting acts on the negative in every pipe** (owner's call, 2026-11-22): each slider moves the layers' log exposure, read back
  through the stock's curves, and the film is developed again (`negative`); the inversion shows it, or the enlarger prints it at its rest. A
  digital photo's scan is read through the model's own curves (three coupled layers, a black and white film's one uncoupled); a scan is read on
  its stock's mean layer scaled to its own grey, so its neutral stays neutral. A legacy print reads its machine's entry itself. Its
  moves turn about the share the pure inversion shows as a scene grey, the sRGB code of 0.184; a legacy print about its own curve's grey.
- **Exposure holds Prepare's black and white** (owner's call, 2026-11-24): on the negative, in the chain before any print, it gains every
  tone share's odds `u / (1 − u)` by what the film's own stop lays on the grey's, the same on the three channels: the bare film and
  the window's densest stay, each end's tones scale about it as by a gain, any travel monotone. A projected slide keeps its own.
- **A slide is projected, never printed** (owner's call, 2026-11-23): its scan over the backlight (its gains put a sprocket hole on 1)
  and the bare base is solved, pixel by pixel, for its film's three dyes under a 5500 K scanner lamp (`dye_amounts`), then shown as a
  projector shows it on a white screen; its film is its roll's reversal stock where the sheet is profiled, else Ektachrome E100.
  Lighting re-exposes the slide through its stock's curve: each layer read as the neutral density holding it, back to log
  exposure along the green layer's curve, so a grey stays grey; the screen then lays the lit slide back onto its rest over the
  deep shadows (`held_black`), so no slider lifts the film's black. Projected blacks keeps the screen's own black and white, off at
  rest, pure black and white as on a display. A measure upstream of the print reads the scan through the windows; a legacy print
  keeps its render.
- **The sidecar format is ONI's to change** (`oni-frame`, plain `serde_json` of `oni_settings::document`): every
  older file migrates on its own (`oni_settings::migrate`, the launch's sweep or its first read), never lost; an older build opens it read-only.
- **A dust stroke is kept on the scan, not on the frame** (sidecar format 21): the corrections are laid before the crop,
  turns and angle, so no framing moves them.
- **A dust patch lies on the decoded scan**, before the capture's exposure and the flat-field, and its file names the
  file's reading alone (`Reading::stamp`): a gain slider never drops or re-heals a patch, which takes the gain of where it lies.
- **The editor reads a RAW at LibRaw's half size** (owner's call, 2026-10-22): 3 to 5 times faster to open, no
  difference on screen; the whole decode is a Settings choice, and Details, De-dust and every export read it.
- **A photo is decoded on its selection**: a plain press leaving one photo selected decodes it into the disk cache behind the hand's
  work, as an open photo's neighbours are, two either side (the next first); an opening whose decode another thread runs waits for it
  and reads it back (`cache::Flight`), never LibRaw twice.
- **femtovg is vendored** (`vendor/femtovg`, 0.27.0, `[patch.crates-io]`) to keep its wgpu pipelines across
  flushes: upstream drops them and an interface frame cost 8.7 ms instead of 1.9 ms; a frame is submitted every 16 passes, as
  Metal holds about 2.6 MB of driver memory per pass until its command buffer completes (590 MB on a first frame). Slint's femtovg renderer is
  vendored too (`vendor/i-slint-renderer-femtovg`, 1.18.1) for no vsync wait on present and three drawables
  (`ONI:` comments in `wgpu.rs`): the display link still paces the draws; with two, the interface waited on drawables.
- **wgpu-hal is vendored** (`vendor/wgpu-hal`, 30.0.1, `[patch.crates-io]`) for upstream's fix, in no release yet: a window's first
  frame owes nothing to AppKit's occlusion state. Without it Slint holds a window at alpha 0 until a frame wgpu refuses until that
  state says visible: where it never does, nothing shows and nothing is logged. The copy goes at the release that carries the fix.
- **wgpu-core is vendored** (`vendor/wgpu-core`, 30.0.1, `[patch.crates-io]`) for upstream's fix gfx-rs/wgpu#9958, on trunk for wgpu 31
  and in no wgpu 30 release (`ONI:` comment in `src/device/resource.rs`): a poll's wait that ran out while another thread emptied the queue
  panicked on an assert, killing the render thread when the canvas waited on the GPU in slices as others submitted. The copy goes with wgpu 31.
- **femtovg redraws the whole window at every frame**: the columns and the bar carry `cache-rendering-hint`, or a
  pan redraws every row of the tree. A gesture renders the view lighter, sharp once the hand rests (`SETTLE`).
- **The version's picture stays 3.4 s at every launch a person makes** (owner's call, 2026-11-27): a launch is the picture's showing, so
  the starter gives way once the kernels are compiled and `starter::HELD` has passed, longer only after an update while the kernels
  compile (6 to 8 s on an M2 Max); meanwhile it prepares, off the interface thread, the default film and the photo opened last, decoded,
  laid and scanned (`launch_ahead`), none of it holding the starter. A bench holds no picture and the window checks launch no starter.

## Build

`scripts/check` runs every check: one build (8 min from nothing, seconds when incremental), then every test binary at
once, the longest on the last run first: about 10 minutes on the M2 Max, nearly all of them the window checks, every
other binary under 30 s but roll #8's when `ONI_RAW_DIR` names it (up to 6 minutes). It runs at the end of a work
stream, at a milestone or before a release (owner's call, 2026-11-25); in between, the app is built and the owner tests
it live. `--release` is the shipped build and the
benches' only. LibRaw is vendored in `crates/oni-libraw/libraw` (0.22.2) and built by `build.rs`, so a new camera
arrives by updating that copy, never from the system.

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
  check in about 15 s; each window check compiles the interface again, 3 s. Its `@tr` texts stay English, so a French
  look is judged on the app profile. `--profile app` and `--release` compile the interface (about 12 min).
- **An interface edit needs no build in `scripts/live`**: the app reads the `.slint` files at launch and
  reloads them on each save (Slint's live preview), in `target/live`.
- **The design system is seen in the 2.5D lab** (`scripts/ds-lab`, `crates/oni-ui/lab`), never in the app:
  it reloads on each save and tunes `Depth` live; the app holds no showcase of its own.
- **The app the owner runs is `--profile app`** (release's numbers, incremental, 256 units): an edit
  relinks in seconds. `--release` stays the shipped build. Debug objects are not kept (`split-debuginfo
  = "off"`), so `target/` holds what is linked and nothing older. The window checks are one binary (`tests/window`) that runs each
  check in a process of its own, as Slint takes one platform a process and a check's workers outlive its shell: one a
  core, the longest on the last run first (`target/tmp/window-timings.tsv`), 303 checks in 540 to 580 s. A check mostly
  waits: a `settle` is 900 ms of real time, the springs running on the real clock. Never wait on a build: work on
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

The conversion is **ONI's own**: its result moves only on purpose, the change named in its task, proven by
the checks and judged by the owner's eye on the reference roll. Nothing is kept or refused because the Swift
app did it so; its hidden rules stay only where they earn their place.

**Flows, UX and UI** are judged on the benches and may be rethought whole, a sidebar or a flow included, as
long as the result stays as good. A step that secures a good
conversion, such as the dark pick in Prepare the Roll, is never cut to save time.

**Done** means every feature present and working, each proven by a check.

- **`docs/parity/` is history** of the port: no change needs to update it, and no rule there binds.
- **Proof**: every result rule has its own check on ONI's own outputs; gestures are replayed through the
  command layer.
- **Green tests are not proof that the app works.** Before saying a feature works, drive the real app
  through the user's flows on a Mac: open the gallery, select, double-click to edit, edit, go back, and
  watch for crashes. A full set of green test targets once shipped an app that crashed on every edit,
  ignored double-click and let the gallery selection drift from the sidebar.
- **Real files**: the automated runs use a test copy of the reference roll. Original RAW and image files
  are never deleted or rewritten; sidecars and the library index may be, since a roll can be re-prepared.

## Performance targets

- **While a parameter slides**, the image follows each render's landing (one in flight, the latest
  waiting, at most one a 120 Hz frame) and the histogram counts at 30 fps, so fast-renewing caches never flood RAM or GPU (owner's call).
- **The rest of the interface** (knob, labels, hovers, scrolling) runs as fast as it can, 120 fps at
  least on Apple silicon.
- **Responsiveness beats frame rate**: a constant 60 fps with very low input-to-photon latency is
  preferred over a 120 fps that feels loose. Measure input-to-photon latency and frame-time consistency
  (no spikes, no deep buffering), never the average frame rate alone.
- **Benchmarks lift the frame cap and vsync**, so they measure the real rate, not the screen's.
- How: `docs/research/performance.md`. In short, the settings store stays in Rust with one Slint property
  per displayed value and one write per notch; a latest-value slot keeps one render in flight and one
  pending; full resolution renders only the visible region plus the filter margin, a preview pyramid
  serves gestures. The display path starts 8-bit, with the screen-profile conversion
  isolated in one kernel; 16-bit and EDR come later through a presentation ONI owns.

## Interface rules

- **Every screen is built from the design system** (`crates/oni-ui/ui/ds/`): no literal size, colour or
  radius outside it, so a change of look is one edit. The design system is ONI's own, its tokens kept by
  hand; its look follows the art direction and the UI bench.
- **ONI's faces are ONI Sans and Zalando Sans Expanded Black**: ONI Sans, Zalando Sans's normal width at 400 to 700 with tabular
  figures and the eight signs it lacks drawn from Inter (`scripts/oni-fonts.py`, renamed as the OFL requires of a modified face), sets
  every word read (`Type.family`); Expanded Black sets titles and headings (`Type.display`). A borrowed sign carries its vertical
  metrics, or a sanitiser refuses the whole weight.
- **The canvas is pure black** (owner's call, 2026-11-27): panel #1a1a1a, keycap #252525, tile #0c0c0c; on a near-black ground cast
  shadows vanish, so lit edges carry the relief (`Depth`: ambient 0.18, raise-light 0.06, raise-edge 0.08, well-lip 0.16).
- **WRAPPED 26 is a season's key** (owner's call, 2026-11-27): through December and the January after it, the bar holds, before the
  app's own keys, a design-system keycap with a holographic face (`Button` `foil`, `ds/holo.slint`) that opens the year on film, in
  the gallery as in the editor; it counts in the bar's widths and folds like any key, so the bar never passes the window's edge.
- **The welcome is a stage the GPU draws** (`app/help/welcome`): four gifts of foil marched as signed distances in WGSL, one function
  of the cue (`stage::direction::frame`), so a check steps it on a held clock; its films are takes of the app's own renderer, WebP
  decoded on up to three threads aimed where the film will be, never holding the interface.
- **Every opening of the welcome begins on its overture** (owner's call, 2026-11-27), its menu's included: ONI 1.6 in 1.6's face
  (`ONI 1.6 Mark`, loaded nowhere else) on 1.6's ground #282828, a constant of the scene and never a live token, turns into ONI 2.0 on
  the welcome's black, then glides off the first gift.
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
  what files read stay English. The language is Settings > General's, else the system's
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
4. The full tool set (Scanner face, De-dust, flat-field, contact sheet, paper proof, full export).
5. Windows.
6. Public beta on both systems.

Mobile milestones (`5e1y/open-negative-initiative-mobile`, Android first): M1 the Android remote against
the demo host, M2 import from the phone (needs milestone 3), M3 the remote on the Rust app (needs the
Scanner face, milestone 4), M4 Bluetooth. Then the **Frontier Look Lab** (the measured SP-3000
table unbaked into parameters, the paper curve edited as a graph, each parameter showing a "neutral" and
a "Frontier" value; research in `docs/research/frontier/`) and **frame detection on a click** (the frame
edge in Crop, the measure window in Prepare the Roll). Later: tablets (iPad, Android), sending images to
the phone, a physical keyboard speaking the same commands, and a way to support the project.

The Swift app gets no work. The phone link crates (`oni-link`, `oni-localsend`,
`oni-link-ffi`, `oni-link-demo`) live here; the desktop drives them through `oni_link::Host`,
`HostEvents::next()` and `Host::publish(State)`, with relative commands (step, reset, machine, walk, undo,
redo).
