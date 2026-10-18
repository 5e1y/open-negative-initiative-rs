# Parity: archive, gallery and rolls

Swift paths are under `Sources/OpenNegative/` of the Swift app; Rust paths are under `crates/`.
Statuses are defined in [README.md](README.md).

## Library, roots and folders

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Open a folder from a picker | `UI/RootPicker.swift`, `UI/EditorCommands.swift:109` | `oni-app/src/app.rs` `pick`, `add`, `system.rs` `pick_folder` | done | A folder opens on the gallery, nothing in the editor; a file opens its folder then itself; a folder reached through a link reads on its real path | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once`, `oni-app/src/library.rs::a_folder_reached_through_a_link_lists_the_photos_on_its_real_path` |
| Several roots, surveyed off the main thread | `Pipeline/Roots.swift` | `oni-library/src/roots.rs` | partial | Crate surveys many; the app holds one folder | `oni-library/src/roots.rs::roots_never_hold_a_photo_twice` |
| Add Folder… on the empty archive | `UI/RootPicker.swift` | `app/organising.rs` `publish_empty` | partial | The empty state's Open Folder… opens the picker; `RootWelcome` listing former folders is absent | no check |
| Former library offered per volume (RootsCatchUp) | `UI/RootsCatchUp.swift` | `oni-library/src/roots.rs` | partial | Model only | `oni-library/src/roots.rs::a_former_library_is_offered_per_volume_never_the_home_itself` |
| SQLite index as a disposable cache | `Pipeline/LibraryIndex.swift` | `oni-library/src/index.rs`, `oni-app/src/library.rs` `read_rows` | done | The index lives in the caches; a second read reads no file again | `oni-app/src/library.rs::a_folder_read_twice_reads_its_files_once_and_drops_a_photo_gone`, `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Library reload, invented and waiting rows | `Pipeline/Library.swift:43-44` | `oni-app/src/library.rs` `read_rows` | done | Invented rows are dropped and unread photos land as waiting rows on every read | `oni-library/src/library.rs::an_invented_row_is_dropped_and_a_waiting_one_kept`, `oni-app/src/library.rs::a_folder_read_twice_reads_its_files_once_and_drops_a_photo_gone` |
| Relink a moved original by fingerprint | `Pipeline/Relink.swift` | `oni-library/src/library.rs`, `oni-app/src/library.rs` `is_stale` | done | The fingerprint keys rows, thumbnails and projects; a stale size drops its row; Recheck Originals reads again | `oni-library/src/relink.rs::two_files_of_one_size_part_on_their_sample_and_a_renamed_one_keeps_its_print`, `oni-library/src/relink.rs::staleness_reads_the_size_at_the_head` |
| Read the Folders Again | `UI/EditorCommands.swift:123` | `oni-app/src/app.rs` `read_again` | done | | no check |
| Show the Selected Photo, Show in Finder | `UI/EditorCommands.swift:125`, `UI/GalleryView.swift:424` | `oni-app/src/app.rs` `show_selected`, `gallery.rs` `reveal`, `oni-app/src/app/organising.rs` `CellAction::Reveal` | partial | Show in Finder reveals the scope's first photo where Swift selects them all; no check opens a file viewer | `oni-app/src/gallery.rs::reveal_moves_the_least_to_show_a_cell_and_not_at_all_when_it_shows` |
| Opened folder remembered across launches | `Scene/EditorScene+Removal.swift:142` `openedFolder` | `oni-app/src/preferences.rs`, `app.rs` `open_remembered` | done | | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Every path from AppFolders, sandboxed under checks | `Pipeline/AppFolders.swift` | `oni-app/src/timing.rs` `support`, `system.rs` `caches` | done | Support and caches come from `Options`, `ONI_SUPPORT` and `ONI_CACHES`, so each test has its own | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |

## Folder tree (navigator)

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Tree of folders then photos, newest first | `UI/TreeEngine.swift`, `Pipeline/ArchiveTree.swift` | `oni-app/ui/library.slint` Tree, `oni-library/src/archive_tree.rs` | done | | `oni-library/src/folders.rs::the_tree_opens_on_its_folders_then_its_photos_and_an_empty_folder_by_its_birth` |
| Unfold on click, chevron | `UI/TreeRowChrome.swift`, `UI/FolderNavigator.swift` | `library.slint` `row-clicked` | done | Chevron aspect `TreeRowChrome.swift:39` not checked | `oni-app/src/library.rs::roll_8_opens_as_one_named_roll_of_its_photos` |
| Navigator walks up to Computer | `Pipeline/Navigator.swift` | none | missing | | no check |
| Favourites, favourites-only | `Pipeline/Favourites.swift`, `UI/FolderNavigator.swift:21` | `oni-library/src/favourites.rs` | partial | Crate only | `oni-library/src/favourites.rs::favourites_star_beside_each_other_and_follow_a_rename` |
| Navigator state remembered (height, base, unfolded) | `UI/FolderNavigator.swift:15-22` | none | missing | | no check |
| Folder menu: Open, Rename…, New Folder Inside, Show in Finder | `UI/FolderNavigator.swift`, `Scene/EditorScene+Gallery.swift` | none | missing | | no check |
| Folder menu: Make a Roll, Back to a Folder | `Scene/EditorScene+Gallery.swift` | `oni-library/src/roll_folder.rs` | partial | Crate only | `oni-library/src/roll_folder.rs::a_roll_folder_takes_its_photos_and_gives_them_back` |
| ONI Trash row | `UI/FolderNavigator.swift:292-322`, `:471-547` | `oni-app/ui/ds/organise.slint` `TrashRow`, `oni-app/src/app/organising.rs` `publish_trash_rows` | done | Count, fold, folders latest first, Restore under the hover, Open / Restore All… / Show in Finder | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand` |
| A roll row shows its default stock | `UI/TreeRowChrome.swift` | none | missing | | no check |
| Drop over the tree lands where its strips say | `UI/ListPicking.swift` `TreeDropTarget` | `oni-app/src/app/organising.rs` `pointer_moved` | partial | The archive refuses every drag (checked); a project's photos reorder in its grid, but the project tree takes no drop | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand` |
| Hide the Library ⌥⌘[ | `UI/EditorCommands.swift:147` | `app.slint` menu bar | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |

## Gallery grid

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Tiles on the Swift grid, columns follow the width | `UI/GalleryLayout.swift`, `UI/GalleryView.swift` | `oni-app/src/gallery.rs`, `library.slint` Grid | done | | `oni-app/src/gallery.rs::the_tile_follows_the_width_and_the_cells_sit_on_the_swift_grid` |
| Only rows in view are asked for | `UI/GalleryLayout.swift:51` overscan | `oni-app/src/gallery.rs:65` | done | | `oni-app/src/gallery.rs::a_scroll_asks_only_for_the_rows_in_view` |
| Scroll by the wheel only | `DesignSystem/DSScrollAdjust` | `oni-app/tests/scroll.rs` | done | | `oni-app/tests/scroll.rs::a_surface_scrolls_by_the_wheel_and_never_by_a_drag` |
| Headings per roll and day | `Pipeline/Film.swift`, `UI/GalleryView.swift` | `oni-library/src/film_group.rs`, `library.slint` headings | done | | `oni-library/src/film_group.rs::rolls_bucket_by_id_and_days_by_local_midnight` |
| ⌘ and ⇧ click selection | `UI/ListPicking.swift`, `UI/GalleryEngine.swift:398-412` | `oni-app/src/organise.rs` `Picks::click`, `library.slint` Grid pointer | done | Kept by path, so a relayout keeps it | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-app/src/picking.rs::a_clicks_command_and_shift_pick_as_shown` |
| Double-click opens in the editor | `UI/GalleryView.swift:165-183` `ClickTracker`, `UI/GalleryEngine.swift:407` | `oni-app/src/picking.rs` `ClickTracker`, `oni-app/src/app/organising.rs` `click` | done | The system's interval; the ONI Trash opens nothing | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-app/src/organise.rs::only_the_second_plain_click_within_the_interval_opens` |
| One selection shared by the grid and the sidebar, its ring, the frame opened designated | `OpenNegativeApp.swift:154-159`, `UI/TreeEngine.swift:247`, `Scene/EditorScene+Removal.swift:563` `designate`, `UI/GalleryEngine.swift:223` | `oni-app/src/app.rs` `tree_marks`, `mark_tree`, `designate`, `oni-app/src/app/organising.rs` `pointer_up` | done | A row lights for the selection or the ring; a click under the last row reaches nothing, as Swift's document ends with its cells | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `ONI_BENCH=flows` in the real window |
| ⌘A, arrows, Return, Space | `UI/GalleryEngine.swift:425-512`, `UI/GalleryView.swift:47-118` | `oni-app/src/organise.rs` `Picks::key`, `oni-app/src/app/organising.rs` `grid_key` | done | | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-app/src/organise.rs::return_and_space_open_and_command_a_completes_the_rolls_touched`, `oni-app/src/picking.rs::arrows_walk_cells_and_rows_and_a_row_never_crosses_a_band_unseen` |
| Zoom the grid ⌘− ⌘+ | `UI/EditorCommands.swift:138-142` | `oni-app/src/gallery.rs` `column_step` | partial | Swift's two rungs; the count is not remembered across launches (`galleryColumns`) | no check |
| Thumbnails: 320 px, disk cache, render version 11 | `Pipeline/Thumbnails.swift:15-21` | `oni-app/src/thumbnails.rs`, `app.rs` `paint` | partial | 320 px JPEGs at 0.6 named by fingerprint, drawn only when absent; an unconverted frame renders through the pipeline where Swift shows the embedded scan | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Thumbnails proofed into the export space | `Pipeline/Thumbnails.swift:13` | `oni-app/src/render.rs` `Engine::render`, the proof stage the canvas lays | done | The same stage as the canvas's; no check of its own | no check |
| Thumbnail refresh 2 s after an edit | `Pipeline/Thumbnails.swift:21` | `oni-app/src/app.rs` `schedule_redraw` | done | | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Triplet of RGB passes shows its lead | `UI/TileLayers.swift:256` | `oni-library/src/index.rs`, `ui/ds/collections.slint` `TileView` `rgb` | partial | The tile wears the RGB seal; the status bar's seal is absent | `oni-library/src/index.rs::a_triplet_draws_as_its_lead_and_its_passes_show_once_it_is_gone` |
| Cell menu: Paste Settings, Reset Settings, Reset to Default… | `UI/GalleryView.swift:320-449` | `oni-app/src/organise.rs` `cell_menu`, `oni-app/src/app/organising.rs` `cell_act` | partial | Paste and Reset act on the scope they select; Reset to Default… reaches a gesture this build lacks | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-app/src/organise.rs::the_cell_menu_acts_on_the_selection_it_points_into_and_counts_each_verbs_own_scope` |
| Cell menu: medals | `UI/GalleryView.swift:368-374` | `oni-app/src/app/organising.rs` `cell_act` `Medal`, keys 0-3 | done | | `oni-app/src/organise.rs::the_cell_menu_acts_on_the_selection_it_points_into_and_counts_each_verbs_own_scope`, `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Cell menu: New Roll from, Ungroup | `UI/GalleryView.swift:378-389`, `Scene/EditorScene+Gallery.swift:145` | `oni-app/src/app/organising.rs` `cell_act`, `oni-core/src/store.rs` `set_film` | done | Ungroup writes no roll on the photos a roll names, one gallery step | `oni-core/src/store.rs::a_roll_set_on_its_frames_writes_the_others_at_once_keeps_their_dates_and_undoes_as_one_step`, `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |
| Cell menu: New Project from, Add to, Remove from Project | `UI/GalleryView.swift:390-417`, `Scene/EditorScene+Rolls.swift:117-284` | `oni-app/src/app/organising.rs` `cell_act`, `oni-library/src/project.rs` gestures | done | Submenus open in the same panel | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-library/src/project.rs::joining_a_project_leaves_every_other_and_an_emptied_one_goes` |
| Cell menu: Export…, Show in Finder, Move to ONI Trash | `UI/GalleryView.swift:419-433` | `oni-app/src/app/organising.rs` `cell_act` | partial | Export… and Move to ONI Trash act on their scope; Show in Finder reveals the scope's first photo | `oni-app/src/organise.rs::the_cell_menu_acts_on_the_selection_it_points_into_and_counts_each_verbs_own_scope` |
| Empty states | `Scene/EditorScene+Card.swift:19-69`, `:428-433` | `ui/ds/organise.slint` `EmptyState`, `oni-app/src/app/organising.rs` `publish_empty` | done | Swift's titles and details for the library, a filter, no folder, the projects and the ONI Trash | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand` |
| Status bar with medal picker | `Scene/EditorScene+Card.swift:206-268` | `library.slint` `GalleryStatus`, `ui/ds/organise.slint` `MedalPicker`, `oni-app/src/app/organising.rs` `publish_status` | partial | One photo toggles its medal, several keep it, the ONI Trash counts; the name has no pixel size after it | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-app/src/picking.rs::the_status_bar_names_the_photo_or_counts_and_its_picker_toggles_one_photo_only` |

## Labels and filter

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Medal written to the sidecar alone | `Pipeline/Medal.swift`, `Pipeline/Sidecar.swift` | `oni-app/src/app.rs:932`, `oni-settings/src/sidecar.rs` | done | | `oni-settings/tests/real_files.rs::label_trash_and_draft_touch_nothing_else_and_keep_the_revision` |
| Medal names and order | `Pipeline/Medal.swift` | `oni-settings/src/modes.rs` | done | | `oni-settings/src/modes.rs::a_medal_is_written_by_name_and_an_unknown_one_refused` |
| Filter: All / medals, ⌘L | `UI/EditorCommands.swift:187` | `app.rs` `cycle_filter`, `toolbar.slint` filter word | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |

## Info sidebar

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Roll title field | `UI/InfoSidebar.swift:174-176`, `:225-252` | `library.slint` `EditableTitle`, `oni-app/src/app/organising.rs` `title_committed` | done | A folder's roll writes its marker, a sidecar roll every member (`set_film`); a blank or unchanged name is refused | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand` |
| Film Stock, Select Roll Type, stock picker sheet | `UI/InfoSidebar.swift:177-192`, `:262-282`, `UI/StockPickerSheet.swift` | `library.slint` Info, `oni-app/src/app/organising.rs` `publish_info` | partial | The stock or the first photo's mode default, said so; Select Roll Type reaches no picker in this build | `oni-settings/src/film.rs::the_catalogue_holds_unique_keys` |
| Film stock icons | `DesignSystem/DSIcon` `FilmStockIcon` | none | missing | | no check |
| Shot Date with clear key, clamped typing | `UI/InfoSidebar.swift:193-206`, `:287-346` | `ui/ds/organise.slint` `DateFields`, `oni-app/src/app/organising.rs` `shot_date_committed`, `oni-library/src/info.rs` `made` | done | | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-library/src/info.rs::a_typed_date_clamps_to_its_months_last_day_and_an_impossible_month_is_refused` |
| Camera Body field | `UI/InfoSidebar.swift:207-221` | `library.slint` Info, `oni-app/src/app/organising.rs` `camera_body_committed` | done | Written on Return or blur, trimmed, an unchanged body writes nothing | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand` |
| Description field, persisted to the roll marker | `UI/InfoSidebar.swift:100-111` | `oni-app/src/app/organising.rs` `description_committed` | done | Swift's roll has no description: the field is the project's, saved on blur to `projects.json` | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand` |
| Project info: name, stocks counted by photo, description | `UI/InfoSidebar.swift:76-160` | `library.slint` Info, `ui/ds/organise.slint` `CardStack`, `oni-library/src/info.rs` | done | The card stack is not rotated | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-library/src/info.rs::a_projects_stocks_are_counted_by_photo_and_a_photo_with_none_gives_no_row` |

## Rolls

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Rolls group by reference id, days by local midnight | `Pipeline/Film.swift` | `oni-library/src/film_group.rs` | done | | `oni-library/tests/roll8.rs::roll_membership_reads_the_reference_id_each_sidecar_names` |
| New Roll from Selection ⇧⌘G (folder on disk, ⌘Z) | `Pipeline/RollFolder.swift`, `Scene/EditorScene+Rolls.swift` | `oni-app/src/app.rs` `new_roll`, `followed`, `oni-library/src/roll_folder.rs` | done | The heading is not offered for renaming | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target`, `oni-library/src/roll_folder.rs::a_roll_folder_takes_its_photos_and_gives_them_back` |
| Ungroup ⌥⇧⌘G | `Scene/EditorScene+Rolls.swift` | `app/rolls.rs` `ungroup`, `app/organising.rs` cell menu | done | From the menu and the cell menu, one gallery step | oni-app/tests/rolls.rs::the_roll_gestures_reach_what_swift_reaches |
| Fresh roll takes the lowest free number | `Pipeline/Film.swift:144` | `oni-library/src/film_group.rs` | done | | `oni-library/src/film_group.rs::a_fresh_roll_takes_the_lowest_free_number` |
| Reset to Default… reaches whole rolls and sheets | `Pipeline/RollSettings.swift:114` `resetReach` | `oni-library/src/roll_membership.rs` `reset_reach`, `app/rolls.rs` | done | Reached from the menu and the cell menu, confirmed, undone as one step | oni-app/tests/rolls.rs::the_roll_gestures_reach_what_swift_reaches, `oni-library/tests/swift_checks.rs::a_reset_to_default_reaches_whole_rolls_and_their_sheets_never_a_neighbour` |
| Paste to Roll writes the whole roll | `Scene/EditorScene+Paste.swift` | `oni-app/src/app.rs` `paste_to_roll`, `oni-core/src/store.rs` | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target`, `oni-core/tests/roll8.rs::paste_to_roll_is_one_editor_step_over_the_whole_roll` |

## Prepare the Roll

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Surface with steps Film, Flat-Field, Base, Flags, Window | `UI/RollReferenceSurface.swift` | none | missing | | no check |
| Steps saved in the frames' sidecars (RollDraft) | `Pipeline/RollDraft.swift` | `oni-core/src/store/roll.rs` `land_roll` | partial | The field round-trips and a landed measure clears it in the crate; no surface writes it | `oni-settings/tests/real_files.rs::label_trash_and_draft_touch_nothing_else_and_keep_the_revision` |
| Frame card in each scan's shape | `UI/RollReferenceSurface.swift` `RollFrameView` | none | missing | | no check |
| Pick and flag frames one click at a time | `UI/RollReferenceSurface.swift` `RollPicking` | none | missing | | no check |
| Clear, Clear Flags, Shift by One File, Previous / Next | `UI/RollReferenceSurface.swift` | none | missing | | no check |
| MEASURE N FRAMES, NEXT / SKIP, Back | `UI/RollReferenceSurface.swift` | none | missing | | no check |
| Base pipette and its radius | `Pipeline/RollReference.swift:22-24`, `Pipeline/Pipette.swift` | `oni-measure/src/key.rs:107` `base`, `oni-measure/src/roll.rs:12` | partial | The reading is ported; the Base step that calls it is not | `oni-measure/tests/pipette.rs::the_ring_the_hand_aims_with_is_the_sample_at_every_radius` (crate only) |
| Roll reference measurement (outvotes a bad frame) | `Pipeline/RollReference.swift` | `oni-measure/src/roll.rs` `measure_roll` | partial | Measured, combined and landed as one gallery step in the crates; no Prepare the Roll surface runs it | `oni-measure/tests/roll_prepare.rs::a_measure_lands_what_swift_wrote_on_every_frame_as_one_gallery_step_and_undoes_byte_for_byte` |
| Frame written onto its roll's reference | `Pipeline/RollReference.swift` | `oni-core/src/balance.rs:71` `written` | done | | `oni-core/src/balance.rs::a_frame_written_onto_its_roll_takes_the_rolls_gains_and_windows` |
| Roll previews rendered once | `UI/RollPreviews.swift` | none | missing | | no check |
| Tutorial video per step | `UI/TutorialVideo.swift` | none | missing | | no check |

## Projects

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Archive / Projects surface switch | `OpenNegativeApp.swift:259-268` `GallerySurface`, `Scene/EditorScene+Card.swift:391-471` | `library.slint` Tree `Tabs`, `oni-app/src/app/organising.rs` `surface_picked` | done | A project's grid stands on every row, never the medal filter | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand` |
| Project file, identity, newest-first members | `Pipeline/Project.swift` | `oni-library/src/project.rs`, `oni-app/src/app/organising.rs` `load_projects`, `save_projects` | done | `projects.json` in the support folder, members' paths written down at each save | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-library/src/project.rs::a_file_round_trips_through_the_disk_in_the_swift_spelling` |
| Project tree, a shared photo once per project | `Pipeline/ProjectTree.swift`, `Scene/EditorScene+Rolls.swift:286-303` | `oni-library/src/project_tree.rs`, `oni-app/src/app/organising.rs` `publish_projects` | done | A project row rings and unfolds, a photo row selects, opens on a double click | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-library/src/project_tree.rs::a_project_opens_into_its_photos_each_tagged_with_its_project` |
| New, Add to, Remove from, Delete Project | `Scene/EditorScene+Rolls.swift:117-284` | `oni-library/src/project.rs` gestures, `oni-app/src/app/organising.rs` | done | Delete Project asks with Swift's words; by project id where Swift indexes before `leave` may delete | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-library/src/project.rs::removing_a_projects_last_photo_deletes_it_and_members_are_read_by_fingerprint` |
| A project rearranges its own photos by drag | `UI/GalleryEngine.swift:230-256`, `:332-337`, `Scene/EditorScene+Rolls.swift:138-158` | `oni-app/src/app/organising.rs` `pointer_moved`, `commit_reorder`, `oni-app/src/gallery.rs` `boundary` | done | 4-point drag distance, ghost and drop line; the archive moves nothing | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand`, `oni-library/src/project.rs::a_drop_lands_before_its_target_members_first_then_the_joining_ones` |

## ONI Trash

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Delete marks the sidecar, no file moves | `Pipeline/OniTrash.swift` | `oni-app/src/app.rs` `move_to_trash`, `oni-library/src/trash.rs` | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target`, `oni-library/tests/roll8.rs::the_trash_marks_sidecars_and_never_moves_a_file` |
| Every listing leaves the trash out | `Pipeline/OniTrash.swift` | `oni-app/src/library.rs` `shown_under` | done | | `oni-app/src/library.rs::a_folder_read_twice_reads_its_files_once_and_drops_a_photo_gone`, `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |
| A delete hides at once, one gallery step | `Pipeline/OniTrash.swift` `wiringCheck` | `oni-app/src/app.rs` `gesture`, `oni-library/src/gallery_history.rs` | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |
| Restore ⇧⌘⌫ | `UI/EditorCommands.swift:131`, `Scene/EditorScene+Removal.swift:35-73` | `oni-app/src/app/organising.rs` `restore`, `open_trash`, `oni-core/src/store.rs` | done | The ONI Trash laid as a grid with its own picks, Restore N Photos, Restore All… asked | `oni-app/tests/gallery.rs::the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand` |

## Contact sheet

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| New Contact Sheet: compose the roll on its window | `Pipeline/ContactSheet.swift` | none | missing | Grid constants `ContactSheet.swift:80-94` absent | no check |
| Sheet sidecar and reference | `Pipeline/RollSettings.swift:67-77` | `oni-measure/src/window.rs` | partial | The model, the sheet's cells and reads and its place in a reset are ported; nothing composes or opens a sheet | `oni-library/tests/swift_checks.rs::a_reset_to_default_reaches_whole_rolls_and_their_sheets_never_a_neighbour` |
| Sharpening and chroma off on a sheet | `UI/DetailsPane.swift:37` | `oni-pipeline/src/lib.rs` | partial | The render leaves detail off on a sheet; the Details tab neither dims nor says so | no check |

## Import, captures and RGB passes

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Nothing applied on import | hard rule 1 | `oni-core/src/store.rs` | done | | `oni-core/src/store.rs::an_untouched_frame_gains_no_sidecar_and_a_rested_edit_writes_one` |
| Capture EXIF read in stops against the roll | `Pipeline/CaptureExposure.swift` | `oni-library/src/capture.rs` | partial | Read in the crate; no Prepare the Roll writes it | `oni-library/src/capture.rs::a_capture_reads_in_stops_against_the_settings_most_of_its_roll_shares` |
| Frame EXIF rides in its sidecar | `Pipeline/Sidecar.swift` `exifChecks` | `oni-settings/src/sidecar.rs` `set_exif`, `export_card.rs` | done | The export box's fields are written to and read from the sidecar; an empty box writes nothing | `oni-settings/tests/swift_checks.rs::the_frames_exif_rides_its_sidecar_and_an_empty_box_is_absent` |
| RGB passes: a frame from three passes | `Pipeline/RGBPasses.swift` | `oni-app/src/render.rs` `composed`, `oni-library/src/rgb_passes.rs` | partial | Composing, the triplet's moves, join and split are ported; files are never grouped by pass order | `oni-library/src/roll_folder.rs::a_triplet_moves_whole_on_its_names_or_stays_whole` |
| RGB registration: magnification and shift | `Pipeline/RGBRegistration.swift` | `oni-app/src/render.rs` `laid` | partial | A stored fit lays its pass; the fit is never measured | `oni-app/src/render.rs::three_passes_give_each_its_own_channel_and_a_registration_moves_its_pass` |
