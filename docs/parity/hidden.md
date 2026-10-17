# Parity: hidden behaviour

What the interface does not show: constants, compensations, interaction rules, codec defaults and
migrations, and which pipeline branches a Rust-vs-Swift numeric test reaches. Swift paths are under
`Sources/OpenNegative/`; Rust paths are under `crates/`. Statuses are defined in [README.md](README.md).

A value said "same" was compared literally against the Swift declaration. A model constant can be
"done" while the feature reading it is missing: the row then says so.

## Compensations and models

| Behaviour | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Density, not `1 − x`; pedestal lifts, never floors | `Pipeline/Pipeline.swift:229-240` | `oni-settings/src/axis.rs:12-15`, `oni-render` stage 5 | done | | `oni-pipeline/tests/roll8.rs::roll8_lands_on_the_swift_render` |
| Density axis: highest 2.6, mosaic rest 3.5, guard `log10(8)` | `Pipeline/DensityAxis.swift:8-35` | `oni-settings/src/axis.rs:48-82` | done | | `oni-settings/src/density_migration.rs::the_migrated_windows_render_the_frozen_kernel` |
| Hunt compensation on the print curve (exponent 0.19, reach 4) | `Pipeline/Print.swift:259-268` | `oni-settings/src/tone.rs:289-290,363` | done | | `oni-settings/tests/kernel_oracle.rs::the_print_replica_is_the_kernel_s_print` |
| Hunt absent in monochrome | `Pipeline/Print.swift:314` | `oni-pipeline/src/conversion.rs` | done | | `oni-pipeline/src/conversion.rs::a_monochrome_frame_carries_no_hunt_density_or_unmix` |
| White and black points' saturation compensation (gain 2.25) | `Pipeline/Print.swift:198-221` | `oni-settings/src/tone.rs:285`, `oni-pipeline/src/finishing.rs:76` | done | | `oni-pipeline/src/finishing.rs::the_points_lay_a_line_of_saturation` |
| Points held to a minimum span 0.02, inward moves first | `Pipeline/Print.swift:211-220` | `oni-settings/src/tone.rs:284` | done | | `oni-ui-poc/src/frame.rs::the_points_hold_a_minimum_span` |
| Points reach into each window; bypassed print leaves them | `Pipeline/Print.swift:241` | `oni-pipeline/src/conversion.rs:18` | done | | `oni-pipeline/src/conversion.rs::the_points_reach_into_each_window_and_a_bypassed_print_leaves_them` |
| Print tone: toe, push (0.35, 0.15), quarters, contrast pivot 0.4587, steepness 15.2 | `Pipeline/Print.swift:47-82,190`, `Contrast.swift:8-12` | `oni-settings/src/tone.rs:14-15,117-118,279` | done | | `oni-settings/src/tone.rs::contrast_is_monotone_pinned_and_inverted_by_its_opposite` |
| Resting exposure 1.5 negative, 0 slide | `Pipeline/Print.swift:224-225` | `oni-settings/src/tone.rs:288` | done | | `oni-settings/tests/codec_checks.rs::the_rest_is_the_import_state_of_each_mode` |
| Temperature and Tint axes `(½, 0, −½)`, `(⅓, −⅔, ⅓)` | `Pipeline/Print.swift:176-177` | `oni-settings/src/tone.rs:269-270` | done | | `oni-core/src/knob.rs::temperature_and_tint_move_on_axes_that_leave_each_other_alone` |
| Colour filters reach no gain | `Pipeline/Curves.swift` `colourFilterChecks` | `oni-core/src/parameters.rs` | done | | `oni-core/src/parameters.rs::lighting_leaves_the_filters_and_color_carries_them` |
| Dye separation: 3×3 on balanced densities, rows sum to one, five families | `Pipeline/Separation.swift:41-151` | `oni-pipeline/src/conversion.rs:36-86` | done | Rows literal, not derived from the 55 nm band (`validatedStrength 0.407` unchecked) | `oni-pipeline/src/conversion.rs::the_separation_keeps_greys_and_the_mean_saturation` |
| Separation chroma kept | `Pipeline/Separation.swift:109-119` | `oni-pipeline/src/conversion.rs:66` | done | | `oni-pipeline/src/conversion.rs::the_separation_keeps_greys_and_the_mean_saturation` |
| Chroma decompression: colour follows the windows by its dose (0.5) | `Pipeline/ChromaDecompression.swift:9-54` | `oni-pipeline/src/conversion.rs:104-119` | done | | `oni-pipeline/tests/roll8.rs::roll8_lands_on_the_swift_render` |
| Color density factor `2^travel` | `Pipeline/ColorDensity.swift:6-17` | `oni-settings/src/tone.rs:49-51`, `conversion.rs:91` | done | | `oni-render/tests/stage47.rs::measured` |
| Gains (stage 4) opposite to the Lighting filters in negative | `Pipeline/Gains.swift:7-9` | `oni-settings/src/blocks.rs:15` | done | | `oni-pipeline/tests/roll8.rs::roll8_lands_on_the_swift_render` |
| Output clip the only compression, hard | `Pipeline/Pipeline.swift` | `oni-render` output clip | done | | `oni-render/tests/kernels.rs::output_clip` |
| Flat-field before the crop, colour reach 0.5 | `Pipeline/FlatField.swift:40-118` | `oni-pipeline/src/flat_field.rs:11-65` | done | Shipped mask only | `oni-pipeline/src/flat_field.rs::the_colour_sliders_turn_round_with_the_mode` |
| Flat-field sliders remembered per Mac | `Pipeline/FlatField.swift:54-66` | none | missing | | no check |
| Custom flat-field masks library | `Pipeline/CustomFlatFields.swift` | none | missing | | no check |
| Capture exposure evened in stops | `Pipeline/CaptureExposure.swift` | `oni-pipeline` (stored stops) | partial | Applied, never read off the EXIF | no check |
| Vignette: midpoint 0.3, Hunt law, range ±2 | `Pipeline/Vignette.swift:10-18` | `oni-render/src/params.rs:256`, `oni-settings/src/blocks.rs:89` | done | | `oni-pipeline/tests/stages.rs::the_vignette_centres_on_its_extent` |
| Chrome (secret sauce) curves | `Pipeline/SaturationCurve.swift:82-86` | `oni-pipeline/src/finishing.rs:53-59` | done | | `oni-pipeline/src/finishing.rs::the_sauce_is_neutral_at_zero_and_continuous_at_one` |
| Saturation curve resting points, spacing 0.04 | `Pipeline/SaturationCurve.swift:8-11` | `oni-settings/src/curves.rs:220-231` | done | | `oni-render/tests/kernels.rs::saturation_curve` |
| Spectrogram bands: margin 3, spread 1.8 | `Pipeline/Spectrogram.swift:68-82` | `oni-settings/src/spectrogram.rs:52-55`, `finishing.rs:92-118` | done | | `oni-pipeline/src/finishing.rs::every_band_owns_part_of_its_centre_and_the_packing_divides_by_it` |
| Manual curves natural cubic spline, LUT 1024, fold 16 | `Pipeline/Curves.swift:110-161` | `oni-settings/src/curves.rs:29-118`, `oni-render/src/params.rs:143` | done | | `oni-settings/src/curves.rs::the_spline_passes_through_its_points` |
| Sharpen: anchor 2.092, ranges, halo | `Pipeline/Sharpen.swift:15-33` | `oni-settings/src/blocks.rs:118-128` | done | | `oni-pipeline/tests/stages.rs::detail_matches_its_reference_whole_and_on_a_region` |
| Chroma denoise after the inversion, radius 0…6 | `Pipeline/ChromaDenoise.swift:13-25` | `oni-settings/src/blocks.rs:150-153` | done | | `oni-pipeline/tests/stages.rs::detail_matches_its_reference_whole_and_on_a_region` |
| Dosed radii in full-resolution pixels, blur floor 0.1614 | `Pipeline/Pipeline.swift:240`, `SettingsCodec.swift:291` | `oni-settings/src/axis.rs:15`, `codec.rs:26` | done | | `oni-settings/tests/codec_checks.rs::the_two_radii_move_onto_pixels_before_the_defaults` |
| Texture: narrow, wide, peak 0.82, edge 0.125, base 0.6 | `Pipeline/Texture.swift:9-27` | `oni-pipeline/src/finishing.rs:142-150` | done | | `oni-render/tests/kernels.rs::texture` |
| Scanner Frontier table, ONI machine (lift 0.5, dose 0.6, scale 1.3) | `Pipeline/ScannerRender.swift:52-86` | `oni-pipeline/src/scanner.rs:17-80` | done | | `oni-pipeline/src/scanner.rs::the_table_reads_and_an_odd_count_sits_between_its_neighbours` |
| Scanner clicks: density ±24, warmth ±10, held to stops | `Pipeline/ScannerClicks.swift:24-25` | `oni-settings/src/scanner.rs:28-29` | done | | `oni-settings/src/scanner.rs::a_keypad_reads_absent_keys_at_rest_and_clamps_past_its_stops` |
| ONI look cube (64 nodes, root 2, Rec. 709 luma) | `Pipeline/OniLook.swift:58-197` | `oni-pipeline/src/oni_look.rs:79-82` | done | | `oni-pipeline/src/oni_look.rs::the_cube_is_red_fastest_on_rooted_code` |
| Scanner look recipes ONI, Noritsu, Frontier | `Pipeline/ScannerLook.swift:63-81` | `oni-settings/src/scanner.rs:78-149` | done | | `oni-settings/src/scanner.rs::a_folded_look_grades_the_curves_as_its_replica_did` |
| Scanner Finish apart from Pro's | `Pipeline/ScannerFinish.swift:25` | `oni-settings/src/settings.rs:164` | partial | Read for the render; nothing stores a Scanner-face edit into it | no check |
| Paper correction gains, probe 0.18 | `Pipeline/PaperCorrection.swift:9-31` | `oni-settings/src/blocks.rs:172-175` | done | | `oni-settings/src/blocks.rs::the_paper_gains_are_powers_of_two_and_neutral_moves_nothing` |
| Screen gamut tolerance 0.01, period 9, duty 0.34 | `Pipeline/ScreenGamut.swift:14-29` | `oni-colour/src/gamut.rs:11-29` | done | | `oni-colour/tests/checks.rs::the_zebra_stripes_what_the_screen_clips` |
| Proof cube 33 in Rec. 2020 | `Pipeline/Proof.swift:53-54` | `oni-colour/src/proof.rs:37` | done | | `oni-colour/tests/checks.rs::the_proof_clips_what_the_export_space_cannot_hold` |
| Gain map lift 4, subsampling 2 | `Pipeline/GainMapContainer.swift:10-12` | `oni-export/src/gain_map.rs:8-10` | done | | `oni-export/tests/files.rs::the_container_says_two_stops_in_both_families` |
| Export sample 500 px, preview 1600 px | `Pipeline/Export.swift:8-12` | `oni-export/src/lib.rs:24-27` | done | | no check |
| Geometry bleed margin 1, angle ±8 | `Pipeline/Geometry.swift:26,401` | `oni-pipeline/src/geometry.rs:11`, `oni-settings/src/frame.rs:74` | done | | `oni-pipeline/src/geometry.rs::the_rotation_keeps_the_centre_and_covers_its_bounding_box` |
| Raw texture side 16 384 | `Pipeline/RawDecode.swift:181` | `oni-graph/src/graph.rs:16` | done | | `oni-graph/tests/graph.rs::a_tiled_source_equals_an_untiled_one` |
| Decode pinned 5000 K, tint 0 | `Pipeline/RawDecode.swift:14-26` | `oni-decode/src/lib.rs:46`, `csrc/pinned.c` | done | LibRaw by design; `CIRAWFilter` knobs (noise 1, sharpness 0, shadow bias 0, version 8) have no LibRaw twin | `oni-decode/src/lib.rs::the_5000_k_white_reads_neutral_at_the_baseline` |
| Sort decodes on the bytes, not the extension | `Pipeline/RawDecode.swift` | `oni-decode/src/container.rs` | done | | `oni-decode/src/container.rs::the_bytes_decide_the_branch` |
| Undeclared file read as the hand says (Source Encoding) | `Pipeline/SourceEncoding.swift` | `oni-settings` field | partial | Field only; no plain-image decode | no check |
| DNG container read: illuminants 17, 20, 21, 23 | `Pipeline/ContainerRead.swift:388` | `oni-decode/src/container.rs` | partial | Linear table read; converted primaries refused | `oni-decode/src/container.rs::a_linear_scan_reads_through_its_table_black_and_white` |
| RGB passes read side 360, registration floor 0.3, 3 frames | `Pipeline/RGBPasses.swift:119`, `RGBRegistration.swift:30-35` | none | missing | | no check |

## Measurement behind the automatic keys

| Behaviour | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Auto White Balance: Minkowski p = 6 at 480 px, greyed with Temperature and Tint | `Pipeline/FilterBalance.swift:15-22` | `oni-core/src/balance.rs:111` trait only | missing | No `Measure` implementation | no check |
| Outcome dosed by Balance Strength 0.67 | `Pipeline/FilterBalance.swift:21-22` | `oni-core/src/balance.rs:88` | done | | `oni-core/src/balance.rs::the_outcome_is_dosed_by_balance_strength` |
| A selection's average gives every frame the mean | `Pipeline/AutomaticKey.swift` | `oni-core/src/balance.rs:98` | done | | `oni-core/src/balance.rs::average_gives_every_frame_the_mean_and_an_unreadable_frame_stops_the_run` |
| Measuring blocks: 933/48 px cells, tail 0.08, named cells, speck dropped | `Pipeline/Blocks.swift:10-146` | none | missing | | no check |
| Classic placement of the ends in two steps on blocks | `Pipeline/AutoLevels.swift:268-299` | none | missing | | no check |
| Auto levels: threshold 0.001, quantiles, recentred, minimum span 0.02 | `Pipeline/AutoLevels.swift:10-195` | `oni-core/src/balance.rs:22-54` | partial | Placement of a suggestion only; nothing suggests | no check |
| Pipette: radius 4 (2…24), density space, mean | `Pipeline/Pipette.swift:7-51` | none | missing | | no check |
| Measuring window: full window, inset 0.1, proposed for a sheet | `Pipeline/Geometry.swift:110-143` | `oni-settings/src/frame.rs` field | missing | | no check |
| Measure side 1400, slot measure 640 | `Negative.swift:50`, `UI/MetalImageView.swift:204` | `oni-graph/tests/roll8.rs:17` (test only) | missing | | no check |
| Roll reference: factor 0.25…4, base radius, outvotes one frame | `Pipeline/RollReference.swift:22-27` | `oni-settings/src/frame.rs:238` | partial | Range only; no measurement | no check |
| Automatic keys write only what they show, one ⌘Z | hard rule 1 | `oni-core/src/store.rs:635` | done | Core level | `oni-core/tests/roll8.rs::the_balance_key_writes_only_on_a_press_and_one_undo_takes_it_back` |

## Interaction rules

| Behaviour | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Slider drag relative; Option fine ×0.2 | `DesignSystem/DSHandle.swift:87-90` | `oni-app/src/track.rs:8,42` | done | | `oni-app/src/track.rs::a_drag_is_relative_and_option_slows_it` |
| Slider values reach the settings 30 times a second | `DesignSystem/DSSlider.swift:150` | `oni-app/src/track.rs:10` | done | | `oni-app/src/track.rs::values_reach_the_settings_thirty_times_a_second` |
| Double-click resets a slider to its base | `DesignSystem/DSSlider.swift:101-103` | `oni-app/src/app.rs:1193-1210` | partial | Fixed 500 ms, not the system interval; base is Negative's on a slide | no check |
| Bent tracks (centred) | `DesignSystem/DSSlider.swift:180` | `oni-app/src/track.rs:17-31` | done | | `oni-app/src/track.rs::a_bent_track_is_its_own_inverse` |
| Scroll over a slider: wheel line 10, trackpad travel 0.25, idle 0.16 s | `DesignSystem/DSScrollAdjust.swift:11-24` | `oni-app/src/track.rs:14` | partial | Trackpad travel only | no check |
| Nested swipe arbitration, bias 1.5 | `DesignSystem/DSScrollAdjust.swift:130` | none | missing | | no check |
| Spring on outside changes, none under the hand | `DesignSystem/DSSlider.swift:91` | `oni-app/src/motion.rs` | done | | `oni-app/src/motion.rs::settle_overshoots_slightly_and_travel_not_at_all` |
| Deferred refresh follows the hand (stale after 1 s) | `DesignSystem/DSGesture.swift:19` | `oni-app/src/track.rs:69-94` | done | | `oni-app/src/track.rs::values_reach_the_settings_thirty_times_a_second` |
| A burst of notches renders the first and last | `Scene/EditorScene+Stages.swift` | `oni-app/src/render.rs` | done | | `oni-app/src/render.rs::a_burst_of_notches_renders_the_first_and_the_last_only` |
| Levels: place, single write path, per-handle reset, grips front to back | `UI/LevelsView.swift:162-240` | none | missing | | no check |
| Levels anchor clamp 0.02…0.98, display shadows and highlights | `Pipeline/Levels.swift:192` | `oni-settings/src/levels.rs:100-104` | partial | `set_black`, `set_white` only | no check |
| Levels black reachable under zero | `UI/LevelsView.swift` | none | missing | | no check |
| Curve: drag adds a point outside a 14 pt radius, double-tap removes | `UI/CurveEditorView.swift:67-100` | none | missing | | no check |
| Curve points: margin 0.001, endpoints undeletable, clamped | `Pipeline/Curves.swift:16-33` | none | missing | | no check |
| Saturation curve: double-tap to neutral, closest along | `UI/SaturationCurveView.swift` | none | missing | | no check |
| Spectrogram wheel grabbed, never jumped; step 0.02 | `UI/SpectrogramPanel.swift`, `Pipeline/Spectrogram.swift:86` | none | missing | | no check |
| Spectrogram luma nudge rest 0.5, reach 0.25; Option | `UI/SpectrogramPanel.swift:81,113-114` | none | missing | | no check |
| Bipolar gain rows name the end the hand reaches | `UI/GainPanel.swift` | none | missing | | no check |
| Temperature and Tint tracks end on their colours | `UI/TemperatureTintPanel.swift:23-24` | none | missing | | no check |
| Canvas zoom per wheel notch 1.10, per trackpad point 1.0025 | `UI/CanvasNavigation.swift:8-22` | none | missing | | no check |
| Viewport range 0.02…4, settle 150 ms, region margin 0.3 | `UI/Viewport.swift:13-131` | none | missing | | no check |
| Crop handles: corners, edges, min crop 0.05, ratio held | `UI/CropOverlay.swift:280-281`, `Geometry.swift:30` | none | missing | | no check |
| Straighten re-fits the crop inside the image | `Pipeline/Geometry.swift:195,234` | `oni-core/src/knob.rs:242` | partial | Raw angle written | no check |
| Rotate keeps the crop, mirror flips it | `Pipeline/Geometry.swift:39-101` | none | missing | | no check |
| Framing paste adopts, fits inside the image | `Pipeline/Geometry.swift:101,195` | `oni-core/src/store.rs:560` | partial | No fit | no check |
| Gallery ⌘ and ⇧ click pick as shown | `UI/ListPicking.swift` | `oni-app/ui/library.slint` | done | | no check |
| Select all, arrows walk the grid | `UI/GalleryEngine.swift:425-439` | none | missing | | no check |
| Column resizer for the filmstrip | `DesignSystem/DSPanel` `DSColumnResizer` | none | missing | | no check |
| A chip across slots lands in the nearest (DSSlots) | `DesignSystem/DSReorder.swift` | none | missing | | no check |
| Pager: anchoring, clicked travel, swipe end of course | `DesignSystem/DSPanel.swift` | none | missing | | no check |
| Help bubbles fit where they hang | `DesignSystem/DSHelp.swift` | none | missing | | no check |
| Hover fallback | `DesignSystem/DSHover.swift` | none | missing | | no check |
| Keycap press floor 140 ms before acting | `DesignSystem/Keycap` | `oni-app/ui/ds/` | partial | Flat buttons; no press animation | no check |
| Roll Settings gesture writes every frame on the reference, one step | `Scene/EditorScene+RollSettings.swift`, `RollSettings.swift:36` | `oni-core/src/store.rs:364` | done | | `oni-core/tests/roll8.rs::a_roll_settings_gesture_writes_every_frame_of_the_roll_as_one_step_and_undoes_byte_for_byte` |
| The bar's reset keeps geometry, flat-field, dye family | `Pipeline/Parameters.swift:257` | `oni-core/src/parameters.rs:337` | done | | `oni-core/src/parameters.rs::the_reset_keeps_geometry_flat_field_and_dye_family` |
| Block reset to the mode's rest | `Pipeline/Parameters.swift:246` `rest(for:)` | `oni-app/src/store.rs:91-92` | partial | Always Negative's rest | `oni-app/src/store.rs::a_block_reset_lands_on_the_rest_in_one_write` (negative only) |
| Dye Separation reset clears family and by-hand | `UI/SettingsPane.swift:74` | `oni-app/src/store.rs:100` | partial | Dose only | no check |
| Mode change carries a block onto the next mode | `Pipeline/Parameters.swift:286` | `oni-core/src/parameters.rs:348` | partial | Core only | no check |
| Reset to Default reaches whole rolls and sheets, never a neighbour | `Pipeline/RollSettings.swift:114` | none | missing | | no check |
| Sidecar write delay 400 ms | `Pipeline/Sidecar.swift:7` | `oni-app/src/app.rs:34` | partial | 500 ms | no check |
| Thumbnail refresh 2 s, after the sidecar write | `Pipeline/Thumbnails.swift:21` | none | missing | | no check |
| Histogram throttle 12 ms, 100 ms while held | `Pipeline/Histogram.swift:59-63` | none | missing | | no check |
| Histogram graduation overshoot 0.35 | `Pipeline/Histogram.swift:20` | `oni-settings/src/axis.rs:105` | done | | no check |
| Medal keys 0-3 bare, live only outside text | `UI/EditorCommands.swift:183` | `oni-app/src/app.rs:1172-1175` | done | | no check |

## Codec, sidecar and index

| Behaviour | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Codec version 19, newer opens read-only | `Pipeline/SettingsCodec.swift:28` | `oni-settings/src/codec.rs:23` | done | | `oni-settings/tests/codec_checks.rs::empty_reads_neutral_garbage_fails_and_the_future_is_flagged` |
| Migration order before `merged()` | `Pipeline/SettingsCodec.swift:130-181` | `oni-settings/src/codec.rs` | done | | `oni-settings/tests/codec_checks.rs::the_envelope_reads_what_the_full_decode_reads` |
| Pre-1.0 chain, versions 3-13 | `Pipeline/SettingsCodec+Legacy.swift:7-70` | `oni-settings/src/legacy.rs:13-35` | done | | `oni-settings/tests/codec_checks.rs::the_separation_chain_before_1_0_lands_where_each_step_says` |
| Density axis migration (v3) | `SettingsCodec+Legacy.swift:33`, `DensityMigration.swift` | `oni-settings/src/density_migration.rs` | done | | `oni-settings/src/density_migration.rs::the_migrated_windows_render_the_frozen_kernel` |
| Radii onto pixels (v4, side 8368) | `SettingsCodec+Legacy.swift:37`, `SettingsCodec.swift:291` | `legacy.rs:17`, `codec.rs:26` | done | | `oni-settings/tests/codec_checks.rs::the_two_radii_move_onto_pixels_before_the_defaults` |
| Positive rest (v5, mid 0.5) | `SettingsCodec+Legacy.swift:41`, `SettingsCodec.swift:185` | `legacy.rs:19,38` | done | | `oni-settings/tests/codec_checks.rs::a_positive_from_before_the_move_opens_on_the_new_rest` |
| Neutral decode gains (v6, ratio 1.7683, 1.1011, 0.38227) | `SettingsCodec+Legacy.swift:45`, `SettingsCodec.swift:287` | `legacy.rs:21,42` | done | | `oni-settings/tests/codec_checks.rs::a_mosaic_gain_shifts_for_the_decode_change_and_a_scanner_gain_never` |
| Separation rest (v7), sharpen (v8), lighting (v9), chroma (v10), black point (v11) | `SettingsCodec+Legacy.swift:48-61` | `legacy.rs:23-31` | done | | `oni-settings/tests/codec_checks.rs::sharpen_chroma_and_black_point_rests_move_once` |
| Mosaic axis (v12), decompression rest (v13) | `SettingsCodec+Legacy.swift:64-67` | `legacy.rs:33-35` | done | | `oni-settings/tests/codec_checks.rs::decompression_rests_move_off_the_key_and_keep_the_hand` |
| Released chain v14-v19: decompression mid, primaries held, separation by hand, literature, half step, ONI machine, look folded | `Pipeline/SettingsCodec.swift:188-248` | `oni-settings/src/codec.rs:29-41` | done | | `oni-settings/tests/codec_checks.rs::a_document_from_before_folds_its_scanner_look_and_one_since_carries_none` |
| Lighting catch-up fits a former document | `Pipeline/LightingCatchUp.swift:192-279` | `oni-settings/src/lighting_catch_up.rs:267-410` | done | | `oni-settings/tests/kernel_oracle.rs::a_former_document_renders_its_greys_through_the_kernel_as_it_did` |
| Former balance lands in the filters | `Pipeline/SettingsCodec.swift` | `oni-settings/src/codec.rs` | done | | `oni-settings/tests/codec_checks.rs::a_former_balance_lands_in_the_filters_and_its_greys_render_as_they_did` |
| Absent stage reads neutral, neighbours intact | `Pipeline/SettingsCodec.swift` | `oni-settings/src/codec.rs` | done | | `oni-settings/tests/codec_checks.rs::an_absent_stage_or_field_reads_neutral_its_neighbours_intact` |
| Crop display flag stays out of the sidecar | `Pipeline/Geometry.swift` | `oni-settings/src/frame.rs` | done | | `oni-settings/tests/codec_checks.rs::the_crop_display_flag_stays_out_of_the_sidecar` |
| JSON spelled as Foundation spells it | `JSONEncoder` | `oni-settings/src/json.rs` | done | | `oni-settings/tests/real_files.rs::a_sidecar_read_and_written_back_is_the_same_file` |
| Every roll #8 sidecar round-trips byte for byte | Swift files | `oni-settings`, `oni-core` | done | | `oni-core/tests/roll8.rs::every_sidecar_of_the_roll_rewrites_to_its_own_bytes` |
| Sidecar EXIF block | `Pipeline/Sidecar.swift` | `oni-settings/src/sidecar.rs` | partial | Read; not written on first open | `oni-settings/tests/swift_checks.rs::the_frames_exif_rides_its_sidecar_and_an_empty_box_is_absent` |
| Roll marker `.oni-roll.json` | `Pipeline/Folders.swift:8` | `oni-library/src/folders.rs:15` | done | | `oni-settings/tests/real_files.rs::the_roll_8_marker_reads_and_writes_back` |
| Library index schema 9, rebuilt whole | `Pipeline/LibraryIndex.swift:7` | `oni-library/src/index.rs:9` | done | Crate only | `oni-library/src/index.rs::a_schema_of_another_version_is_thrown_away` |
| Journal records: 16 bytes, magic ONIW, epoch 2020-01-02 | `Pipeline/Journal.swift:24-29` | `oni-library/src/journal.rs:56-62` | done | Crate only | `oni-library/src/journal.rs::three_gestures_read_back_whole_and_a_torn_or_foreign_file_does_not` |
| Project file and untitled stems | `Pipeline/Project.swift:25`, `Film.swift:144` | `oni-library/src/project.rs:42` | done | Crate only | `oni-library/src/project.rs::identity_is_the_id_and_the_free_name_the_lowest_number` |
| Export settings preference | `Pipeline/Export.swift:428` | `oni-export/src/settings.rs:108` | done | Crate only | `oni-export/tests/settings.rs::an_elder_preference_reads_its_defaults` |
| Preferences: welcome, what's new, navigator, opened folder | `UI/WelcomeSheet.swift:7`, `WhatsNew.swift:2`, `FolderNavigator.swift:15-22` | none | missing | | no check |

## Pipeline branches and the Swift comparison

The only test comparing a Rust render with a Swift render is
`oni-pipeline/tests/roll8.rs::roll8_lands_on_the_swift_render` (two frames by default, every frame with
`ONI_FULL_ROLL=1`, mean 2e-3, p99 1e-2), plus `oni-ui-poc/tests/reference.rs` on one frame. Both start
from `per_pixel_only()`, which zeroes sharpening, turns chroma denoise off and rests texture. Every
other numeric test compares a GPU kernel with its own CPU reference, or a crate with lcms2, never with
Swift. "Compared" below means a roll #8 sidecar moves that branch away from rest inside the Swift test.

| Branch | Swift | Rust | Compared to Swift | GPU vs CPU check |
|---|---|---|---|---|
| Negative mode, Pro face | `Pipeline/Pipeline.swift` | `oni-pipeline/src/lib.rs:91` | yes | `oni-render/tests/kernels.rs::stage47_on_roll8` |
| Scanner face, Frontier and ONI machines, clicks | `Pipeline/ScannerRender.swift` | `oni-pipeline/src/scanner.rs` | yes | `oni-render/tests/kernels.rs::scanner` |
| Roll reference, gains, capture stops | `Pipeline/RollReference.swift`, `CaptureExposure.swift` | `oni-pipeline` | yes | `oni-render/tests/roll8.rs::every_frame_of_roll8` |
| Print exposure, highlights, white and black points | `Pipeline/Print.swift` | `oni-settings/src/tone.rs` | yes | `oni-render/tests/stage47.rs::measured` |
| Contrast (0 and 0.40) | `Pipeline/Contrast.swift` | `oni-settings/src/tone.rs` | yes | `oni-render/tests/stage47.rs::measured` |
| Temperature and Tint filters | `Pipeline/Print.swift:169` | `oni-settings/src/tone.rs` | yes | `oni-render/tests/stage47.rs::measured` |
| Colour density (small values) | `Pipeline/ColorDensity.swift` | `conversion.rs:91` | yes | `oni-render/tests/stage47.rs::measured` |
| Chroma decompression 0.5 | `Pipeline/ChromaDecompression.swift` | `conversion.rs:104` | yes | `oni-render/tests/stage47.rs::measured` |
| Kodak C-41 separation at 0.6 | `Pipeline/Separation.swift` | `conversion.rs:36` | yes | `oni-render/tests/stage47.rs::measured` |
| Scanner Finish contrast | `Pipeline/ScannerFinish.swift` | `oni-pipeline` | yes | `oni-render/tests/kernels.rs::scanner` |
| Lighting shadows | `Pipeline/Print.swift:72` | `oni-settings/src/tone.rs` | no | `oni-render/tests/stage47.rs::measured` |
| Other dye families: Fuji, generic, ECN-2, E-6 | `Pipeline/Separation.swift:48` | `conversion.rs:36` | no | `oni-pipeline/tests/stages.rs::every_family_renders_finite_and_a_region_is_the_cut_of_the_whole` |
| Positive (slide) mode | `Pipeline/ConversionMode.swift` | `oni-pipeline` | no | `oni-app/src/render.rs::the_engine_inverts_a_negative_and_keeps_a_positive_in_order` |
| Monochrome mode, its ranges | `Pipeline/Print.swift:195-196` | `oni-pipeline` | no | `oni-pipeline/src/conversion.rs::a_monochrome_frame_carries_no_hunt_density_or_unmix` |
| Balance strength off its rest | `Pipeline/FilterBalance.swift` | `oni-core/src/balance.rs` | no | none |
| Density ceiling off the mosaic rest | `Pipeline/DensityAxis.swift` | `oni-settings/src/axis.rs` | no | `oni-render/tests/stage47.rs::measured` |
| Manual curves | `Pipeline/Curves.swift` | `oni-pipeline/src/finishing.rs:12` | no | `oni-render/tests/kernels.rs::curves` |
| Spectrogram | `Pipeline/Spectrogram.swift` | `finishing.rs:118` | no | `oni-render/tests/kernels.rs::spectrogram` |
| Saturation curve | `Pipeline/SaturationCurve.swift` | `finishing.rs:39` | no | `oni-render/tests/kernels.rs::saturation_curve` |
| Chrome (secret sauce) | `Pipeline/SaturationCurve.swift:82` | `finishing.rs:59` | no | `oni-render/tests/kernels.rs::saturation_curve` |
| Vignette, Scanner Finish vignette and chrome | `Pipeline/Vignette.swift`, `ScannerFinish.swift` | `oni-pipeline` | no | `oni-render/tests/kernels.rs::vignette` |
| Texture | `Pipeline/Texture.swift` | `finishing.rs:150` | no | `oni-render/tests/kernels.rs::texture` |
| Sharpening | `Pipeline/Sharpen.swift` | `finishing.rs:134` | no | `oni-render/tests/kernels.rs::detail` |
| Chroma denoise | `Pipeline/ChromaDenoise.swift` | `finishing.rs:134` | no | `oni-render/tests/kernels.rs::detail` |
| Flat-field (shipped mask) | `Pipeline/FlatField.swift` | `oni-pipeline/src/flat_field.rs` | no | `oni-render/tests/kernels.rs::flat_field` |
| Dust corrections | `Pipeline/Corrections.swift` | `oni-pipeline/src/corrections.rs` | no | `oni-render/tests/kernels.rs::correction_search` |
| Geometry: crop, quarter turns, mirror, angle | `Pipeline/Geometry.swift` | `oni-pipeline/src/geometry.rs` | no | `oni-pipeline/tests/stages.rs::rotation_turns_and_strokes_match` |
| RGB passes | `Pipeline/RGBPasses.swift` | `oni-pipeline` | no | `oni-render/tests/kernels.rs::rgb_passes` |
| Linear TIFF and DNG decode | `Pipeline/ContainerRead.swift` | `oni-decode/src/container.rs` | no | none |
| Source encoding | `Pipeline/SourceEncoding.swift` | none | no | none |
| Paper correction | `Pipeline/PaperCorrection.swift` | `oni-colour/src/paper.rs` | no | none |
| Contact-sheet ground | `Pipeline/ContactSheet.swift` | none | no | none |
| Proof, ICC export legs | `Pipeline/Proof.swift`, `OutputProfile.swift` | `oni-colour` | no | lcms2 oracle in `oni-colour/tests/oracle*.rs` |

Adversarial settings for a second Swift dump, one frame each, would reach every "no" row that has a
Rust render path: extreme shadows, each dye family, a slide and a monochrome frame, balance strength 0
and 1, a manual curve with overshoot, every spectrogram band, a saturation curve, Chrome 1.5, vignette
±2, texture ±, sharpening and chroma at their widest, a flat-field, two corrections, a 3° crop with a
quarter turn and a mirror, and an RGB triplet.
