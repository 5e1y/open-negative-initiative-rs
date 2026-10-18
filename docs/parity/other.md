# Parity: presets, windows, help and the rest

Swift paths are under `Sources/OpenNegative/` of the Swift app; Rust paths are under `crates/`.
Statuses are defined in [README.md](README.md).

## Presets

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Preset file round trip, rename leaves one file | `Pipeline/Propagation.swift` `Preset` | `oni-core/src/presets.rs`, `oni-app/src/app/presets.rs` `commit_preset` | done |  | `oni-core/src/presets.rs::a_preset_round_trips_through_its_file_and_a_rename_leaves_one_file`, oni-app/tests/presets.rs::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does |
| Dialled per row, geometry absent | `Pipeline/Parameters.swift:21-67` | `oni-core/src/parameters.rs:73` | done |  | `oni-core/src/presets.rs::a_preset_carries_its_rows_and_never_the_framing` |
| Old preset reads its groups and retired rows | `Pipeline/Propagation.swift:169` | `oni-core/src/presets.rs:152` | done |  | `oni-core/src/presets.rs::an_old_preset_reads_its_groups_and_its_retired_rows` |
| Default name and file stem (64) | `Pipeline/Propagation.swift:189,366` | `oni-core/src/presets.rs:97-110` | done |  | `oni-core/src/presets.rs::names_and_files_are_formed_as_the_shelf_expects`, oni-app/tests/presets.rs::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does |
| Apply a preset, auto balance row measured | `Scene/EditorScene+Presets.swift` | `oni-core/src/store.rs` `apply_preset`, `app/presets.rs` `apply_preset_at` | done | One gallery step on the targets, then the balance key's own batch for a preset naming it | oni-app/tests/presets.rs::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does |
| Preset tints readable | `Pipeline/PresetTint.swift` | `oni-core/src/presets.rs:28` | done |  | `oni-core/src/presets.rs::every_tint_carries_a_readable_ink` |
| Preset deck: cards, peek, fade | `UI/PresetCarousel.swift:177-189` | `oni-app/ui/presets.slint` `PresetDeck` | partial | Cards, the peek and the rise under the pointer; no fade mask at the roll's ends | oni-app/tests/presets.rs::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does, `ONI_BENCH=presets` |
| Deck menu: Edit…, Update with Current Settings, Duplicate, Delete | `Scene/EditorScene+Panel.swift` | `app/presets.rs` `preset_card_menu` | done | Update and Delete ask first, as `PresetDialogs` does | oni-app/tests/presets.rs::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does |
| Share Preset…, Copy Preset Code, Paste Preset Code (`ONI1:`) | `Pipeline/Propagation.swift:295` | `app/presets.rs` `share_preset`, `copy_preset_code`, `adopt_preset_code` | done | Share writes through the save panel; Copy and Paste use the general clipboard | oni-app/tests/presets.rs::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does |
| Save as Preset… sheet | `UI/PresetEditor.swift` | `ui/presets.slint` `PresetSaveSheet`, `app/presets.rs` `save_preset` | done | Opens on the first free name and the rows the photo moved | oni-app/tests/presets.rs::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does |
| Preset editor: every parameter under its interface name | `UI/PresetParameters.swift` | `ui/presets.slint` `PresetEditorSheet` | done | The description is one line where Swift's field shows three | oni-app/tests/presets.rs::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does |
| Import Presets…, adopt incoming | `Scene/EditorScene+Presets.swift` | `app/presets.rs` `import_preset_files` | done |  | `oni-core/tests/swift_checks.rs::an_import_never_overwrites_and_a_duplicate_says_so`, oni-app/tests/presets.rs::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does |
| Show Presets Folder | `UI/EditorCommands.swift:231` | `app/presets.rs` `show_presets_folder` | done |  | no check |
| Retired shipped presets remembered | `Pipeline/Propagation.swift:242` | `app/presets.rs` `retire_shipped_presets` | done | Run once per support folder before the deck first shows | `oni-core/src/presets.rs::untouched_shipped_presets_are_taken_back_once` |

## Settings board

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Settings on the system pasteboard, own types | `Pipeline/Propagation.swift:22,96,127` | `oni-app/src/pasteboard.rs`, `oni-app/src/app/board.rs`, `oni-core/src/store.rs:496` | partial | Settings, framing and measuring window under Swift's three types on macOS, the other two laid back after a clear; Windows keeps a board of the process's own | `oni-app/src/pasteboard.rs::a_copy_keeps_the_other_two_copies_and_bumps_the_count`, `::the_system_board_holds_the_apps_own_types_and_never_a_text`, `oni-app/tests/editor_leftovers.rs::the_editors_last_gestures_behave_as_swifts` |
| Paste carries Color and Details whole | `Pipeline/LookSettings.swift:6` | `oni-settings/src/settings.rs:198` | done | Core level | `oni-core/tests/roll8.rs::a_paste_carries_the_color_and_details_tabs_whole_and_undoes_byte_for_byte` |
| Every field in exactly one family | `Pipeline/RollSettings.swift:12`, `LookSettings.swift` | `oni-core/src/fields.rs` | done | | `oni-core/src/fields.rs::every_field_is_the_rolls_the_looks_or_the_frames_and_one_only` |
| Framing board, measuring window board | `Pipeline/Propagation.swift:96,127` | `oni-core/src/store.rs` `copy_framing`, `paste_framing`, `app.rs` `framing_act` | done | Both boards live for the session, as `Propagation`'s; only the open frame is fitted | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |
| Paste check (what would be overwritten) | `Scene/EditorScene+Paste.swift` | `oni-core/src/store.rs` `paste_check`, `app.rs` `confirm` | done | Another roll's look asks on the Another Roll sheet | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |

## Windows and sheets

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Editor window, dark appearance pinned | `OpenNegativeApp.swift`, `Pipeline/QuitGuard.swift:12` | `oni-app/ui/app.slint` | done | | `oni-app/tests/headless.rs::the_window_opens_roll_8_and_a_slider_drives_the_canvas` |
| Console window | `UI/ConsoleView.swift`, `Pipeline/Diagnostics.swift` | `ui/help.slint` `ConsoleWindow`, `app/help.rs` `open_console` | partial | The session's log polled every 0.5 s with Copy and Reveal; no watchdog counts blocks, so it always says none | `ONI_BENCH=console` |
| Design system playground (dev builds) | `DesignSystem/DSPlayground.swift` | `oni-app` `--catalogue` | done | | `oni-app/tests/catalogue.rs::the_catalogue_shows_every_component` |
| Welcome sheet with tutorial video | `UI/WelcomeSheet.swift`, `UI/TutorialVideo.swift` | `ui/help.slint` `WelcomeSheet`, `app/help.rs` | partial | Shown once per edition with Swift's text, Website, Feedback and GET STARTED; the picture's caption sits under it, and no tutorial video plays | oni-app/tests/help.rs::the_help_menus_surfaces_behave_as_swifts |
| What's New, shown once per edition | `UI/WhatsNew.swift`, `DesignSystem/DSNews.swift` | `ui/ds/help.slint` `NewsSeal`, `app/help.rs` | done | Each note's seal on its heading until hovered, kept until the app quits; Show What's New Again | oni-app/tests/help.rs::the_help_menus_surfaces_behave_as_swifts |
| Report a Bug sheet, sealed log | `UI/ReportSheet.swift`, `Pipeline/BugReport.swift`, `ReportSeal.swift` | `oni-app/src/report.rs`, `ui/help.slint` `ReportSheet` | done | The log sealed as CryptoKit seals it, checked against CryptoKit both ways (`tools/seal_cross.swift`); posted to the site's `/report` | oni-app/tests/help.rs::the_help_menus_surfaces_behave_as_swifts, `oni-app/src/report.rs::a_sealed_log_opens_with_the_private_key_alone`, `oni-app/src/report.rs::a_seal_crosses_to_cryptokit_and_back` |
| Show My Year on Film (Wrapped) | `UI/WrappedView.swift`, `Pipeline/Wrapped.swift` | `oni-library/src/wrapped.rs`, `oni-app/src/wrapped.rs`, `app/year.rs`, `ui/wrapped.slint`, `ui/ds/wrapped.slint` | done | Figures grouped by commas whatever the region; the title grows in without Swift's blur and phosphor glow, and a turned card fades rather than flying up | `oni-app/tests/wrapped.rs::every_gesture_is_noted_and_the_year_reads_back_as_its_deck`, `oni-library/src/wrapped.rs::a_years_gestures_read_back_as_its_cards_and_a_year_later_count_for_nothing`, `oni-app/src/wrapped.rs::the_sample_year_deals_every_card_in_swifts_words` |
| Year's journal written on each gesture | `Pipeline/Journal.swift` | `app/year.rs`, `export_card.rs`, `app/prepare.rs` | done | Edits are read when the sidecar write settles, not at Swift's 150 ms history step; notes are appended on the interface thread rather than a queue | `oni-app/tests/wrapped.rs::every_gesture_is_noted_and_the_year_reads_back_as_its_deck`, `oni-library/src/journal.rs::one_support_folder_holds_one_journal_for_the_process` |
| Splash | `UI/Splash.swift` | `oni-app/src/splash.rs`, `main.rs` | done | The version's picture, then the window; skipped under a bench | `oni-app/tests/launch.rs::the_splash_names_the_version_and_the_notice_covers_the_window` |
| Stock picker sheet | `UI/StockPickerSheet.swift` | `ui/stocks.slint` `StockPickerSheet`, `app/stock_picker.rs` | done | By brand with each stock's artwork, from the roll's info and the export's film field; six columns where Swift's adaptive grid lays ten, so names read whole in the simplified look | oni-app/tests/stocks.rs::a_rolls_stock_is_picked_by_brand_and_shown_with_its_artwork, `oni-app/tests/export_window.rs::the_export_surface_writes_what_the_proofed_canvas_shows_and_only_onis_metadata` |
| Paper profiles sheet | `UI/PaperProfilesSheet.swift` | `oni-app/src/paper_sheet.rs`, `ui/export.slint` `PaperSheet`; see [export-colour.md](export-colour.md) | done | | `oni-app/tests/paper_sheet_window.rs::a_maker_s_profile_is_listed_installed_chosen_and_uninstalled` |
| Popovers and dialogs (`dsPopover`, `DSDialog`) | `DesignSystem/DSPopover.swift`, `DSDialog.swift` | `ui/ds/forms.slint` `Popover`, `ui/presets.slint` `AdviceSheet` | partial | Export's pickers open the popover, confirmations and notices use the dialog; the Dyes picker and Select Roll Type open nothing | no check |
| Help bubbles on a layer above the pane | `DesignSystem/DSHelp.swift` | `ui/ds/containers.slint` `HelpMark`, `ui/ds/base.slint` `TipLayer` | partial | The help mark's note draws on the window's top layer after a tooltip delay; Swift's shows at once | no check |

## Application life

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Quit waits for the sidecar, refuses on a failed write | `Pipeline/QuitGuard.swift:31` | `app.rs` `close_requested`, `Pending::QuitAnyway` | done | Asks with Swift's words, Cancel the default | oni-app/tests/quit.rs::closing_writes_the_last_settings_and_a_failed_write_asks_first |
| A newer sidecar opens read-only | `Pipeline/Sidecar.swift` | `oni-settings` sidecar | done | | `oni-settings/tests/real_files.rs::a_newer_format_opens_read_only_and_is_never_rewritten` |
| Sidecar batches off the main thread | `Pipeline/SidecarBatch.swift` | none | missing | | no check |
| Main-thread watchdog, anonymous console log | `Pipeline/Diagnostics.swift` | none | missing | | no check |
| Cache budget follows the machine | `Pipeline/MemoryBudget.swift` | `oni-app/src/viewport.rs` | partial | The budget formula is ported but always fed 24 Mpx; previews are a fixed four | `oni-app/src/render.rs::previews_keep_the_last_four_used` |
| Frame-rate meter `--fps` | `Pipeline/FrameRate.swift` | `oni-app/src/timing.rs` (`ONI_BENCH`) | done | | `oni-app/src/timing.rs::the_centiles_read_the_slow_tail_and_the_gaps_between_frames` |
| Key click sound, Mute Key Clicks | `DesignSystem/Keycap`, `UI/EditorCommands.swift` | `app.rs` `act` "toggle-sound" | partial | The choice is remembered under `soundMuted`; this build plays no key clicks | oni-app/tests/session.rs::the_tile_size_and_the_muted_clicks_outlast_the_session |
| Haptics cadence | `DesignSystem/DSHaptics.swift` | none | missing | | no check |
| Self-checks `--check` | `Pipeline/SelfCheck.swift` | `cargo test` | partial | See [checks.md](checks.md) | no check |
| Command-line instruments (`--decoder-pins`, `--scanner-export`, `--autokey`, `--window-chroma`, `--scanner`, report keygen and unseal) | `Pipeline/CommandLineTools.swift` | none | missing | | no check |
| Probes and benches | `Pipeline/*Probe.swift`, `Bench.swift` | `oni-app` `ONI_BENCH` | partial | Bench only | no check |

## Updates and distribution

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Checks for updates on its own, never installs | `Pipeline/Updates.swift` | `oni-app/src/updates.rs`, `app/updates.rs` | done | At launch then every 43 200 s from `SULastCheckTime`; a scheduled find shows the key, not Sparkle's alert; a development build (no `ONI_BUILD`) looks only when asked; the install count (`/installed`) is not sent; the feed still serves the Swift app's archives | `oni-app/tests/updates.rs::updates_are_looked_for_on_their_own_shown_on_the_key_and_only_ever_downloaded`, `oni-app/src/updates.rs::the_feed_reads_its_items_and_the_newest_runnable_one_is_offered`, `oni-app/src/updates.rs::versions_compare_as_sparkle_compares_them` |
| Check for Updates… | `UI/EditorCommands.swift:105` | `app.slint` Help menu, `app/updates.rs` `check_for_updates` | done | First in Help: Slint builds the macOS application menu itself; ends on Sparkle's found, up-to-date and error dialogs, without the release notes | `oni-app/tests/updates.rs::updates_are_looked_for_on_their_own_shown_on_the_key_and_only_ever_downloaded` |
| UPDATE key in the bar | `Scene/EditorScene+Bar.swift` | `toolbar.slint`, `app/updates.rs` `publish_update_key` | done | Download keeps the verified archive in Downloads and shows it; nothing installs | `oni-app/tests/updates.rs::updates_are_looked_for_on_their_own_shown_on_the_key_and_only_ever_downloaded` |
| EdDSA feed, signed, notarised builds | `Makefile`, `Pipeline/Updates.swift:90-97` | `oni-app/src/updates.rs` `verify`, `checked` | partial | Every archive checked against `SUPublicEDKey` before it is kept; the Rust app is not yet signed, notarised or published, and Windows needs its own channel (no item runs off macOS) | `oni-app/src/updates.rs::a_signed_archive_verifies_and_a_forged_or_truncated_one_does_not`, `oni-app/src/updates.rs::the_published_archives_verify_against_the_apps_key` (`ONI_RELEASES_DIR`) |
| `--check` never hits the network | `Pipeline/Updates.swift:51` | n/a | done | No updater to call | no check |

## Phone link (port only)

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Pair Phone… sheet with QR code | none | `oni-app/ui/pairing.slint`, `oni-app/src/phone.rs` | port-only | | `oni-app/src/phone.rs::the_qr_code_draws_its_finder_in_the_corner_inside_a_white_margin` |
| Trusted phone written once | none | `oni-app/src/phone.rs` | port-only | | `oni-app/src/phone.rs::a_trusted_phone_is_written_once_and_read_back` |
| Phone steps the open frame | none | `oni-link`, `oni-core/src/command.rs` | port-only | | `oni-app/tests/phone.rs::a_phone_pairs_from_the_sheet_and_steps_the_open_frame` |
| LocalSend transfer | none | `oni-localsend` | port-only | | `oni-localsend/tests/transfer.rs::sending_can_run_on_any_runtime_thread` |
