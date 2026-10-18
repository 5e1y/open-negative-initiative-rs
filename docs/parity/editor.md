# Parity: the editor

Swift paths are under `Sources/OpenNegative/` of the Swift app; Rust paths are under `crates/`.
Statuses are defined in [README.md](README.md).

## Canvas

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Open frame rendered through the pipeline | `UI/MetalImageView.swift`, `Negative.swift:46` | `oni-app/src/render.rs`, `oni-graph` | done | 2000 px preview, as Swift's `previewSide` | `oni-app/tests/headless.rs::the_window_opens_roll_8_and_a_slider_drives_the_canvas` |
| Full-resolution region at zoom | `UI/Viewport.swift:138` `needsFullResolution` | `oni-app/src/render.rs` `Engine::resident`, `oni-app/src/viewport.rs` | done | Past the preview's ratio, once the view settles (150 ms), the frame is decoded again at full resolution and only the visible region renders; Swift keeps that region with a 30 % margin so a pan reuses it, the port renders the visible region again on each pan step | `oni-app/src/render.rs::a_view_renders_its_visible_region_from_the_source_its_scale_needs`, `oni-app/src/viewport.rs::full_resolution_is_asked_for_at_a_hundred_and_beyond_never_wide_out`, `ONI_BENCH=zoom` |
| Zoom: wheel, trackpad pinch, ⌘ scroll | `UI/CanvasNavigation.swift:8-38` | `oni-app/src/viewport.rs`, `app.rs` `wire_canvas`, `main.rs` wheel hook | partial | Slint drops the scroll phases: a precise (pixel) delta stands for a phased touch surface, a line delta for a wheel; pinch through Slint's `ScaleRotateGestureHandler`. Neither tried by hand on the Mac | `oni-app/src/viewport.rs::the_wheel_zooms_by_notches_and_a_trackpad_by_points`, `a_zoom_holds_the_pixel_under_the_pointer` |
| Zoom ladder, fit, 100 % toggle | `UI/Viewport.swift:27-116` | `oni-app/src/viewport.rs`, `app.rs` `step_zoom`, the canvas's double click | done | | `oni-app/src/viewport.rs::the_ladder_steps_strictly_and_fit_is_its_floor`, `a_double_click_lands_on_a_hundred_then_back_on_fit`, `ONI_BENCH=zoom` |
| Pan by drag and scroll | `UI/CanvasNavigation.swift:17`, `Viewport.swift:94` | `oni-app/src/viewport.rs`, `app.rs` `wire_canvas` | done | The offset is clamped to the overflow; a new frame keeps the scale and drops the pan | `oni-app/src/viewport.rs::the_placement_shows_the_part_of_the_image_inside_the_view`, `walking_the_roll_keeps_the_scale_and_drops_the_pan`, `ONI_BENCH=zoom` |
| Canvas proofed into the export space | `Scene/EditorScene+Stages.swift`, `Pipeline/Proof.swift` | `oni-colour/src/proof.rs` | partial | Proof exists in the crate; the app shows the working render on the display matrix | `oni-colour/tests/checks.rs::the_proof_clips_what_the_export_space_cannot_hold` (crate only) |
| Screen gamut stripes | `Pipeline/ScreenGamut.swift`, `UI/MetalImageView.swift` `stripes` | `oni-app/src/display.rs` (the display kernel), `app.rs` `toggle_screen_gamut`, `oni-colour/src/gamut.rs` constants | done | Laid in the display kernel over the render the canvas holds, nine drawable pixels a period whatever the zoom, the canvas encoded again thirty times a second while on; the range is the sRGB curve extended by the tolerance (Display P3 shares it), not read off the screen\'s own profile | `oni-app/src/display.rs::the_zebra_stripes_what_the_screen_cannot_show_as_its_reference_does` |
| Compare before/after | `Scene/EditorScene+Actions.swift` `.compare`, `UI/NegativeView.swift` `splitDivider`, `Pipeline.splitView` | `oni-app/src/app.rs` `toggle_compare`, `render.rs` `Job::reference`, `ui/ds/base.slint` `SplitGuide` | done | The before is the bar's reset frame with the dust kept (`imported` plus the corrections), rendered on the same view and kept while its settings and the view hold; the split starts at half, follows the guide, and a mode change drops it. The split's haptic ticker is not ported | `ONI_BENCH=compare` (one before over a 60-notch drag, the split moved to 0.80) |
| Dust patches on the canvas render | `Scene/EditorScene+Panel.swift` `ensureGenerated`, `Pipeline/Corrections.swift` `CorrectionRenderer.generate`, `CorrectionCache` | `oni-app/src/heal.rs`, `patches.rs`, `render.rs` `search_region`, `app.rs` `heal` | done | As a frame with strokes opens, each correction without a patch is generated in turn on the full-resolution frame framed with the others healed: the coarse then fine search on the CPU in Core Image's frame, the parabolic refine, the tone field; written to `Caches/…/Corrections/<id>.tiff` in single float, read and laid by the pipeline, the cache purged at quit. The fallback flag and the source markers are not shown | `oni-app/src/heal.rs::the_search_finds_the_place_the_surroundings_were_copied_from`, `a_bias_lifts_the_copy_to_its_surroundings_tone`, `oni-app/src/render.rs::a_dust_patch_from_the_cache_heals_its_stroke`, `oni-app/src/patches.rs::a_patch_is_found_under_its_corrections_id_as_swift_names_it` |
| Custom flat-field mask on the canvas | `Scene/EditorScene+FlatField.swift`, `Pipeline/CustomFlatFields.swift` | `oni-app/src/flat_fields.rs`, `render.rs` `Engine::read_custom` | done | Read from the support folder's `FlatFields/masks.json` and `<id>.tiff` as Swift saved them, once per id, its peaks the mask's maximum, its passes flag from the list; on the canvas and the thumbnails. The export (`export.rs`) still passes no mask, and loading a new mask (Load…, Use This Photo) is not ported | `oni-app/src/render.rs::a_custom_flat_field_reaches_the_render_by_its_id`, `oni-app/src/flat_fields.rs::the_swift_apps_own_masks_read_whole` |
| Crop overlay: corners, edges, rule of thirds | `UI/CropOverlay.swift` | `oni-app/ui/ds/frame.slint` `FrameOverlay`, `oni-app/src/framing.rs`, `app.rs` `frame_dragged` | done | The scrim outside, the thirds, the brackets and edge bars clamped by the frame shown, the body moving the frame; each move constrained about the corner opposite the hand and fitted under rotation, one undo step a gesture; the canvas shows the straightened frame uncropped while the Crop tab is open. Not ported: the live size badge, the off-view arrow, Esc cancelling a drag | `oni-app/src/framing.rs::a_corner_moves_its_two_sides_and_the_body_moves_all`, `a_locked_ratio_keeps_the_corner_opposite_the_hand`, `ONI_BENCH=crop` |
| Measuring window overlay | `UI/CropOverlay.swift` `MeasureWindowOverlay` | `ds/frame.slint` (window style), `framing.rs` `window_shown`, `bounded` | done | Drawn while armed, on the upright box while framing and inside the crop otherwise; its body moves only outside framing | `oni-app/src/framing.rs::the_window_is_drawn_inside_the_crop_outside_framing` |
| Dust brush cursor and strokes | `UI/NegativeView.swift` `paintingLayer`, `correctionMarks` | `oni-app/ui/ds/frame.slint` `BrushDisc`, `LiveStroke`, `CorrectionMark`, `SourceTrace`, `app.rs` `paint_*`, `publish_dedust` | done | The disc, the live stroke, each correction's ring and number, the line to its source and the copied stroke; the line is solid, not Swift's dash | `ONI_BENCH=dedust` |
| Canvas readouts (size, zoom, status) | `UI/NegativeView.swift` | `oni-app/src/app.rs` status line, the zoom bubble | done | The zoom in the bubble as `AppBar.zoomLabel`, the rendered size in the status line | `ONI_BENCH=zoom` |
| Neutral decode notice | `UI/NegativeView.swift` (NeutralDecodeNotice) | none | missing | | no check |
| Plain image (JPEG, PNG, RGB TIFF) opens | `Pipeline/RawDecode.swift` | `oni-app/src/render.rs` `decode` | missing | Refused with "not read yet" | no check |
| Linear scan with converted primaries opens | `Pipeline/ContainerRead.swift:388` | `oni-app/src/render.rs` | missing | Refused | no check |
| Ground behind the frame, contact-sheet ground | `UI/MetalImageView.swift:324-330` | none | missing | | no check |
| Splash while the first frame decodes | `UI/Splash.swift` | none | missing | | no check |

## Main band above the panel

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Histogram, morphing between renders | `Pipeline/Histogram.swift`, `DesignSystem/DSHistogram` | `oni-app/src/render.rs` histogram | partial | Counted per channel and drawn; no morph, no throttle (`Histogram.swift:59-63`) | `oni-app/src/render.rs::the_histogram_counts_each_channel_on_its_own` |
| Print curve drawn over the histogram (Color tab) | `UI/PrintPanel.swift:46-85`, `Scene/EditorScene+Panel.swift:35-44` | `oni-app/src/editors/print.rs`, `oni-app/ui/editors.slint` `PrintOverlay`, `oni-app/ui/panel.slint` | done | Moved channels in their tint, the RGB curve and its mirror dashed; none per channel in monochrome | `oni-app/src/editors/print.rs::the_print_draws_its_moved_channels_its_curve_and_the_mirror`, `::the_mirror_is_the_curve_with_its_axes_swapped` |
| Loupe on the Details tab | `UI/Loupe.swift` | none | missing | | no check |
| Lab screen on the Scanner face | `UI/Lab/` | none | missing | | no check |
| Preset deck (carousel) | `UI/PresetCarousel.swift` | none | missing | See [other.md](other.md) | no check |

## Panel: tabs

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Tabs Roll, Crop, Color, Details | `UI/PanelTab.swift` | `oni-app/ui/panel.slint` | done | | `oni-app/tests/catalogue.rs::the_catalogue_shows_every_component` |
| De-dust tab, hidden and modal | `UI/PanelTab.swift:15-17` | `panel.slint` `DedustPane` (pane 4, no tab), `app.rs` `toggle_correcting` | done | Entered and left by the bar's key and the menu; leaving lands on the panel the hand was in, never the crop | `ONI_BENCH=dedust` |
| Scanner face tabs (Lab keypad) | `Scene/EditorScene+Lab.swift` | none | missing | | no check |
| Block help bubbles (`help:`) | every `DSSection` | none | missing | | no check |
| Block reset key, shown off rest | `DSSection(resetting:)` | `oni-app/src/store.rs:94-108` | partial | Nine blocks; rest is always Negative's (`store.rs:92`) | `oni-app/src/store.rs::a_block_reset_lands_on_the_rest_in_one_write` |
| Blocks folded by default | `.folded(byDefault: true)` | none | missing | Every block is open | no check |
| Panel width resizer | `DesignSystem/DSPanel` `DSColumnResizer` | none | missing | | no check |
| Hide the Panel ⌥⌘] | `UI/EditorCommands.swift:150` | `oni-app/src/app.rs:1127` | partial | Menu works; the shortcut is not mapped | no check |

## Panel: Roll tab

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Block order | `UI/SettingsPane.swift:28-112` | `oni-app/ui/panel.slint:45-95` | partial | Swift: Measured Roll, Film Type, Flat-Field, Source Encoding, Source Balance, Density Ceiling, Per-Channel Levels, Dye Separation, Balance Strength. Rust puts Dye Separation and Balance Strength third and fourth | no check |
| Measured Roll: count note, warnings | `UI/SettingsPane.swift:114-140` | `panel.slint:45` | partial | Static text | no check |
| Prepare the Roll… key | `UI/SettingsPane.swift:117` | none | missing | | no check |
| Join the Roll / Join the N | `UI/SettingsPane.swift:126` | `oni-core/src/store.rs:680` `join_roll` | partial | Core only | no check |
| Roll of the N | `UI/SettingsPane.swift:129` | none | missing | | no check |
| Film Type tabs (negative, slide, B&W) | `UI/SettingsPane.swift:38` | `panel.slint:54`, `oni-core/src/store.rs:610` | partial | Picker unwired | no check |
| Flat-Field toggle, gear, mask picker | `UI/FlatFieldControls.swift` | `panel.slint:59-63` | partial | Toggle unbound | no check |
| Use This Photo as Flat-Field, Load…, Rename, Folder | `UI/FlatFieldControls.swift` | none | missing | | no check |
| Apply to Selection | `UI/FlatFieldControls.swift` | none | missing | | no check |
| Flat-field Amount, Temperature, Tint | `UI/FlatFieldControls.swift` | none | missing | Model fields exist | no check |
| Source Encoding (undeclared files) | `UI/SettingsPane.swift:53-66` | none | missing | Model field exists | no check |
| Source Balance (gains R, G, B) | `UI/SettingsPane.swift:136`, `UI/GainPanel.swift` | none | missing | | no check |
| Density Ceiling slider | `UI/SettingsPane.swift:146` | `panel.slint:87` field 12 | done | | `oni-app/src/fields.rs::each_slider_spans_its_knobs_range_on_a_mosaic_negative` |
| Per-Channel Levels: tabs and handles on the histogram | `UI/SettingsPane.swift:157-171`, `UI/LevelsView.swift` | `oni-app/src/editors/levels.rs`, `oni-app/src/editors/mod.rs`, `oni-app/ui/editors.slint` `LevelsEditor` | partial | R, G, B tabs lit off their rest, five handles, markers, readouts, reset to the roll's window. The histogram is `Plots.levels`, which nothing fills yet (Swift draws the levels input); no 8-bit source badge; not yet hidden on the Scanner face, which has no panel | `oni-app/src/editors/tests.rs::a_levels_handle_moves_by_the_pointer_s_travel_and_a_double_click_resets_it_alone`, `oni-app/tests/editors.rs::a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each` |
| Dye Separation: family tabs | `UI/SettingsPane.swift:73-84` | `panel.slint:70` | partial | Picker static | no check |
| Dye Separation dose slider | `UI/SettingsPane.swift:85` | `panel.slint:71` field 13 | done | Reset does not clear family and `byHand` | `oni-app/src/fields.rs::each_slider_spans_its_knobs_range_on_a_mosaic_negative` |
| Dye Separation hidden in B&W, RGB passes, Scanner face | `UI/SettingsPane.swift:69` | none | missing | Always shown | no check |
| Balance Strength slider | `UI/SettingsPane.swift:93` | `panel.slint:79` field 11 | done | | `oni-app/src/fields.rs::each_slider_spans_its_knobs_range_on_a_mosaic_negative` |
| Roll-wide write of a Roll gesture | `Scene/EditorScene+RollSettings.swift` | `oni-core` `measured_with`, `tests/roll8.rs` | done | One step over every frame naming the reference | `oni-core/tests/roll8.rs::a_roll_settings_gesture_writes_every_frame_of_the_roll_as_one_step_and_undoes_byte_for_byte` |

## Panel: Crop tab

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Crop block: Copy, Paste, Paste to Roll framing | `UI/FramePane.swift:21` | `panel.slint` `CropPane`, `app.rs` `framing_act`, `paste_framing` | partial | Wired to the store; the paste fits the crop inside the open frame only, the others keep the copied crop as it is; Paste reads "this photo", never the selection | `ONI_BENCH=crop` (copy not exercised) |
| Aspect Ratio tabs Free, 3:2, 4:3, 1:1, 6:7, 5:4 | `UI/GeometryPanel.swift:17` | `oni-settings/src/geometry.rs` `apply_ratio`, `app.rs` `choose_framing` | done | The choice is remembered and handed to the next frame as its Crop tab opens | `oni-settings/src/geometry.rs::a_ratio_cuts_the_frame_about_its_anchor_and_holds_the_floor`, `ONI_BENCH=crop` |
| Vertical toggle | `UI/GeometryPanel.swift:17` | `panel.slint`, `app.rs` `choose_framing` | done | Disabled on Free and 1:1 | `oni-settings/src/geometry.rs::a_ratio_cuts_the_frame_about_its_anchor_and_holds_the_floor` |
| Straighten Angle slider | `UI/GeometryPanel.swift:27` | `app.rs` `straighten`, `geometry.rs` `straightened` | done | Re-fitted from the gesture's starting frame; a 160 ms pause or a frame moved elsewhere starts a new gesture | `oni-settings/src/geometry.rs::straightening_refits_the_frame_inside_the_turned_photo`, `ONI_BENCH=crop` |
| Rotate 90°, Mirror horizontally, vertically | `UI/GeometryPanel.swift:37`, `Pipeline/Geometry.swift:39-72` | `geometry.rs` `rotate`, `mirror`, `mirror_vertically`, `app.rs` `framing_act` | done | Rotate is three quarters counter-clockwise, as Swift's key | `oni-settings/src/geometry.rs::four_quarter_turns_return_the_frame_and_one_swaps_its_sides`, `a_mirror_keeps_the_crop_on_its_content`, `ONI_BENCH=crop` |
| Measuring Window keys | `UI/GeometryPanel.swift:63`, `Geometry.swift:143` | `panel.slint`, `app.rs` `framing_act` | done | Reset, Measuring Window / Apply Window, copy and paste; the copied window lives for the session, as `Propagation`'s | `oni-settings/src/geometry.rs::the_measuring_window_reads_inside_the_crop` |
| Kept pixels LCD | `UI/GeometryPanel.swift`, `Geometry.swift:163` | `panel.slint`, `geometry.rs` `pixel_label` | done | A value label, not Swift's LCD look | `oni-settings/src/geometry.rs::the_measuring_window_reads_inside_the_crop`, `ONI_BENCH=crop` |
| Crop-first warning before Auto White Balance | `UI/GeometryPanel.swift:5` | none | missing | | no check |

## Panel: Color tab

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Auto White Balance key | `UI/ColorPane.swift:57` | `panel.slint:131` | partial | Button unwired; `Measure` has no implementation | no check |
| Batch choice for a selection (AutoBalanceChoice) | `UI/ColorPane.swift:335` | `oni-core/src/balance.rs:98` `averaged` | partial | Core averaging only | `oni-core/src/balance.rs::average_gives_every_frame_the_mean_and_an_unreadable_frame_stops_the_run` |
| Temperature, Tint sliders, coloured tracks | `UI/TemperatureTintPanel.swift` | `panel.slint:137-141` | done | Track ends not tinted | `oni-core/src/knob.rs::temperature_and_tint_move_on_axes_that_leave_each_other_alone` |
| Color density slider | `UI/ColorPane.swift:57` | `panel.slint:144` field 2 | done | | `oni-app/src/fields.rs::each_slider_spans_its_knobs_range_on_a_mosaic_negative` |
| Paper Correction block (paper profile only) | `UI/ColorPane.swift:82` | none | missing | Model in `oni-colour/src/paper.rs` | no check |
| Auto Paper Correction key | `UI/ColorPane.swift:82` | `oni-colour/src/paper.rs` | partial | Solver exists; no UI | `oni-colour/tests/checks.rs::the_automatic_key_solves_the_paper_s_white` (crate only) |
| Lighting: Exposure, Highlights, Shadows, White point, Black point | `UI/ColorPane.swift:104`, `UI/PrintPanel.swift` | `panel.slint:147-160` | done | | `oni-app/src/fields.rs::the_black_point_runs_the_other_way`, `each_slider_spans_its_knobs_range_on_a_mosaic_negative` |
| Effects: Contrast, Vignette, Chrome | `UI/ColorPane.swift:114` | `panel.slint:164-170` | done | | `oni-app/src/fields.rs::each_slider_spans_its_knobs_range_on_a_mosaic_negative` |
| Effects on the Scanner face: CONTRAST, VIGNETTE, CHROME step keys | `UI/ColorPane.swift:121-131` | none | missing | | no check |
| Curves editor: RGB, Luma, R, G, B | `UI/ColorPane.swift:153-166`, `UI/CurveEditorView.swift` | `oni-app/src/editors/curve.rs`, `oni-app/ui/editors.slint` `CurvesEditor` | partial | Every point rule, tabs lit per moved channel, reset, one step per press; drawn over the output histogram where Swift draws the manual curves' input with the channel in focus | `oni-app/src/editors/tests.rs::a_curve_drag_writes_at_the_hand_s_pace_and_undoes_in_one_step`, `oni-app/tests/editors.rs::a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each` |
| Spectrogram Finishing | `UI/GradePane.swift:10-19`, `UI/SpectrogramPanel.swift`, `UI/SpectrogramBands.swift` | `oni-app/src/editors/spectrogram.rs`, `oni-app/ui/editors.slint` `SpectrogramEditor` | partial | Wheel, eight band tabs, Hue, Saturation, Luminance sliders, reset. A press elsewhere unlights the point only when it lands on another focusable control (`dsReleasesFocus`); the band sliders' knobs jump rather than spring on an outside change | `oni-app/src/editors/tests.rs::the_wheel_selects_on_a_press_and_moves_by_the_travel_from_where_it_grabbed`, `oni-app/src/editors/tests.rs::a_band_slider_writes_its_axis_and_a_double_click_zeroes_it`, `oni-app/tests/editors.rs::a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each` |
| Saturation Curve | `UI/GradePane.swift:20-30`, `UI/SaturationCurveView.swift` | `oni-app/src/editors/saturation.rs`, `oni-app/ui/editors.slint` `SaturationEditor` | done | | `oni-app/src/editors/tests.rs::a_saturation_double_click_puts_its_point_back_on_neutral` |
| Grade blocks hidden in monochrome | `Scene/EditorScene+Panel.swift:228-233` | `oni-app/ui/panel.slint` `Editors.monochrome` | done | | `oni-app/tests/editors.rs::a_point_and_a_handle_dragged_in_the_window_reach_the_store_as_one_step_each` |

## Panel: Details tab

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Sharpening: Amount, Radius, Halo limit | `UI/DetailsPane.swift:9`, `UI/SharpenPanel.swift` | `panel.slint:182-187` | done | | `oni-app/src/fields.rs::each_slider_spans_its_knobs_range_on_a_mosaic_negative` |
| Chroma Denoise: Amount, Radius | `UI/DetailsPane.swift:17` | `panel.slint:190-194` | done | Not hidden in monochrome | `oni-app/src/fields.rs::each_slider_spans_its_knobs_range_on_a_mosaic_negative` |
| Texture | `UI/DetailsPane.swift:25` | `panel.slint:197-200` | done | | `oni-app/src/fields.rs::each_slider_spans_its_knobs_range_on_a_mosaic_negative` |
| Off on a contact sheet, with its note | `UI/DetailsPane.swift:37` | none | missing | | no check |

## Panel: De-dust

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Brush Size slider, [ and ] keys | `UI/DedustPane.swift:14`, `UI/EditorCommands.swift:159-163` | `panel.slint` `DedustPane`, `app.rs` `brush_*`, `act` | done | 4 to 120 px, 12 by default, four pixels a key, live only in De-dust; a double press resets the slider | `ONI_BENCH=dedust` |
| Stroke painting on the canvas | `UI/NegativeView.swift` | `app.rs` `paint_at`, `paint_end` | done | Points in fractions of the rendered frame, y down; a stroke of two points at least becomes a correction at the brush's radius, its patch generated next | `ONI_BENCH=dedust` |
| Patch search and generation | `Pipeline/Corrections.swift`, `CorrectionRenderer` | `oni-app/src/heal.rs`, `app.rs` `heal` | done | On the CPU in Core Image's frame; the fallback is flagged | `oni-app/src/heal.rs::the_search_finds_the_place_the_surroundings_were_copied_from`, `ONI_BENCH=dedust` |
| Corrections list, Remove this correction | `UI/CorrectionsPanel.swift` | `panel.slint` `DedustPane`, `app.rs` `remove_correction` | done | Numbered by the marker's number, not the row; removal discards the patch from the cache and the engine | `ONI_BENCH=dedust` |
| Generating… note, fallback warning | `UI/CorrectionsPanel.swift` | `panel.slint` `DedustPane`, `app.rs` `publish_dedust` | done | A help mark, not Swift's warning glyph, carries the fallback's note; no progress spinner | `ONI_BENCH=dedust` |
| Space held: hand tool | `UI/NegativeView.swift`, `UI/CanvasNavigation.swift` `NavigationCatcher` | `oni-app/ui/app.slint` (the key scope's press and release, the painting layer disabled) | done | While held in De-dust a drag pans and the brush's disc hides | no check |
| Patch cache | `Pipeline/Corrections.swift` `CorrectionCache` | `oni-app/src/patches.rs` | done | `Caches/Open Negative Initiative/Corrections/<id>.tiff`, purged at quit | `oni-app/src/patches.rs::a_patch_is_found_under_its_corrections_id_as_swift_names_it` |

## Scanner face (Lab)

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Pro / Scanner face tab bar | `Scene/EditorScene+Bar.swift` | none | missing | | no check |
| Machine: ONI, Frontier | `Scene/EditorScene+Lab.swift:15` | `oni-pipeline/src/scanner.rs` | partial | Render done; no picker | `oni-pipeline/src/scanner.rs::the_oni_machine_rests_lighting_and_moves_the_gains_by_the_clicks` |
| Keypad: DARK/LIGHT, RED/CYAN, GREEN/MAGENTA, BLUE/YELLOW, COOL/WARM | `UI/Lab/LabKeyboard.swift` | `oni-settings/src/scanner.rs`, `oni-core/src/command.rs` | partial | Clicks modelled and reachable from the phone; no desktop keys | `oni-app/tests/phone.rs::a_phone_pairs_from_the_sheet_and_steps_the_open_frame` (phone path) |
| Walk keys between frames | `Scene/EditorScene+Lab.swift:149-159` | none | missing | | no check |
| Zoom bubble, column key | `Scene/EditorScene+Lab.swift` `zoomBubble` | `oni-app/ui/app.slint` (the card's top band), `app.rs` `lay` | partial | The readout and the − / + keys above the canvas's top right, enabled as the ladder allows; the column key beside them is not there | `oni-app/src/viewport.rs::the_ladder_steps_strictly_and_fit_is_its_floor`, `ONI_BENCH=zoom` |
| Esc leaves the Lab | `Scene/EditorScene+Lab.swift` | none | missing | | no check |
| Scanner Emulator menu item | `UI/EditorCommands.swift:175` | `app.slint:77` | partial | Unwired | no check |

## Bar

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Three zones, one item per zone | `Scene/EditorScene+Bar.swift`, `UI/AppBar.swift` | `oni-app/ui/toolbar.slint` | done | | `oni-app/tests/design_system.rs::screens_use_only_the_design_system` (tokens only) |
| Framing, compare, contact sheet keys | `Scene/EditorScene+Bar.swift:66` | `toolbar.slint` EditorKeys | partial | Drawn, unwired | no check |
| Undo, redo keys with their state | `Scene/EditorScene+Bar.swift` | `toolbar.slint`, `app.rs` | done | Editor surface only | `oni-app/src/store.rs::a_drag_is_one_undo_step_and_reports_only_its_slider` |
| Copy, Paste, PASTE TO ROLL keys | `Scene/EditorScene+Bar.swift` | `toolbar.slint` | partial | Drawn, unwired; core done | no check |
| Correction key | `Scene/EditorScene+Bar.swift` | `toolbar.slint` | partial | Unwired | no check |
| RESET SCAN | `Scene/EditorScene+Bar.swift` | `app.rs:1117` | done | | `oni-core/tests/roll8.rs::the_bars_reset_keeps_framing_flat_field_and_roll` |
| Gallery keys: filter word, PREPARE ROLL, trash | `Scene/EditorScene+Bar.swift` | `toolbar.slint` GalleryKeys | partial | Filter label live; the rest unwired | no check |
| Roll-step keys (Film, Flat-Field, Base, Flags, Window) | `Scene/EditorScene+Bar.swift`, `UI/RollReferenceSurface.swift` | none | missing | | no check |
| FEEDBACKS key | `Scene/EditorScene+Bar.swift` | `toolbar.slint` | partial | Unwired | no check |
| EXPORT / ADD FOLDER key | `Scene/EditorScene+Bar.swift` | `toolbar.slint` | partial | Unwired | no check |
| UPDATE key when an update waits | `Scene/EditorScene+Bar.swift`, `Pipeline/Updates.swift` | none | missing | | no check |
| Traffic lights inset | `Scene/EditorScene+Bar.swift` | `app.rs` | missing | Never set | no check |

## Menus and shortcuts

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Open Folder… ⌘O | `UI/EditorCommands.swift:109` | `app.slint:40`, `app.rs` key | missing | "not in this build" | no check |
| Gallery / Editor ⌘G | `UI/EditorCommands.swift:113` | `app.slint:43`, `app.rs:1118` | done | | no check |
| Export… ⌘E | `UI/EditorCommands.swift:116` | `app.slint:44` | missing | Title lacks the ellipsis and the count | no check |
| Recheck Originals ⌘R | `UI/EditorCommands.swift:120` | `app.slint:46` | missing | | no check |
| Read the Folders Again | `UI/EditorCommands.swift:123` | `app.slint:47` | missing | | no check |
| Show the Selected Photo | `UI/EditorCommands.swift:125` | `app.slint:48` | missing | | no check |
| Move to ONI Trash ⌘⌫ | `UI/EditorCommands.swift:128` | `app.slint:50`, `oni-core/src/store.rs:704` | partial | Core only | `oni-core/tests/roll8.rs::the_trash_marks_the_sidecar_and_never_moves_the_file` (core only) |
| Restore ⇧⌘⌫ | `UI/EditorCommands.swift:131` | `app.slint:51`, `store.rs:708` | partial | Core only | no check |
| Zoom Out ⌘−, Zoom In ⌘+ | `UI/EditorCommands.swift:138-142` | `app.rs` `act`, `step_zoom` | done | The editor walks the zoom ladder, the gallery its columns | `oni-app/src/viewport.rs::the_ladder_steps_strictly_and_fit_is_its_floor`, `oni-app/src/gallery.rs::the_tile_follows_the_width_and_the_cells_sit_on_the_swift_grid` |
| Compare Before/After | `UI/EditorCommands.swift:145` | `app.slint` "Compare", `app.rs` `act` | partial | Works; the title reads "Compare", Swift's "Compare Before/After" | `ONI_BENCH=compare` |
| Hide the Library ⌥⌘[ | `UI/EditorCommands.swift:147` | `app.slint:66` "Hide Library" | partial | Works; title differs, no shortcut | no check |
| Hide the Panel ⌥⌘] | `UI/EditorCommands.swift:150` | `app.slint:67` | partial | Works; title differs, no shortcut | no check |
| Crop and Straighten ⌘K | `UI/EditorCommands.swift:153` | `app.slint:68` | missing | | no check |
| Remove Dust & Scratches | `UI/EditorCommands.swift:156` | `app.slint` "De-dust", `app.rs` `act` | partial | Enters and leaves De-dust; the title reads "De-dust" | `ONI_BENCH=dedust` |
| Bigger Brush ], Smaller Brush [ | `UI/EditorCommands.swift:159-163` | `app.slint:71-72` | missing | Title carries the key as text | no check |
| Mirror Horizontally ⇧⌘M | `UI/EditorCommands.swift:166` | `app.slint:73` "Mirror" | missing | | no check |
| Mute Key Clicks | `UI/EditorCommands.swift` `soundTitle` | none | missing | | no check |
| Show What This Screen Cannot Show ⇧⌘Y | `UI/EditorCommands.swift:171` | `app.slint` "Screen Gamut", `app.rs` `act`, ⇧⌘Y in `key` | partial | Works; the title reads "Screen Gamut" | `oni-app/src/display.rs::the_zebra_stripes_what_the_screen_cannot_show_as_its_reference_does` |
| Labels: None 0, Bronze 1, Silver 2, Gold 3 | `UI/EditorCommands.swift:180-183` | `app.slint:81-84`, `app.rs` `label` | done | | `oni-settings/tests/real_files.rs::label_trash_and_draft_touch_nothing_else_and_keep_the_revision` (sidecar only) |
| Filter: All ⌘L | `UI/EditorCommands.swift:187` | `app.slint:86` "Cycle Filter" | missing | | no check |
| New Roll from Selection ⇧⌘G | `UI/EditorCommands.swift:194` | `app.slint:90` | missing | | no check |
| Ungroup ⌥⇧⌘G | `UI/EditorCommands.swift:198` | `app.slint:91` | missing | | no check |
| New Contact Sheet | `UI/EditorCommands.swift:204` | `app.slint:93` | missing | | no check |
| Prepare the Roll… | `UI/EditorCommands.swift:206` | `app.slint:94` | missing | | no check |
| Reset to Default… | `UI/EditorCommands.swift:208` | `app.slint:95`, `oni-core/src/store.rs:602` | partial | Core only | no check |
| Copy Settings ⇧⌘C | `UI/EditorCommands.swift:213` | `app.slint:99`, `store.rs:496` | partial | Core only, in-memory board | `oni-core/tests/roll8.rs::a_paste_carries_the_color_and_details_tabs_whole_and_undoes_byte_for_byte` (core only) |
| Paste Settings ⇧⌘V | `UI/EditorCommands.swift:216` | `app.slint:100`, `store.rs:526` | partial | Core only | `oni-core/tests/roll8.rs::a_paste_carries_the_color_and_details_tabs_whole_and_undoes_byte_for_byte` (core only) |
| Paste to Roll (no shortcut) | `UI/EditorCommands.swift:220` | `app.slint:101`, `store.rs:535` | partial | Core only | `oni-core/tests/roll8.rs::paste_to_roll_is_one_editor_step_over_the_whole_roll` (core only) |
| Save as, Edit, Import Presets, Paste Preset Code, Show Presets Folder | `UI/EditorCommands.swift:223-231` | `app.slint:103-107` | missing | | no check |
| Preset list with its parameters | `UI/EditorCommands.swift:238` | none | missing | | no check |
| Delete a Preset submenu | `UI/EditorCommands.swift:245` | none | missing | | no check |
| Undo ⌘Z, Redo ⇧⌘Z with action names | `UI/EditorCommands.swift:255-259` | `app.slint:55-56` | partial | Shortcuts work; titles never name the action | `oni-app/src/store.rs::a_drag_is_one_undo_step_and_reports_only_its_slider` |
| Reset to Imported State ⇧⌘R | `UI/EditorCommands.swift:262` | `app.slint:58` | partial | Menu works; no shortcut | no check |
| Help: Console, What's New, Report, Year on Film | `UI/EditorCommands.swift:96-100` | `app.slint:111-115` | missing | | no check |
| Check for Updates… | `UI/EditorCommands.swift:105` | none | missing | | no check |
| Enabled state of every item | `UI/EditorCommands.swift` `.disabled(!live(...))` | undo, redo only | partial | | no check |
| Pair Phone… | none | `app.slint:41` | port-only | | `oni-app/tests/phone.rs::a_phone_pairs_from_the_sheet_and_steps_the_open_frame` |

## Keys outside the menus

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Gallery ⌘A, arrows, Return, Space | `UI/GalleryEngine.swift:425-439` | none | missing | | no check |
| Bare keys yield to a focused text field | `DesignSystem/DSKeyboard` `DSTextFocus` | `oni-app/src/app.rs:1155` | done | Slint text fields consume their keys first | no check |
| Exclusive key surfaces | `DSKeyMonitor.exclusiveSurfaces` | none | missing | | no check |
| Esc closes Lab, Crop, Wrapped | `Scene/EditorScene+Lab.swift`, `UI/CropOverlay.swift`, `UI/WrappedView.swift` | Esc closes pairing only | missing | | no check |
| Tab walks spectrogram bands | `UI/SpectrogramPanel.swift:76-84` | `oni-app/src/editors/mod.rs` `Gestures::key` | done | | `oni-app/src/editors/tests.rs::the_wheel_s_keys_step_the_selected_point_and_tab_walks_it_only_while_it_holds_the_focus` |
| Option on the spectrogram | `UI/SpectrogramPanel.swift:81` | `oni-app/src/editors/spectrogram.rs` `walked` | done | Option-Tab walks anticlockwise | `oni-app/src/editors/tests.rs::the_wheel_s_keys_step_the_selected_point_and_tab_walks_it_only_while_it_holds_the_focus` |
| Prepare the Roll arrows | `UI/RollReferenceSurface.swift:223` | none | missing | | no check |

## History

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Editor history, coalescing 0.4 s, 200 steps | `Pipeline/History.swift:6-8` | `oni-core/src/history.rs:8-9` | done | | `oni-core/src/history.rs::a_drag_within_the_window_is_one_step`, `the_stack_holds_its_limit` |
| Gallery history, 100 steps | `Pipeline/GalleryHistory.swift:28` | `oni-library/src/gallery_history.rs:39` | partial | Model done; bar and menu read the editor's only | `oni-library/src/gallery_history.rs::the_stack_keeps_its_last_hundred_steps` (model only) |
| Undo title names the action | `Scene/EditorScene+History.swift` | `oni-app/ui/toolbar.slint:14` `undo-title` | partial | Property never set: always "Undo" | no check |
| A roll write is one step | `Scene/EditorScene+RollSettings.swift` | `oni-core/tests/roll8.rs` | done | | `oni-core/tests/roll8.rs::a_drag_is_one_step_and_one_roll_write` |
| Sidecar written 400 ms after the last notch | `Pipeline/Sidecar.swift:7` | `oni-app/src/app.rs:34` | partial | App waits 500 ms; `oni-settings/src/sidecar.rs:65` has 400 unused | `oni-core/src/store.rs::an_untouched_frame_gains_no_sidecar_and_a_rested_edit_writes_one` (no delay asserted) |
| Flush on quit and window close | `Pipeline/QuitGuard` | none | missing | Pending writes are lost on close | no check |
