# Translated texts in dev and test builds

Dev and test builds read the `.slint` files at run time through Slint's live preview (`crates/oni-ui/build.rs`),
so every `@tr` there is evaluated by `slint-interpreter` 1.18.1, never by generated code. Read in its sources:

- **Bundled translations are generated code only.** `with_bundled_translations` makes the compiler embed the
  `.po` catalogues as tables that compiled code reads (`translate_from_bundle`); the live preview's generated
  wrapper (`i-slint-compiler/generator/rust_live_preview.rs`) passes the include paths, the style, the domain
  and the context to the interpreter, never the bundle, and the interpreter's `Compiler` has no setting for one.
  `select_bundled_translation` then has nothing to select.
- **The interpreter's `@tr` asks gettext or an external translator.** Its `BuiltinFunction::Translate` calls
  `i_slint_core::translations::translate`, which tries a translator registered through i-slint-core's `tr`
  feature, then gettext (`gettext-rs`, unix only, `gettext-system`), then the English text.
- **Gettext does not fit the build.** It needs compiled `.mo` catalogues and a libintl: macOS ships none,
  only Homebrew's, and Windows is outside the feature. Slint's `gettext` feature cannot be set for dev builds
  alone, as every command shares one feature set (`oni-hack`), so the shipped app would link Homebrew's
  library; building GNU gettext from source instead runs `configure` and `make`, which the `/usr/bin` shims
  refuse where the Xcode licence is not accepted.
- **The external translator is internal API.** `SlintContext::set_external_translator` lives in `i-slint-core`
  behind a feature no public `slint` feature enables; reaching it means depending on that crate's unstable
  internals, a hack this port does not take.

So a dev build switches every Rust-side text (`tr`, `tr!`, `trn`) and the window's `@tr` texts stay English,
as `CLAUDE.md` says; a French or Spanish look is judged on `--profile app` or `--release`, which compile the
interface with the bundle. Worth reading again when Slint's interpreter gains a bundled-translation setting.
