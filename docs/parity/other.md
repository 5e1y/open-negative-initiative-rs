# Parity: presets, windows, help and the rest

Swift paths are under `Sources/OpenNegative/` of the Swift app; Rust paths are under `crates/`.
Statuses are defined in [README.md](README.md).

## Presets

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Preset file round trip, rename leaves one file | `Pipeline/Propagation.swift` `Preset` | `oni-core/src/presets.rs:207` | partial | Core only | `oni-core/src/presets.rs::a_preset_round_trips_through_its_file_and_a_rename_leaves_one_file` |
| Dialled per row, geometry absent | `Pipeline/Parameters.swift:21-67` | `oni-core/src/parameters.rs:73` | partial | Core only | `oni-core/src/presets.rs::a_preset_carries_its_rows_and_never_the_framing` |
| Old preset reads its groups and retired rows | `Pipeline/Propagation.swift:169` | `oni-core/src/presets.rs:152` | partial | Core only | `oni-core/src/presets.rs::an_old_preset_reads_its_groups_and_its_retired_rows` |
| Default name and file stem (64) | `Pipeline/Propagation.swift:189,366` | `oni-core/src/presets.rs:97-110` | partial | Core only | `oni-core/src/presets.rs::names_and_files_are_formed_as_the_shelf_expects` |
| Apply a preset, auto balance row measured | `Scene/EditorScene+Presets.swift` | `oni-core/src/store.rs:620` | partial | Core only; no `Measure` | no check |
| Preset tints readable | `Pipeline/PresetTint.swift` | `oni-core/src/presets.rs:28` | partial | Eight tints; contrast not checked | `oni-core/tests/swift_checks.rs::an_unknown_tint_is_refused_and_the_default_is_neutral` |
| Preset deck: cards, peek, fade | `UI/PresetCarousel.swift:177-189` | none | missing | | no check |
| Deck menu: Edit…, Update with Current Settings, Duplicate, Delete | `Scene/EditorScene+Panel.swift` | `oni-core/src/presets.rs:70,230` | partial | Duplicate and delete in core; no UI | no check |
| Share Preset…, Copy Preset Code, Paste Preset Code (`ONI1:`) | `Pipeline/Propagation.swift:295` | none | missing | | no check |
| Save as Preset… sheet | `UI/PresetEditor.swift` | none | missing | | no check |
| Preset editor: every parameter under its interface name | `UI/PresetParameters.swift` | none | missing | | no check |
| Import Presets…, adopt incoming | `Scene/EditorScene+Presets.swift` | `oni-core/src/presets.rs:241` | partial | Core only | `oni-core/tests/swift_checks.rs::an_import_never_overwrites_and_a_duplicate_says_so` |
| Show Presets Folder | `UI/EditorCommands.swift:231` | none | missing | | no check |
| Retired shipped presets remembered | `Pipeline/Propagation.swift:242` | none | missing | | no check |

## Settings board

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Settings on the system pasteboard, own types | `Pipeline/Propagation.swift:22,96,127` | `oni-core/src/store.rs:496` | partial | In-memory board; nothing crosses apps or relaunches | no check |
| Paste carries Color and Details whole | `Pipeline/LookSettings.swift:6` | `oni-settings/src/settings.rs:198` | done | Core level | `oni-core/tests/roll8.rs::a_paste_carries_the_color_and_details_tabs_whole_and_undoes_byte_for_byte` |
| Every field in exactly one family | `Pipeline/RollSettings.swift:12`, `LookSettings.swift` | `oni-core/src/fields.rs` | done | | `oni-core/src/fields.rs::every_field_is_the_rolls_the_looks_or_the_frames_and_one_only` |
| Framing board, measuring window board | `Pipeline/Propagation.swift:96,127` | `oni-core/src/store.rs` `copy_framing`, `paste_framing`, `app.rs` `framing_act` | done | Both boards live for the session, as `Propagation`'s; only the open frame is fitted | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |
| Paste check (what would be overwritten) | `Scene/EditorScene+Paste.swift` | `oni-core/src/store.rs` `paste_check`, `app.rs` `confirm` | done | Another roll's look asks on the Another Roll sheet | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |

## Windows and sheets

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Editor window, dark appearance pinned | `OpenNegativeApp.swift`, `Pipeline/QuitGuard.swift:12` | `oni-app/ui/app.slint` | done | | `oni-app/tests/headless.rs::the_window_opens_roll_8_and_a_slider_drives_the_canvas` |
| Console window | `UI/ConsoleView.swift`, `Pipeline/Diagnostics.swift` | none | missing | | no check |
| Design system playground (dev builds) | `DesignSystem/DSPlayground.swift` | `oni-app` `--catalogue` | done | | `oni-app/tests/catalogue.rs::the_catalogue_shows_every_component` |
| Welcome sheet with tutorial video | `UI/WelcomeSheet.swift`, `UI/TutorialVideo.swift` | none | missing | | no check |
| What's New, shown once per edition | `UI/WhatsNew.swift`, `DesignSystem/DSNews.swift` | none | missing | | no check |
| Report a Bug sheet, sealed log | `UI/ReportSheet.swift`, `Pipeline/BugReport.swift`, `ReportSeal.swift` | none | missing | | no check |
| Show My Year on Film (Wrapped) | `UI/WrappedView.swift`, `Pipeline/Wrapped.swift` | `oni-library/src/journal.rs` | partial | Journal only | `oni-library/src/journal.rs::three_gestures_read_back_whole_and_a_torn_or_foreign_file_does_not` |
| Year's journal written on each gesture | `Pipeline/Journal.swift` | `oni-library/src/journal.rs` | partial | Format done; the app writes nothing | `oni-library/src/journal.rs::three_gestures_read_back_whole_and_a_torn_or_foreign_file_does_not` |
| Splash | `UI/Splash.swift` | none | missing | | no check |
| Stock picker sheet | `UI/StockPickerSheet.swift` | none | missing | | no check |
| Paper profiles sheet | `UI/PaperProfilesSheet.swift` | none | missing | See [export-colour.md](export-colour.md) | no check |
| Popovers and dialogs (`dsPopover`, `DSDialog`) | `DesignSystem/DSPopover.swift`, `DSDialog.swift` | `oni-app/ui/ds/` | partial | Pickers open nothing | no check |
| Help bubbles on a layer above the pane | `DesignSystem/DSHelp.swift` | none | missing | | no check |

## Application life

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Quit waits for the sidecar, refuses on a failed write | `Pipeline/QuitGuard.swift:31` | none | missing | | no check |
| A newer sidecar opens read-only | `Pipeline/Sidecar.swift` | `oni-settings` sidecar | done | | `oni-settings/tests/real_files.rs::a_newer_format_opens_read_only_and_is_never_rewritten` |
| Sidecar batches off the main thread | `Pipeline/SidecarBatch.swift` | none | missing | | no check |
| Main-thread watchdog, anonymous console log | `Pipeline/Diagnostics.swift` | none | missing | | no check |
| Cache budget follows the machine | `Pipeline/MemoryBudget.swift` | `oni-app/src/render.rs` (four previews) | partial | Fixed count, not a byte budget | `oni-app/src/render.rs::previews_keep_the_last_four_used` |
| Frame-rate meter `--fps` | `Pipeline/FrameRate.swift` | `oni-app/src/timing.rs` (`ONI_BENCH`) | done | | `oni-app/src/timing.rs::the_centiles_read_the_slow_tail_and_the_gaps_between_frames` |
| Key click sound, Mute Key Clicks | `DesignSystem/Keycap`, `UI/EditorCommands.swift` | none | missing | | no check |
| Haptics cadence | `DesignSystem/DSHaptics.swift` | none | missing | | no check |
| Self-checks `--check` | `Pipeline/SelfCheck.swift` | `cargo test` | partial | See [checks.md](checks.md) | no check |
| Command-line instruments (`--decoder-pins`, `--scanner-export`, `--autokey`, `--window-chroma`, `--scanner`, report keygen and unseal) | `Pipeline/CommandLineTools.swift` | none | missing | | no check |
| Probes and benches | `Pipeline/*Probe.swift`, `Bench.swift` | `oni-app` `ONI_BENCH` | partial | Bench only | no check |

## Updates and distribution

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Checks for updates on its own, never installs | `Pipeline/Updates.swift` | none | missing | | no check |
| Check for Updates… | `UI/EditorCommands.swift:105` | none | missing | | no check |
| UPDATE key in the bar | `Scene/EditorScene+Bar.swift` | none | missing | | no check |
| EdDSA feed, signed, notarised builds | `Makefile`, `Pipeline/Updates.swift:90-97` | none | missing | Windows needs its own channel | no check |
| `--check` never hits the network | `Pipeline/Updates.swift:51` | n/a | done | No updater to call | no check |

## Phone link (port only)

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Pair Phone… sheet with QR code | none | `oni-app/ui/pairing.slint`, `oni-app/src/phone.rs` | port-only | | `oni-app/src/phone.rs::the_qr_code_draws_its_finder_in_the_corner_inside_a_white_margin` |
| Trusted phone written once | none | `oni-app/src/phone.rs` | port-only | | `oni-app/src/phone.rs::a_trusted_phone_is_written_once_and_read_back` |
| Phone steps the open frame | none | `oni-link`, `oni-core/src/command.rs` | port-only | | `oni-app/tests/phone.rs::a_phone_pairs_from_the_sheet_and_steps_the_open_frame` |
| LocalSend transfer | none | `oni-localsend` | port-only | | `oni-localsend/tests/transfer.rs::sending_can_run_on_any_runtime_thread` |
