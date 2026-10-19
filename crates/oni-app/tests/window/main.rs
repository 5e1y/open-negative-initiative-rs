//! Every window check in one binary, each in a process of its own, as Slint takes one platform a
//! process: `cargo test --test window [filter]`; `ONI_ONE=<name>` runs one check in this process.

mod common;
mod runner;
mod archive;
mod auto_balance;
mod canvas_switch;
mod catalogue;
mod contact_sheet;
mod design_system;
mod editor_leftovers;
mod editors;
mod export_paper_window;
mod export_window;
mod extended;
mod gallery;
mod gestures;
mod headless;
mod help;
mod keycap;
mod launch;
mod library_column;
mod masks;
mod navigator;
mod paper_sheet_window;
mod phone;
mod prepare;
mod preset_deck;
mod presets;
mod quit;
mod rgb_passes;
mod roll_tab;
mod rolls;
mod scanner;
mod scroll;
mod session;
mod space_chip;
mod stocks;
mod updates;
mod wrapped;

/// Each check by the name the runner prints and filters on.
const CHECKS: &[(&str, fn())] = &[
    ("archive::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once", archive::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once),
    ("auto_balance::the_key_balances_the_open_frame_off_the_interface_thread_and_one_undo_takes_it_back", auto_balance::the_key_balances_the_open_frame_off_the_interface_thread_and_one_undo_takes_it_back),
    ("canvas_switch::a_photo_switched_to_shows_nothing_of_the_last_and_numbers_its_corrections_from_one", canvas_switch::a_photo_switched_to_shows_nothing_of_the_last_and_numbers_its_corrections_from_one),
    ("catalogue::the_catalogue_shows_every_component", catalogue::the_catalogue_shows_every_component),
    ("contact_sheet::a_roll_lays_its_sheet_and_a_selection_composes_one_that_opens_dimming_what_a_sheet_skips", contact_sheet::a_roll_lays_its_sheet_and_a_selection_composes_one_that_opens_dimming_what_a_sheet_skips),
    ("design_system::screens_use_only_the_design_system", design_system::screens_use_only_the_design_system),
    ("design_system::a_planted_value_is_refused_outside_the_design_system_only", design_system::a_planted_value_is_refused_outside_the_design_system_only),
    ("editor_leftovers::the_editors_last_gestures_behave_as_swifts", editor_leftovers::the_editors_last_gestures_behave_as_swifts),
    ("editor_leftovers::every_block_says_swifts_own_help", editor_leftovers::every_block_says_swifts_own_help),
    ("editors::a_point_dragged_on_the_curves_reaches_the_store_as_one_step", editors::curves::a_point_dragged_on_the_curves_reaches_the_store_as_one_step),
    ("editors::a_double_click_on_a_point_removes_it_with_its_move_as_one_step", editors::curves::a_double_click_on_a_point_removes_it_with_its_move_as_one_step),
    ("editors::a_folded_curves_block_takes_no_press", editors::curves::a_folded_curves_block_takes_no_press),
    ("editors::a_handle_dragged_on_the_levels_is_one_step_and_springs_back_on_undo", editors::panel::a_handle_dragged_on_the_levels_is_one_step_and_springs_back_on_undo),
    ("editors::the_wheel_takes_the_arrows_once_pressed_and_its_swatches_name_their_bands", editors::panel::the_wheel_takes_the_arrows_once_pressed_and_its_swatches_name_their_bands),
    ("editors::the_panel_lays_out_only_the_blocks_a_photo_and_a_face_have", editors::panel::the_panel_lays_out_only_the_blocks_a_photo_and_a_face_have),
    ("export_paper_window::a_paper_proofs_the_canvas_and_its_correction_is_laid_on_the_paper_alone", export_paper_window::a_paper_proofs_the_canvas_and_its_correction_is_laid_on_the_paper_alone),
    ("export_window::the_export_surface_writes_what_the_proofed_canvas_shows_and_only_onis_metadata", export_window::the_export_surface_writes_what_the_proofed_canvas_shows_and_only_onis_metadata),
    ("extended::a_decode_outside_libraw_is_swift_s", extended::a_decode_outside_libraw_is_swift_s),
    ("extended::a_patch_is_swift_s_on_swift_s_source", extended::a_patch_is_swift_s_on_swift_s_source),
    ("extended::a_frame_renders_its_swift_patches_as_swift_does", extended::a_frame_renders_its_swift_patches_as_swift_does),
    ("extended::a_patch_reduces_as_core_image_reduces_it", extended::a_patch_reduces_as_core_image_reduces_it),
    ("extended::a_patch_lands_and_feathers_as_swift_s", extended::a_patch_lands_and_feathers_as_swift_s),
    ("extended::an_export_writes_swift_s_pixels_profile_and_tags", extended::an_export_writes_swift_s_pixels_profile_and_tags),
    ("gallery::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand", gallery::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand),
    ("gestures::the_menus_name_what_they_do_and_the_gestures_reach_every_target", gestures::the_menus_name_what_they_do_and_the_gestures_reach_every_target),
    ("headless::the_window_opens_roll_8_and_a_slider_drives_the_canvas", headless::the_window_opens_roll_8_and_a_slider_drives_the_canvas),
    ("help::the_help_menus_surfaces_behave_as_swifts", help::the_help_menus_surfaces_behave_as_swifts),
    ("keycap::a_tap_holds_the_cap_down_its_floor_and_a_long_press_lifts_at_once", keycap::a_tap_holds_the_cap_down_its_floor_and_a_long_press_lifts_at_once),
    ("launch::the_splash_names_the_version_and_the_notice_covers_the_window", launch::the_splash_names_the_version_and_the_notice_covers_the_window),
    ("library_column::the_column_follows_the_open_photo_and_keeps_the_wheel_s_scroll", library_column::the_column_follows_the_open_photo_and_keeps_the_wheel_s_scroll),
    ("masks::the_mask_library_loads_renames_shows_remembers_and_moves_a_photo_out_as_swift_does", masks::the_mask_library_loads_renames_shows_remembers_and_moves_a_photo_out_as_swift_does),
    ("navigator::the_navigator_walks_its_disk_from_computer_and_the_next_launch_finds_it_where_it_was_left", navigator::the_navigator_walks_its_disk_from_computer_and_the_next_launch_finds_it_where_it_was_left),
    ("paper_sheet_window::a_maker_s_profile_is_listed_installed_chosen_and_uninstalled", paper_sheet_window::a_maker_s_profile_is_listed_installed_chosen_and_uninstalled),
    ("phone::a_phone_pairs_from_the_sheet_and_steps_the_open_frame", phone::a_phone_pairs_from_the_sheet_and_steps_the_open_frame),
    ("prepare::a_roll_is_prepared_left_resumed_from_its_draft_and_measured_as_one_gallery_step", prepare::a_roll_is_prepared_left_resumed_from_its_draft_and_measured_as_one_gallery_step),
    ("preset_deck::the_deck_answers_the_pointer_as_swift_s_carousel", preset_deck::the_deck_answers_the_pointer_as_swift_s_carousel),
    ("presets::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does", presets::a_preset_is_saved_previewed_applied_edited_shared_and_deleted_as_swift_does),
    ("quit::closing_writes_the_last_settings_and_a_failed_write_asks_first", quit::closing_writes_the_last_settings_and_a_failed_write_asks_first),
    ("rgb_passes::a_roll_of_passes_is_grouped_by_threes_ordered_by_its_light_and_registered_as_measured", rgb_passes::a_roll_of_passes_is_grouped_by_threes_ordered_by_its_light_and_registered_as_measured),
    ("roll_tab::the_roll_tabs_blocks_write_what_swifts_write", roll_tab::the_roll_tabs_blocks_write_what_swifts_write),
    ("rolls::the_roll_gestures_reach_what_swift_reaches", rolls::the_roll_gestures_reach_what_swift_reaches),
    ("scanner::the_keypad_presses_the_phone_s_commands_and_the_lab_screen_reads_them_back", scanner::the_keypad_presses_the_phone_s_commands_and_the_lab_screen_reads_them_back),
    ("space_chip::the_space_chips_tabs_take_the_pointer", space_chip::the_space_chips_tabs_take_the_pointer),
    ("scroll::a_surface_scrolls_by_the_wheel_and_never_by_a_drag", scroll::a_surface_scrolls_by_the_wheel_and_never_by_a_drag),
    ("session::the_tile_size_and_the_muted_clicks_outlast_the_session", session::the_tile_size_and_the_muted_clicks_outlast_the_session),
    ("stocks::a_rolls_stock_is_picked_by_brand_and_shown_with_its_artwork", stocks::a_rolls_stock_is_picked_by_brand_and_shown_with_its_artwork),
    ("updates::updates_are_looked_for_on_their_own_shown_on_the_key_and_only_ever_downloaded", updates::updates_are_looked_for_on_their_own_shown_on_the_key_and_only_ever_downloaded),
    ("wrapped::every_gesture_is_noted_and_the_year_reads_back_as_its_deck", wrapped::every_gesture_is_noted_and_the_year_reads_back_as_its_deck),
];

fn main() {
    runner::run(CHECKS);
}
