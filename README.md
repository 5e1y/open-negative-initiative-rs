# Open Negative Initiative in Rust

The Rust rewrite of [Open Negative Initiative](https://github.com/5e1y/open-negative-initiative-releases),
the film negative converter and editor, native on macOS and Windows. It is a work in progress and does
not run as an app yet; the released app is the Swift one.

- `wgpu` and WGSL kernels, each with a scalar CPU reference, for the render graph.
- LibRaw, vendored in `crates/oni-decode/libraw`, for RAW decoding.
- Slint for the interface, copied from the Swift app.
- The same JSON sidecars as the Swift app, so a roll opens in either.

`cargo test --release --workspace` runs every check. Checks that need RAW files read them from
`ONI_RAW_DIR` and print a line saying they are skipped when it is absent.

GPL-3.0-or-later, see `LICENSE`. LibRaw is under LGPL-2.1 or CDDL-1.0, see `crates/oni-decode/libraw`.
