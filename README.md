# Open Negative Initiative: the working repository

This is where [Open Negative Initiative](https://open-negative-initiative.com) is written: a film
negative converter and editor, native on macOS and Windows, in Rust. Every release since 1.4.0 is built
from this code. What you see here is its day-to-day state, work in progress included.

**This repository is for reading, building and changing the code.** Everything else has its own place:

| You want | Go to |
|---|---|
| The app | [Download page](https://open-negative-initiative.com/download), or the [releases repository](https://github.com/5e1y/open-negative-initiative-releases/releases/latest) |
| To learn to use it | [Quick guide](https://open-negative-initiative.com/guide) and [manual](https://open-negative-initiative.com/manual) |
| To report a bug or share an idea | The app's FEEDBACKS key, or [the issues of the releases repository](https://github.com/5e1y/open-negative-initiative-releases/issues) |
| What each version changed | [Changelog](https://open-negative-initiative.com/changelog) |
| How each stage works and why, for photographers | [Feature pages](https://open-negative-initiative.com/features) |
| The exact source of a released version | `OpenNegative-<version>-source.zip`, attached to [that release](https://github.com/5e1y/open-negative-initiative-releases/releases) |

## The stack

| | |
|---|---|
| Language | Rust, edition 2024, on the stable toolchain; a Cargo workspace of 23 crates |
| Render | `wgpu` 30 and WGSL kernels, on Metal (macOS) and DirectX 12 (Windows) |
| Pixels | `rgba32float` between every stage; a decode and its reductions in `rgba16float` |
| RAW decoding | LibRaw 0.22.2, vendored in `crates/oni-libraw/libraw` and built by `build.rs`, every automatic knob pinned in `csrc/pinned.c` |
| Colour | Little CMS (`lcms2`) for ICC profiles |
| Interface | Slint 1.18.1, with femtovg, Slint's femtovg renderer and wgpu-hal vendored in `vendor/` |
| Files | One JSON sidecar beside each photo, the source of truth; a disposable SQLite index |
| Languages | English in the source; French and Spanish in gettext catalogues |

## The workspace

```
scan ─▶ oni-decode ─▶ oni-pipeline ─▶ oni-graph + oni-render ─▶ canvas (oni-app, oni-ui)
        LibRaw,       a frame's        WGSL stages on wgpu,      or file (oni-export)
        scene-linear  settings         each with a CPU twin
                         ▲
        oni-settings (sidecars) · oni-core (store, undo, roll gestures) · oni-measure (the roll)
```

| Crate | What it holds |
|---|---|
| **The render** | |
| `oni-decode` | RAW decoding through LibRaw, pinned; routes a file onto its branch from its bytes |
| `oni-render` | Every GPU kernel in WGSL, beside the scalar reference that judges it |
| `oni-graph` | The render graph: chains of WGSL stages over `rgba32float`, evaluated by region |
| `oni-pipeline` | A frame's settings to its working-space image, on the render graph |
| `oni-measure` | The measurement behind the automatic keys: the roll's reference, the pipette, Auto White Balance |
| `oni-colour` | Working and export spaces, ICC conversion, proof, paper correction |
| `oni-export` | TIFF and JPEG writers, metadata, resize, size estimate, presets |
| **The model and the files** | |
| `oni-settings` | Every serialised settings type and the sidecar codec with its migrations |
| `oni-core` | The headless editing model: the open frame's store, commands, undo, paste, presets, roll-wide gestures |
| `oni-library` | The archive read off the disk: folders, rolls, projects, ONI Trash, SQLite index |
| **The app** | |
| `oni-app` | The desktop shell and the `oni` binary: window, bar, panel, canvas, gallery, updates, reports |
| `oni-ui` | The Slint interface and its design system (`ui/ds/`), compiled apart so a Rust change never rebuilds it |
| `oni-keycap` | The keycap, rasterised on the CPU: lit walls, dished face, socket and shadows |
| `oni-i18n` | The gettext catalogues the interface bundles, read by the Rust side too |
| `oni-demos` | The help cards' demos, coded scenes filmed headless to GIF |
| `oni-lab` | The 2.5D lab, where the design system is seen and tuned live |
| **The phone link** | |
| `oni-keys` | The Scanner face's keys, one vocabulary for the editor and the phone |
| `oni-link` | ONI Link, the remote-control protocol over any byte stream |
| `oni-link-ffi` | The phone side of ONI Link for Swift and Kotlin, through UniFFI |
| `oni-link-demo` | A stand-in desktop and phone to try ONI Link from a terminal |
| `oni-localsend` | LocalSend protocol v2: receiver, sender and discovery |
| **The tooling** | |
| `oni-fixtures` | Where the checks find their reference files, and when they skip |
| `oni-hack` | The `cargo-hakari` workspace-hack: one feature set for every command |

The phone app lives in [open-negative-initiative-mobile](https://github.com/5e1y/open-negative-initiative-mobile).

## Build and run

You need:

- **Rust stable**, through [rustup](https://rustup.rs).
- **A C and C++ compiler**, as LibRaw and Little CMS are built from source: the Xcode Command Line Tools
  on macOS, the MSVC build tools on Windows.
- **`bash` and `jq`** for `scripts/`, Git Bash on Windows.
- **`cargo-hakari`**, once you change a dependency: `cargo hakari generate && cargo hakari manage-deps`.

```bash
git clone https://github.com/5e1y/open-negative-initiative-rs.git
cd open-negative-initiative-rs
cargo run -p oni-app --bin oni
```

| Command | What you get |
|---|---|
| `cargo run -p oni-app --bin oni` | The dev build: incremental, and it reads the `.slint` files at run time, in English |
| `scripts/live` | The app with its interface reloaded on each save of a `.slint` file |
| `cargo run --profile app -p oni-app --bin oni` | Release's numbers, built incrementally: an edit relinks in seconds, the first build of the interface takes about 12 minutes |
| `scripts/ds-lab` | The 2.5D lab, reloaded on each save |
| `scripts/release-build <version>` | The shipped binary, stamped with its version and build number |

`ONI_BACKGROUND=1` launches the app without taking the focus. `ONI_LANGUAGE` sets its language.

On a Mac where the Xcode licence is not accepted, the `/usr/bin` shims refuse to run: point `CC`, `CXX`,
`AR` and `CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER` at the binaries in
`$(xcode-select -p)/Toolchains/XcodeDefault.xctoolchain/usr/bin`.

## Checks

```bash
scripts/check                # one build, then every test binary at once
scripts/check -p oni-render  # one crate
```

It prints the failures whole, then each check it skipped and why, and exits non-zero on any failure.

The checks that need real files read them from these folders, kept out of the repository, and print a
`SKIPPED` line when one is absent. So do the GPU checks where no adapter is found.

| Variable | Folder |
|---|---|
| `ONI_RAW_DIR` | The reference roll of Nikon NEFs |
| `ONI_LUMIX_DIR` | The Lumix S1R calibration RW2 files |
| `ONI_NIKON_HE_DIR` | Nikon Z 8 samples from raw.pixls.us: one lossless NEF, one in each High Efficiency mode |
| `ONI_REFERENCE` | The Swift app's `--decode` and `--render` dumps |
| `ONI_ADVERSARIAL` | The Swift app's renders of `oni-pipeline/tests/adversarial/branches.json` |

GitHub Actions runs Windows only, on every push (`ci.yml`); the macOS checks run locally.
`windows-package.yml`, started by hand, keeps a tester's build and its installer as artifacts for 14 days.

## How the code is written

- **Nothing decides on its own.** The automatic keys run on a click and write only what they show.
- **Floating point end to end.** The render graph accepts no other texture format.
- **LibRaw demosaics, always.** Changing one of its pinned knobs moves every render.
- **Every GPU kernel has a scalar CPU reference** beside it in `oni-render/src/kernel/<name>.rs`, and a
  test comparing them on settings that reach every branch.
- **Every numeric test carries a twin**, a deliberately wrong variant asserted to fail: a check that
  cannot see the bug is worse than none.
- **Measure before theorising**, on real files.
- **Sidecars keep their JSON format and field names**, and every format change carries its migration:
  they are the user's work.
- **Originals are never rewritten.** Sidecars and the index may be, as a roll can be prepared again.
- **Every screen comes from the design system** (`crates/oni-ui/ui/ds/`): a change of look is one edit.
- **Every text the app shows is translated.** Slint writes `@tr("…")`, Rust `tr("…")`; `scripts/i18n`
  adds new texts to the catalogues, and a check fails while one lacks a translation.
- **Every change is logged in `docs/tasks.json`** before its commit, which cites it as `(#id)`.
- **Comments are in English**, two lines at most, and say what and why: the why is a goal or a constraint.
- **Nothing private enters this repository**: no address, no machine or account detail.

## Where to read more

- [`CLAUDE.md`](CLAUDE.md): the working notes, for people and agents alike. The hard rules, the
  decisions that look like mistakes and are not, the build, the performance targets and the roadmap.
- [`docs/tasks.json`](docs/tasks.json): every change, by the id its commit cites.
- [`docs/parity/`](docs/parity/README.md): the inventory of every function and hidden rule of the Swift
  app this port started from, each with its status and the test that proves it.
- [`docs/ui-spec.md`](docs/ui-spec.md): every Swift interface item and its Slint construct.
- [`docs/research/`](docs/research/README.md): the research behind the decoder, the Frontier look and the
  performance work, in French.

The Swift app stops at 1.3.1 and is the reference these notes cite. Its source is attached to its
releases, up to [v1.3.1](https://github.com/5e1y/open-negative-initiative-releases/releases/tag/v1.3.1).

## Releases and keys

Publishing is the owner's step, and the builds, notes and update feeds live in the
[releases repository](https://github.com/5e1y/open-negative-initiative-releases). The version number is
in `crates/oni-app/Cargo.toml`; the build number the feeds compare is 10000 plus the commit count.

Two `PUBLIC_KEY` constants sit in the source, and both are public halves: the one in
`oni-app/src/updates.rs` verifies an update's EdDSA signature, the one in `oni-app/src/report.rs` encrypts
the log a report attaches. Their private halves are kept outside this repository, and
`scripts/windows-appcast` reads the signing key from the keychain. A build of your own keeps its updates
and reports on the project's feeds and website until you change those constants and addresses.

## Licence

GPL-3.0-or-later, see [`LICENSE`](LICENSE). Under their own licences, each kept beside its copy: LibRaw
(LGPL-2.1 or CDDL-1.0, `crates/oni-libraw/libraw`), femtovg (MIT or Apache-2.0, `vendor/femtovg`), Slint's
femtovg renderer (GPL-3.0-only or Slint's own licences, `vendor/i-slint-renderer-femtovg`), wgpu-hal (MIT or
Apache-2.0, `vendor/wgpu-hal`), notify (CC0-1.0, `vendor/notify`), and the Inter and Enriqueta fonts (OFL,
`crates/oni-ui/fonts`).
