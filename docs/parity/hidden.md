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
| Points reach into each window; bypassed print leaves them | `Pipeline/Print.swift:241` | `oni-pipeline/src/conversion/points.rs` | done | | `oni-pipeline/src/conversion/points.rs::the_points_reach_into_each_window_and_a_bypassed_print_leaves_them` |
| Print tone: toe, push (0.35, 0.15), quarters, contrast pivot 0.4587, steepness 15.2 | `Pipeline/Print.swift:47-82,190`, `Contrast.swift:8-12` | `oni-settings/src/tone.rs:14-15,117-118,279` | done | | `oni-settings/src/tone.rs::contrast_is_monotone_pinned_and_inverted_by_its_opposite` |
| Resting exposure 1.5 negative, 0 slide | `Pipeline/Print.swift:224-225` | `oni-settings/src/tone.rs:288` | done | | `oni-settings/tests/codec_checks.rs::the_rest_is_the_import_state_of_each_mode` |
| Temperature and Tint axes `(½, 0, −½)`, `(⅓, −⅔, ⅓)` | `Pipeline/Print.swift:176-177` | `oni-settings/src/tone.rs:269-270` | done | | `oni-core/src/knob.rs::temperature_and_tint_move_on_axes_that_leave_each_other_alone` |
| Colour filters reach no gain | `Pipeline/Curves.swift` `colourFilterChecks` | `oni-core/src/parameters.rs` | done | | `oni-core/src/parameters.rs::lighting_leaves_the_filters_and_color_carries_them` |
| Dye separation: 3×3 on balanced densities, rows sum to one, five families | `Pipeline/Separation.swift:41-151` | `oni-pipeline/src/conversion/separation.rs` | done | Rows literal, not derived from the 55 nm band (`validatedStrength 0.407` unchecked) | `oni-pipeline/src/conversion/separation.rs::the_separation_keeps_greys_and_the_mean_saturation` |
| Separation chroma kept | `Pipeline/Separation.swift:109-119` | `oni-pipeline/src/conversion/separation.rs` `chroma_kept` | done | | `oni-pipeline/src/conversion/separation.rs::the_separation_keeps_greys_and_the_mean_saturation` |
| Chroma decompression: colour follows the windows by its dose (0.5) | `Pipeline/ChromaDecompression.swift:9-54` | `oni-pipeline/src/conversion.rs:104-119` | done | | `oni-pipeline/tests/roll8.rs::roll8_lands_on_the_swift_render` |
| Color density factor `2^travel` | `Pipeline/ColorDensity.swift:6-17` | `oni-settings/src/tone.rs:49-51`, `conversion.rs:91` | done | | `oni-render/tests/stage47.rs::measured` |
| Gains (stage 4) opposite to the Lighting filters in negative | `Pipeline/Gains.swift:7-9` | `oni-settings/src/blocks.rs:15` | done | | `oni-pipeline/tests/roll8.rs::roll8_lands_on_the_swift_render` |
| Output clip the only compression, hard | `Pipeline/Pipeline.swift` | `oni-render` output clip | done | | `oni-render/tests/kernels.rs::output_clip` |
| Flat-field before the crop, colour reach 0.5 | `Pipeline/FlatField.swift:40-118` | `oni-pipeline/src/flat_field.rs:11-65` | done | Shipped mask only | `oni-pipeline/src/flat_field.rs::the_colour_sliders_turn_round_with_the_mode` |
| Flat-field sliders remembered per Mac | `Pipeline/FlatField.swift:54-66`, `Scene/EditorScene+Effects.swift:52-55`, `Scene/EditorScene+Actions.swift:72` | `oni-app/src/flat_fields.rs` `remember`, `with_remembered`; `app/masks.rs` `remember_flat_field`; `oni-core/src/store.rs` `open_adjusted` | done | `flatFieldSliders` in `preferences.json`; kept on a hand's edit, never on undo or redo; a frame whose sliders were never moved opens on the colour kept, its Amount whole, the history starting there | `oni-app/src/flat_fields.rs::a_fresh_frame_opens_on_the_colour_last_dialled_and_a_dialled_one_keeps_its_own`, `oni-core/src/store.rs::an_adjusted_opening_starts_the_history_on_the_adjusted_settings`, `oni-app/tests/masks.rs::the_mask_library_loads_renames_shows_remembers_and_moves_a_photo_out_as_swift_does` |
| Custom flat-field masks library | `Pipeline/CustomFlatFields.swift`, `Pipeline/Library.swift` `moveOut` | `oni-app/src/flat_fields.rs`, `render.rs` `preview_pixels`, `oni-export/src/tiff.rs` `encoded_half` | done | Decoded at the preview's side through the photographs' path (LibRaw for a RAW), kept as a half-float TIFF in working light with no metadata, id the first 8 bytes of the SHA-256 plus a triplet's pass names; rename, forget, `isMaskSource` (read by New Contact Sheet); `moveOut` renames, or copies then deletes across volumes | `oni-app/src/flat_fields.rs::a_loaded_picture_joins_the_list_under_its_files_name_once`, `::a_triplets_lead_is_a_mask_of_passes_under_an_id_of_its_own`, `::the_mask_source_is_known_by_its_name_then_its_content`, `::a_photo_moved_out_leaves_its_sidecar_and_index_row_and_takes_a_free_name`, `::the_id_is_the_first_eight_bytes_of_the_contents_sha_256`, `oni-app/src/render.rs::a_custom_flat_field_reaches_the_render_by_its_id` |
| Capture exposure evened in stops | `Pipeline/CaptureExposure.swift` | `oni-library/src/capture.rs`, `oni-app/src/app/prepare.rs` `measure_roll` | done | Read off the EXIF, matched to Swift's sidecars and written by Prepare the Roll's measure | `oni-library/tests/roll8.rs::roll8_capture_stops_read_off_the_exif_are_those_swift_wrote` |
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
| Scanner Finish apart from Pro's | `Pipeline/ScannerFinish.swift:25`, `OpenNegativeApp.swift:95-99` | `oni-settings/src/settings.rs:164`, `oni-core/src/store.rs` `face_view`, `Command::Finish` | done | Knob and Finish commands go through the open face's view | `oni-core/tests/scanner.rs::a_gesture_on_one_face_never_moves_the_other` |
| Paper correction gains, probe 0.18 | `Pipeline/PaperCorrection.swift:9-31` | `oni-settings/src/blocks.rs:172-175` | done | | `oni-settings/src/blocks.rs::the_paper_gains_are_powers_of_two_and_neutral_moves_nothing` |
| Screen gamut tolerance 0.01, period 9, duty 0.34 | `Pipeline/ScreenGamut.swift:14-29` | `oni-colour/src/gamut.rs:11-29` | done | | `oni-colour/tests/checks.rs::the_zebra_stripes_what_the_screen_clips` |
| Proof cube 33 in Rec. 2020 | `Pipeline/Proof.swift:53-54` | `oni-colour/src/proof.rs:37` | done | | `oni-colour/tests/checks.rs::the_proof_clips_what_the_export_space_cannot_hold` |
| Gain map lift 4, subsampling 2 | `Pipeline/GainMapContainer.swift:10-12` | `oni-export/src/gain_map.rs:8-10` | done | | `oni-export/tests/files.rs::the_container_says_two_stops_in_both_families` |
| Export sample 500 px, preview 1600 px | `Pipeline/Export.swift:8-12` | `oni-export/src/lib.rs:24-27` | done | The preview side has no reader outside the export card | `oni-export/tests/roll8.rs::the_estimate_holds_on_roll_8` (the sample side) |
| Geometry bleed margin 1, angle ±8 | `Pipeline/Geometry.swift:26,401` | `oni-pipeline/src/geometry.rs:11`, `oni-settings/src/frame.rs:74` | done | | `oni-pipeline/src/geometry.rs::the_rotation_keeps_the_centre_and_covers_its_bounding_box` |
| Raw texture side 16 384 | `Pipeline/RawDecode.swift:181` | `oni-graph/src/graph.rs:16` | done | | `oni-graph/tests/graph.rs::a_tiled_source_equals_an_untiled_one` |
| Decode pinned 5000 K, tint 0 | `Pipeline/RawDecode.swift:14-26` | `oni-decode/src/lib.rs:46`, `csrc/pinned.c` | done | LibRaw by design; `CIRAWFilter` knobs (noise 1, sharpness 0, shadow bias 0, version 8) have no LibRaw twin | `oni-decode/src/lib.rs::the_5000_k_white_reads_neutral_at_the_baseline`, `oni-decode/tests/swift_checks.rs::nothing_the_pinned_decode_hands_out_piles_at_the_resting_ceiling` |
| An 8-bit source is flagged, never refused (`isLowPrecision`) | `Pipeline/RawDecode.swift:400`, `ContainerRead.swift:198` | `oni-app/src/editors/shell.rs`, `export_card.rs`, `export.rs` | done |  | `oni-app/tests/editors.rs::a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each`, `oni-app/src/export.rs::the_summary_reads_as_the_panel_reads` |
| Sort decodes on the bytes, not the extension | `Pipeline/RawDecode.swift` | `oni-decode/src/container.rs` | done | | `oni-decode/src/container.rs::the_bytes_decide_the_branch` |
| Undeclared file read as the hand says (Source Encoding) | `Pipeline/SourceEncoding.swift` | `oni-colour/src/source.rs`, `oni-decode/src/plain.rs` | done | | `oni-colour/tests/swift_checks.rs::an_undeclared_file_is_read_as_the_hand_says_and_a_profile_as_it_says` |
| DNG container read: illuminants 17, 20, 21, 23 | `Pipeline/ContainerRead.swift:388` | `oni-colour/src/source.rs` `calibrated_to_working`, `oni-app/src/render.rs` | done |  | `oni-app/tests/extended.rs::a_decode_outside_libraw_is_swift_s`, `oni-colour/src/source.rs::a_dng_matrix_converts_its_own_white_to_the_working_white` |
| RGB passes read side 360, registration floor 0.3, 3 frames | `Pipeline/RGBPasses.swift:119`, `RGBRegistration.swift:30-35` | `oni-library/src/rgb_passes.rs` `READ_SIDE`, `oni-measure/src/registration.rs` `FLOOR`, `SHIFT_SIDE`, `MAGNIFICATION_FRAMES` | done | | `oni-measure/src/registration.rs::a_pass_s_magnification_turn_and_shift_read_back_off_its_edges`, `oni-app/tests/rgb_passes.rs::a_roll_of_passes_is_grouped_by_threes_ordered_by_its_light_and_registered_as_measured` |

## Measurement behind the automatic keys

| Behaviour | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Auto White Balance: Minkowski p = 6 at 480 px, greyed with Temperature and Tint | `Pipeline/FilterBalance.swift:15-22` | `oni-measure/src/filter_balance.rs`, `oni-measure/src/key.rs:57` | done | | `oni-measure/tests/filter_balance.rs::the_press_lands_the_three_means_together_through_the_kernel_and_the_dose_writes_its_share`, `oni-measure/tests/roll8.rs::roll8_auto_white_balance_lands_on_the_filters_swift_wrote` |
| Outcome dosed by Balance Strength 0.67 | `Pipeline/FilterBalance.swift:21-22` | `oni-core/src/balance.rs:88` | done | | `oni-core/src/balance.rs::the_outcome_is_dosed_by_balance_strength` |
| A selection's average gives every frame the mean | `Pipeline/AutomaticKey.swift` | `oni-core/src/balance.rs:98` | done | | `oni-core/src/balance.rs::average_gives_every_frame_the_mean_and_an_unreadable_frame_stops_the_run` |
| Measuring blocks: 933/48 px cells, tail 0.08, named cells, speck dropped | `Pipeline/Blocks.swift:10-146` | `oni-measure/src/blocks.rs` | done | | `oni-measure/tests/blocks.rs::a_speck_never_votes_and_an_area_places_the_end_on_its_own_value`, `::the_grid_names_its_cells_as_a_sheet_is_read` |
| Classic placement of the ends in two steps on blocks | `Pipeline/AutoLevels.swift:268-299` | `oni-measure/src/auto_levels.rs` `two_steps`, `oni-measure/src/key.rs:34` | done | | `oni-measure/tests/auto_levels.rs::two_steps_recover_a_channel_piled_on_the_window_s_top_and_change_nothing_on_a_sane_one`, `oni-measure/tests/roll8.rs::roll8_a_second_press_finds_nothing_and_a_paste_places_ends_on_held_gains` |
| Auto levels: threshold 0.001, quantiles, recentred, minimum span 0.02 | `Pipeline/AutoLevels.swift:10-195` | `oni-measure/src/auto_levels.rs` | done | | `oni-measure/tests/auto_levels.rs::the_threshold_skips_isolated_pixels_and_bites_when_raised`, `::the_bound_holds_recentring_to_the_room_between_the_tail_and_the_window` |
| Pipette: radius 4 (2…24), density space, mean | `Pipeline/Pipette.swift:7-51` | `oni-measure/src/pipette.rs`, `oni-measure/src/key.rs:107`, `oni-app/src/app/prepare.rs` `base_clicked` | done | Swift's only caller is Prepare the Roll's base pick, a click on the unturned scan; its editor has no canvas pipette | `oni-measure/tests/pipette.rs::the_ring_the_hand_aims_with_is_the_sample_at_every_radius`, `oni-measure/tests/roll8.rs::roll8_the_pipette_reads_before_the_levels_so_a_second_pick_cannot_answer_the_first`, `oni-app/tests/prepare.rs::a_roll_is_prepared_left_resumed_from_its_draft_and_measured_as_one_gallery_step` |
| Measuring window: full window, inset 0.1, proposed for a sheet | `Pipeline/Geometry.swift:110-143` | `oni-settings/src/geometry.rs`, `oni-app/src/framing.rs`, `ui/panel.slint` | done |  | `oni-settings/src/geometry.rs::the_measuring_window_reads_inside_the_crop`, `oni-app/src/framing.rs::the_window_is_drawn_inside_the_crop_outside_framing` |
| Measure side 1400, slot measure 640 | `Negative.swift:50`, `UI/MetalImageView.swift:204` | `oni-measure/src/reader.rs:16`, `oni-measure/src/key.rs:141` | partial | The keys read a 1400 px copy of a 2000 px preview; the slot's 640 is not ported | no check |
| Roll reference: factor 0.25…4, base radius, outvotes one frame | `Pipeline/RollReference.swift:22-27` | `oni-settings/src/frame.rs:238`, `oni-measure/src/roll.rs` | done | | `oni-measure/tests/roll.rs::one_leaked_frame_of_four_is_outvoted_and_two_frames_drop_nothing`, `oni-measure/tests/roll8.rs::roll8_reference_lands_on_the_roll_swift_measured` |
| Automatic keys write only what they show, one ⌘Z | hard rule 1 | `oni-core/src/store.rs:635` | done | Core level | `oni-core/tests/roll8.rs::the_balance_key_writes_only_on_a_press_and_one_undo_takes_it_back` |

## Interaction rules

| Behaviour | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Slider drag relative; Option fine ×0.2 | `DesignSystem/DSHandle.swift:87-90` | `oni-app/src/track.rs:8,42` | done | | `oni-app/src/track.rs::a_drag_is_relative_and_option_slows_it` |
| Slider values reach the settings 30 times a second | `DesignSystem/DSSlider.swift:150` | `oni-app/src/track.rs:10` | done | | `oni-app/src/track.rs::values_reach_the_settings_thirty_times_a_second` |
| Double-click resets a slider to its base | `DesignSystem/DSSlider.swift:101-103` | `app.rs` press, `system.rs` `double_click`, `fields.rs` `base` | done |  | `oni-app/tests/headless.rs::the_window_opens_roll_8_and_a_slider_drives_the_canvas`, `oni-app/src/system.rs::the_double_click_is_the_systems_and_never_zero`, `oni-app/src/fields.rs::a_double_click_on_the_ceiling_lands_on_the_frames_own_rest` |
| Bent tracks (centred) | `DesignSystem/DSSlider.swift:180` | `oni-app/src/track.rs:17-31` | done | | `oni-app/src/track.rs::a_bent_track_is_its_own_inverse` |
| Scroll over a slider: wheel line 10, trackpad travel 0.25, idle 0.16 s | `DesignSystem/DSScrollAdjust.swift:11-24` | `oni-app/src/track.rs` `scroll_travel`, `oni-app/src/app/scroll.rs` | done | The wheel's lines come from the window hook; without it Slint's points stand for a touch surface. The hand is held until 0.16 s pass without an event | `oni-app/src/track.rs::a_wheel_line_travels_ten_points_and_a_trackpad_point_a_quarter`, `::the_drag_is_not_damped_and_a_notch_is_a_few_percent_of_the_track`, `oni-app/tests/editor_leftovers.rs::the_editors_last_gestures_behave_as_swifts` |
| Nested swipe arbitration, bias 1.5 | `DesignSystem/DSScrollAdjust.swift:130` | none | missing | | no check |
| Spring on outside changes, none under the hand | `DesignSystem/DSSlider.swift:91`, `DesignSystem/DSHandle.swift:171` | `oni-app/src/motion.rs`, `oni-app/src/editors/sprung.rs` | done | The levels handles and the band sliders too: none under the hand, none while scrolled, none on a photo opened | `oni-app/src/motion.rs::settle_overshoots_slightly_and_travel_not_at_all`, `oni-app/src/editors/sprung.rs::a_value_from_elsewhere_springs_and_one_under_the_hand_is_placed`, `::a_new_target_mid_flight_springs_on_from_where_the_knob_shows`, `oni-app/tests/editors.rs::a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each` |
| Deferred refresh follows the hand (stale after 1 s) | `DesignSystem/DSGesture.swift:19` | `oni-app/src/track.rs:69-94` | done | | `oni-app/src/track.rs::values_reach_the_settings_thirty_times_a_second` |
| A burst of notches renders the first and last | `Scene/EditorScene+Stages.swift` | `oni-app/src/render.rs` | done | | `oni-app/src/render.rs::a_burst_of_notches_renders_the_first_and_the_last_only` |
| Levels: place, single write path, per-handle reset, grips front to back | `UI/LevelsView.swift:162-240` | `oni-app/src/editors/levels.rs` | done | | `oni-app/src/editors/levels.rs::a_handle_moves_through_one_path_and_resets_alone`, `::a_press_catches_the_front_handle_and_a_scroll_the_nearest`, `oni-app/src/editors/tests.rs::a_scroll_along_the_track_moves_the_nearest_handle_and_one_across_is_left_to_the_panel` |
| Levels anchor clamp 0.02…0.98, display shadows and highlights | `Pipeline/Levels.swift:192` | `oni-settings/src/levels.rs` `ANCHOR_CLAMP`, `display_*`, `set_display_*` | done | | `oni-app/src/editors/levels.rs::a_handle_moves_through_one_path_and_resets_alone` |
| Levels black reachable under zero | `UI/LevelsView.swift`, `Checks/UI/LevelsView+Checks.swift:7` | `oni-settings/src/levels.rs` `domain` | done | | `oni-app/src/editors/levels.rs::the_levels_track_holds_and_its_black_reaches_under_zero` |
| Curve: drag adds a point outside a 14 pt radius, double-tap removes | `UI/CurveEditorView.swift:67-100` | `oni-app/src/editors/curve.rs`, `oni-app/src/editors/mod.rs` | done | | `oni-app/src/editors/curve.rs::a_press_near_a_point_takes_it_to_the_pointer_and_elsewhere_adds_one`, `oni-app/src/editors/tests.rs::a_double_click_on_a_curve_point_removes_it_and_a_slow_one_does_not` |
| Selection is focus: a press anywhere off the wheel releases it | `DesignSystem/DSKeyboard.swift:27-30, 110-160`, `UI/SpectrogramPanel.swift:64-66` | `oni-app/src/editors/mod.rs` `Gestures::pressed_anywhere`, `oni-app/src/main.rs` | done | The hook sees the press before the window does and lets it through, as `DSOutsideClick` does | `oni-app/src/editors/tests.rs::a_press_off_the_wheel_releases_its_focus_and_one_on_it_keeps_it`, `oni-app/tests/editors.rs::a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each` |
| A swatch tab names its band in the tooltip | `DesignSystem/DSTabBar.swift:81` | `oni-ui/ui/ds/controls.slint` `Tabs` | done | | `oni-app/tests/editors.rs::a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each` |
| Curve points: margin 0.001, endpoints undeletable, clamped | `Pipeline/Curves.swift:16-33` | `oni-settings/src/curves.rs` | done | | `oni-settings/src/curves.rs::a_curve_point_keeps_its_margin_its_square_and_its_ends` |
| Saturation curve: double-tap to neutral, closest along | `UI/SaturationCurveView.swift:55-103` | `oni-app/src/editors/saturation.rs` | done | | `oni-app/src/editors/saturation.rs::a_double_click_neutralises_from_before_its_first_press`, `::a_press_takes_the_point_under_it_else_the_closest_along_the_light` |
| Spectrogram wheel grabbed, never jumped; step 0.02 | `UI/SpectrogramPanel.swift:217-245`, `Pipeline/Spectrogram.swift:86` | `oni-app/src/editors/spectrogram.rs` | done | | `oni-app/src/editors/spectrogram.rs::the_colour_wheel_is_grabbed_never_jumped_to`, `::an_arrow_steps_two_percent_of_the_axis_and_tab_walks_the_wheel` |
| Spectrogram luma nudge rest 0.5, reach 0.25; Option | `UI/SpectrogramPanel.swift:81,113-114` | `oni-app/src/editors/spectrogram.rs` | done | | `oni-app/src/editors/spectrogram.rs::the_luma_point_reads_along_its_spoke` |
| Bipolar gain rows name the end the hand reaches | `UI/GainPanel.swift` | `oni-ui/ui/panel.slint` Source Balance and Paper Correction | done |  | oni-app/tests/roll_tab.rs::the_roll_tabs_blocks_write_what_swifts_write |
| Temperature and Tint tracks end on their colours | `UI/TemperatureTintPanel.swift:23-24` | `oni-ui/ui/panel.slint` | done |  | no check |
| Canvas zoom per wheel notch 1.10, per trackpad point 1.0025 | `UI/CanvasNavigation.swift:8-22` | `oni-app/src/viewport.rs` | done |  | `oni-app/src/viewport.rs::the_wheel_zooms_by_notches_and_a_trackpad_by_points` |
| Viewport range 0.02…4, settle 150 ms, region margin 0.3 | `UI/Viewport.swift:13-131` | `oni-app/src/viewport.rs` | done |  | `oni-app/src/viewport.rs::full_resolution_is_asked_for_at_a_hundred_and_beyond_never_wide_out`, `oni-app/src/viewport.rs::the_ladder_steps_strictly_and_fit_is_its_floor` |
| Crop handles: corners, edges, min crop 0.05, ratio held | `UI/CropOverlay.swift:280-281`, `Geometry.swift:30` | `oni-app/src/framing.rs`, `oni-settings/src/geometry.rs` | done |  | `oni-app/src/framing.rs::a_corner_moves_its_two_sides_and_the_body_moves_all`, `oni-app/src/framing.rs::a_locked_ratio_keeps_the_corner_opposite_the_hand` |
| Straighten re-fits the crop inside the image | `Pipeline/Geometry.swift:195,234` | `oni-settings/src/geometry.rs` `straightened`, `app.rs` straighten | done |  | `oni-settings/src/geometry.rs::straightening_refits_the_frame_inside_the_turned_photo` |
| Rotate keeps the crop, mirror flips it | `Pipeline/Geometry.swift:39-101` | `oni-settings/src/geometry.rs`, `app.rs` | done |  | `oni-settings/src/geometry.rs::a_mirror_keeps_the_crop_on_its_content`, `oni-settings/src/geometry.rs::four_quarter_turns_return_the_frame_and_one_swaps_its_sides` |
| Framing paste adopts, fits inside the image | `Pipeline/Geometry.swift:101,195` | `oni-core/src/store.rs`, `app.rs` `adopt` | done | Only the open frame is fitted, as `pasteFraming(to:)` does | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |
| Gallery ⌘ and ⇧ click pick as shown | `UI/ListPicking.swift` | `oni-app/src/picking.rs`, `oni-app/src/organise.rs` | done | | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-app/src/picking.rs::a_clicks_command_and_shift_pick_as_shown` |
| Select all, arrows walk the grid | `UI/GalleryEngine.swift:425-512` | `oni-app/src/organise.rs` `Picks::key` | done | | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-app/src/picking.rs::command_a_completes_the_groups_the_selection_touches_else_the_focus_else_the_last` |
| Column resizer for the filmstrip | `DesignSystem/DSPanel` `DSColumnResizer` | none | missing | | no check |
| A chip across slots lands in the nearest (DSSlots) | `DesignSystem/DSReorder.swift` | none | missing | | no check |
| Pager: anchoring, clicked travel, swipe end of course | `DesignSystem/DSPanel.swift` | none | missing | | no check |
| Help bubbles fit where they hang | `DesignSystem/DSHelp.swift` | `oni-ui/ui/ds/base.slint` `Tips.place`, `TipLayer` | partial | Held Space.sm in from the window's four edges and cut to it; Swift holds it in the frame its layer claims and hangs it on the row's corner, the port under the mark | `oni-app/tests/editor_leftovers.rs::the_editors_last_gestures_behave_as_swifts` |
| Hover fallback | `DesignSystem/DSHover.swift` | none | missing | | no check |
| Keycap press floor 140 ms before acting | `DesignSystem/Keycap` | `oni-ui/ui/ds/keypad.slint` `Key`, `ui/ds/theme.slint` `ScannerStyle.key-hold` | done | A tap holds the cap drawn down 0.14 s; the action fires at the release, as SwiftUI's button does | `oni-app/tests/keycap.rs::a_tap_holds_the_cap_down_its_floor_and_a_long_press_lifts_at_once` |
| Roll Settings gesture writes every frame on the reference, one step | `Scene/EditorScene+RollSettings.swift`, `RollSettings.swift:36` | `oni-core/src/store.rs:364` | done | | `oni-core/tests/roll8.rs::a_roll_settings_gesture_writes_every_frame_of_the_roll_as_one_step_and_undoes_byte_for_byte` |
| The bar's reset keeps geometry, flat-field, dye family | `Pipeline/Parameters.swift:257` | `oni-core/src/parameters.rs:337` | done | | `oni-core/src/parameters.rs::the_reset_keeps_geometry_flat_field_and_dye_family` |
| Block reset to the mode's rest | `Pipeline/Parameters.swift:246` `rest(for:)` | `oni-app/src/store.rs` `rest` | done | | `oni-app/src/store.rs::a_block_reset_lands_on_the_rest_of_the_frames_own_mode` |
| Dye Separation reset clears family and by-hand | `UI/SettingsPane.swift:74` | `oni-app/src/store.rs` `reset` | done | | `oni-app/src/store.rs::the_dye_separation_reset_clears_the_dose_the_family_and_the_hand` |
| Mode change carries a block onto the next mode | `Pipeline/Parameters.swift:286` | `oni-core/src/store.rs` `apply_mode`, `adapted` | done |  | `oni-core/tests/swift_checks.rs::a_mode_change_carries_the_handles_at_rest_and_leaves_a_placed_black`, `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |
| Reset to Default reaches whole rolls and sheets, never a neighbour | `Pipeline/RollSettings.swift:114` | `oni-library/src/roll_membership.rs`, `app/rolls.rs` | done |  | oni-app/tests/rolls.rs::the_roll_gestures_reach_what_swift_reaches, `oni-library/tests/swift_checks.rs::a_reset_to_default_reaches_whole_rolls_and_their_sheets_never_a_neighbour` |
| Sidecar write delay 400 ms | `Pipeline/Sidecar.swift:7` | `oni-app/src/app.rs` `WRITE_DELAY` | done | | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Thumbnail refresh 2 s, after the sidecar write | `Pipeline/Thumbnails.swift:21` | `oni-app/src/app.rs` `schedule_redraw`, `thumbnails.rs` `REFRESH_DELAY` | done | | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Histogram throttle 12 ms, 100 ms while held | `Pipeline/Histogram.swift:59-63` | `oni-app/src/render.rs` | partial | The histograms follow the render at the 1/30 s image cadence the performance targets set | no check |
| Histogram graduation overshoot 0.35 | `Pipeline/Histogram.swift:20` | `oni-settings/src/axis.rs:105` | done | | `oni-settings/src/axis.rs::the_graduation_reaches_under_the_lowest_density_a_test_film_holds` |
| Medal keys 0-3 bare, live only outside text | `UI/EditorCommands.swift:183` | `oni-app/src/app.rs:1172-1175` | done | | no check |
| A frame opens on the face it is opened in; the switch is one undo step | `Scene/EditorScene+Lab.swift:93-107`, `Scene/EditorScene+Actions.swift:83-95`, `Scene/EditorScene+Effects.swift:59-66` | `oni-core/src/store.rs` `open`, `Command::Face` | done | `History(opened)` starts on the stored face; `adoptFace` then changes it, and the deferred `record` keeps that as the first undo step, which undoes the face alone | `oni-core/tests/scanner.rs::the_face_is_one_step_and_a_frame_opens_on_the_face_it_is_opened_in` |
| The face last left is kept across launches (`lastFaceScanner`) | `OpenNegativeApp.swift:329`, `Scene/EditorScene+Removal.swift:613` | `oni-app/src/app/lab.rs` `remembered` | done | A file in the support folder | `oni-app/src/app/lab.rs::the_face_is_remembered_across_launches`, `oni-app/tests/scanner.rs::the_keypad_presses_the_phone_s_commands_and_the_lab_screen_reads_them_back` |

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
| Sidecar EXIF block | `Pipeline/Sidecar.swift` | `oni-settings/src/sidecar.rs`, `oni-app/src/export.rs` | done | Written on export, as Swift writes it | `oni-settings/tests/swift_checks.rs::the_frames_exif_rides_its_sidecar_and_an_empty_box_is_absent` |
| Roll marker `.oni-roll.json` | `Pipeline/Folders.swift:8` | `oni-library/src/folders.rs:15` | done | | `oni-settings/tests/real_files.rs::the_roll_8_marker_reads_and_writes_back` |
| Library index schema 9, rebuilt whole | `Pipeline/LibraryIndex.swift:7` | `oni-library/src/index.rs:9` | done | Crate only | `oni-library/src/index.rs::a_schema_of_another_version_is_thrown_away` |
| Journal records: 16 bytes, magic ONIW, epoch 2020-01-02 | `Pipeline/Journal.swift:24-29` | `oni-library/src/journal.rs:56-62` | done | Crate only | `oni-library/src/journal.rs::three_gestures_read_back_whole_and_a_torn_or_foreign_file_does_not` |
| Project file and untitled stems | `Pipeline/Project.swift:25`, `Film.swift:144` | `oni-library/src/project.rs:42` | done | Crate only | `oni-library/src/project.rs::identity_is_the_id_and_the_free_name_the_lowest_number` |
| Export settings preference | `Pipeline/Export.swift:428` | `oni-export/src/settings.rs:108` | done | Crate only | `oni-export/tests/settings.rs::an_elder_preference_reads_its_defaults` |
| Preferences: welcome, what's new, navigator, opened folder | `UI/WelcomeSheet.swift:7`, `WhatsNew.swift:2`, `FolderNavigator.swift:15-22` | `oni-app/src/preferences.rs`, `oni-app/src/app/navigator.rs` | done | `openedFolder`, `retiredShippedPresets`, `welcomeSeen`, `whatsNewSeen`, `galleryColumns`, `soundMuted` and the navigator's keys kept under Swift's names | `oni-app/src/preferences.rs::a_value_written_is_read_back_by_the_next_session`, `oni-app/tests/navigator.rs::the_navigator_walks_its_disk_from_computer_and_the_next_launch_finds_it_where_it_was_left`, `oni-app/tests/help.rs::the_help_menus_surfaces_behave_as_swifts` |

## Pipeline branches and the Swift comparison

Two tests compare a Rust render with a Swift render, both fed the Swift decode, mean 2e-3, p99 1e-2.
`oni-pipeline/tests/roll8.rs::roll8_lands_on_the_swift_render` runs roll #8's own sidecars (two frames
by default, every frame with `ONI_FULL_ROLL=1`), per pixel only. `oni-pipeline/tests/adversarial.rs`
runs the settings of `oni-pipeline/tests/adversarial/branches.json` on DSC_6440.NEF and P1072392.RW2,
each branch also asserted to move Swift's render off the frame at rest (p99 5e-3), so settings that
reach nothing fail. Its dumps come from `scripts/adversarial-dump.sh` into `ONI_ADVERSARIAL`: the app's
`--render` for per-pixel branches, and for sharpening, chroma denoise and texture a build of a copy of
the Swift sources whose `--render` keeps them under `ONI_RENDER_WHOLE`. The crop is quantised on
Swift's grid, ImageIO's width of the RAW, which LibRaw's need not match. "Compared" below means one of
these two tests moves that branch.

| Branch | Swift | Rust | Compared to Swift | GPU vs CPU check |
|---|---|---|---|---|
| Negative mode, Pro face | `Pipeline/Pipeline.swift` | `oni-pipeline/src/lib.rs:91` | yes | `oni-render/tests/kernels.rs::stage47_on_roll8` |
| Scanner face, Frontier and ONI machines, clicks | `Pipeline/ScannerRender.swift` | `oni-pipeline/src/scanner.rs` | yes | `oni-render/tests/kernels.rs::scanner` |
| Roll reference, gains, capture stops | `Pipeline/RollReference.swift`, `CaptureExposure.swift` | `oni-pipeline` | yes | `oni-render/tests/roll8.rs::every_frame_of_roll8` |
| Print exposure, highlights, white and black points | `Pipeline/Print.swift` | `oni-settings/src/tone.rs` | yes | `oni-render/tests/stage47.rs::measured` |
| Contrast (0 and 0.40) | `Pipeline/Contrast.swift` | `oni-settings/src/tone.rs` | yes | `oni-render/tests/stage47.rs::measured` |
| Temperature and Tint filters | `Pipeline/Print.swift:169` | `oni-settings/src/tone.rs` | yes | `oni-render/tests/stage47.rs::measured` |
| Colour density (small values) | `Pipeline/ColorDensity.swift` | `conversion.rs:91` | yes | `oni-render/tests/stage47.rs::measured` |
| Chroma decompression 0.5 | `Pipeline/ChromaDecompression.swift` | `conversion/decompression.rs` | yes | `oni-render/tests/stage47.rs::measured` |
| Kodak C-41 separation at 0.6 | `Pipeline/Separation.swift` | `conversion/separation.rs` | yes | `oni-render/tests/stage47.rs::measured` |
| Scanner Finish contrast | `Pipeline/ScannerFinish.swift` | `oni-pipeline` | yes | `oni-render/tests/kernels.rs::scanner` |
| Lighting shadows | `Pipeline/Print.swift:72` | `oni-settings/src/tone.rs` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/stage47.rs::measured` |
| Other dye families: Fuji, generic, ECN-2, E-6 | `Pipeline/Separation.swift:48` | `conversion/separation.rs` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-pipeline/tests/stages.rs::every_family_renders_finite_and_a_region_is_the_cut_of_the_whole` |
| Positive (slide) mode | `Pipeline/ConversionMode.swift` | `oni-pipeline` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-app/src/render.rs::the_engine_inverts_a_negative_and_keeps_a_positive_in_order` |
| Monochrome mode, its ranges | `Pipeline/Print.swift:195-196` | `oni-pipeline` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-pipeline/src/conversion.rs::a_monochrome_frame_carries_no_hunt_density_or_unmix` |
| Balance strength off its rest | `Pipeline/FilterBalance.swift` | `oni-core/src/balance.rs` | n/a: a dose written into the filters, which `print-channels` compares | none |
| Density ceiling off the mosaic rest | `Pipeline/DensityAxis.swift` | `oni-settings/src/axis.rs` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/stage47.rs::measured` |
| Manual curves | `Pipeline/Curves.swift` | `oni-pipeline/src/finishing.rs:12` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/kernels.rs::curves` |
| Spectrogram | `Pipeline/Spectrogram.swift` | `finishing.rs:118` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/kernels.rs::spectrogram` |
| Saturation curve | `Pipeline/SaturationCurve.swift` | `finishing.rs:39` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/kernels.rs::saturation_curve` |
| Chrome (secret sauce) | `Pipeline/SaturationCurve.swift:82` | `finishing.rs:59` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/kernels.rs::saturation_curve` |
| Vignette, Scanner Finish vignette and chrome | `Pipeline/Vignette.swift`, `ScannerFinish.swift` | `oni-pipeline` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/kernels.rs::vignette` |
| Texture | `Pipeline/Texture.swift` | `finishing.rs:150` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/kernels.rs::texture` |
| Sharpening | `Pipeline/Sharpen.swift` | `finishing.rs:134` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/kernels.rs::detail` |
| Chroma denoise | `Pipeline/ChromaDenoise.swift` | `finishing.rs:134` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/kernels.rs::detail` |
| Flat-field (shipped mask) | `Pipeline/FlatField.swift` | `oni-pipeline/src/flat_field.rs` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-render/tests/kernels.rs::flat_field` |
| Dust corrections | `Pipeline/Corrections.swift` | `oni-app/src/heal.rs`, `oni-pipeline/src/corrections.rs` | yes, `oni-app/tests/extended.rs` | `oni-render/tests/kernels.rs::correction_search` |
| Geometry: crop, quarter turns, mirror, angle | `Pipeline/Geometry.swift` | `oni-pipeline/src/geometry.rs` | yes, `oni-pipeline/tests/adversarial.rs` | `oni-pipeline/tests/stages.rs::rotation_turns_and_strokes_match` |
| RGB passes | `Pipeline/RGBPasses.swift` | `oni-app/src/render.rs` `composed` | yes, `oni-app/tests/extended.rs` | `oni-render/tests/kernels.rs::rgb_passes` |
| Linear TIFF and DNG decode | `Pipeline/ContainerRead.swift` | `oni-decode/src/container.rs`, `oni-colour/src/source.rs` | yes, `oni-app/tests/extended.rs` | `oni-decode/tests/swift_checks.rs::a_linear_scan_is_read_through_its_table_and_its_reduction_does_not_alias` |
| Source encoding | `Pipeline/SourceEncoding.swift` | `oni-colour/src/source.rs` | yes, `oni-app/tests/extended.rs` | `oni-colour/tests/swift_checks.rs::an_undeclared_file_is_read_as_the_hand_says_and_a_profile_as_it_says` |
| Paper correction | `Pipeline/PaperCorrection.swift` | `oni-colour/src/paper.rs` | n/a: laid only on an export into a paper profile | none |
| Contact-sheet ground | `Pipeline/ContactSheet.swift` | `oni-pipeline/src/stages.rs` `OnGround` | no | none |
| Proof, ICC export legs; export files | `Pipeline/Proof.swift`, `OutputProfile.swift`, `Export.swift` | `oni-colour`, `oni-export` | export: yes, `oni-app/tests/extended.rs` | lcms2 oracle in `oni-colour/tests/oracle*.rs` |

`oni-app/tests/extended.rs` reads `ONI_ADVERSARIAL/extended`, written by `scripts/extended-dump.sh`
through the probe build (`--probe-decode`, `--probe-heal`, `--probe-export`): decodes outside LibRaw
(a linear DNG held and converted, a 16-bit PNG held, converted and under each chosen encoding, three
registered RGB passes, plain TIFFs at the preview side), four corrections' patches generated on
Swift's own source, reduced, placed and feathered, two frames rendered with them, and fifteen
exports. What it found and the port now follows: `CIAreaAverage` averages the part of a ring inside
the image; `CILanczosScaleTransform` keeps a partial last pixel at the exact scale, anchored at the
bottom, clear past the edge; a stroke mask is CoreGraphics' grey coverage read at gamma 2.2; a clear
pixel stays clear to the file (premultiplied, associated alpha) and shows the canvas's ground; a
RAW's frame is the maker's crop, `CIRAWFilter`'s 6048 × 4032 of a D750's 6064 × 4040; TIFF writes
ImageIO's FillOrder, Orientation, ResolutionUnit, SampleFormat and Exif pixel size.

Open: ImageIO's thumbnail filter for a plain file's preview (Catmull-Rom nears it, 0.002 to 0.005);
`CILanczosScaleTransform` under an eighth (0.0014 on a patch); a one or two pixel stroke's
antialiasing; the EXIF defaults ImageIO adds to a JPEG, and its encoder's tables (quality 0.85 is
finer than libjpeg's 85); a reduced contact sheet, whose clear gutters Core Image writes white.
