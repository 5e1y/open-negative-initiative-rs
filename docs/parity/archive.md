# Parity: archive, gallery and rolls

Swift paths are under `Sources/OpenNegative/` of the Swift app; Rust paths are under `crates/`.
Statuses are defined in [README.md](README.md).

## Library, roots and folders

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Open a folder from a picker | `UI/RootPicker.swift`, `UI/EditorCommands.swift:109` | `oni-app/src/app.rs` `pick`, `add`, `system.rs` `pick_folder` | done | A folder opens on the gallery, nothing in the editor; a file opens its folder then itself; a folder reached through a link reads on its real path | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once`, `oni-app/src/library.rs::a_folder_reached_through_a_link_lists_the_photos_on_its_real_path` |
| Several roots, surveyed off the main thread | `Pipeline/Roots.swift` | `oni-library/src/roots.rs` | partial | Crate surveys many; the app holds one folder | `oni-library/src/roots.rs::roots_never_hold_a_photo_twice` |
| Add Folder… on the empty archive | `UI/RootPicker.swift` | none | missing | | no check |
| Former library offered per volume (RootsCatchUp) | `UI/RootsCatchUp.swift` | `oni-library/src/roots.rs` | partial | Model only | `oni-library/src/roots.rs::a_former_library_is_offered_per_volume_never_the_home_itself` |
| SQLite index as a disposable cache | `Pipeline/LibraryIndex.swift` | `oni-library/src/index.rs`, `oni-app/src/library.rs` `read_rows` | done | The index lives in the caches; a second read reads no file again | `oni-app/src/library.rs::a_folder_read_twice_reads_its_files_once_and_drops_a_photo_gone`, `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Library reload, invented and waiting rows | `Pipeline/Library.swift:43-44` | `oni-library/src/library.rs:56-57` | partial | Crate only | `oni-library/src/library.rs::an_invented_row_is_dropped_and_a_waiting_one_kept` |
| Relink a moved original by fingerprint | `Pipeline/Relink.swift` | `oni-library/src/relink.rs` | partial | Crate only; Recheck Originals unwired | `oni-library/src/relink.rs::two_files_of_one_size_part_on_their_sample_and_a_renamed_one_keeps_its_print` |
| Read the Folders Again | `UI/EditorCommands.swift:123` | `oni-app/src/app.rs` `read_again` | done | | no check |
| Show the Selected Photo, Show in Finder | `UI/EditorCommands.swift:125`, `UI/GalleryView.swift` | `oni-app/src/app.rs` `show_selected`, `gallery.rs` `reveal` | partial | Show the Selected Photo done; Show in Finder absent | `oni-app/src/gallery.rs::reveal_moves_the_least_to_show_a_cell_and_not_at_all_when_it_shows` |
| Opened folder remembered across launches | `Scene/EditorScene+Removal.swift:142` `openedFolder` | `oni-app/src/preferences.rs`, `app.rs` `open_remembered` | done | | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Every path from AppFolders, sandboxed under checks | `Pipeline/AppFolders.swift` | `oni-app/src/render.rs:75` | partial | Support folder named; no per-process sandbox for tests | no check |

## Folder tree (navigator)

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Tree of folders then photos, newest first | `UI/TreeEngine.swift`, `Pipeline/ArchiveTree.swift` | `oni-app/ui/library.slint` Tree, `oni-library/src/archive_tree.rs` | done | | `oni-library/src/folders.rs::the_tree_opens_on_its_folders_then_its_photos_and_an_empty_folder_by_its_birth` |
| Unfold on click, chevron | `UI/TreeRowChrome.swift`, `UI/FolderNavigator.swift` | `library.slint` `row-clicked` | done | Chevron aspect `TreeRowChrome.swift:39` not checked | `oni-app/src/library.rs::roll_8_opens_as_one_named_roll_of_ten_photos` |
| Navigator walks up to Computer | `Pipeline/Navigator.swift` | none | missing | | no check |
| Favourites, favourites-only | `Pipeline/Favourites.swift`, `UI/FolderNavigator.swift:21` | `oni-library/src/favourites.rs` | partial | Crate only | `oni-library/src/favourites.rs::favourites_star_beside_each_other_and_follow_a_rename` |
| Navigator state remembered (height, base, unfolded) | `UI/FolderNavigator.swift:15-22` | none | missing | | no check |
| Folder menu: Open, Rename…, New Folder Inside, Show in Finder | `UI/FolderNavigator.swift`, `Scene/EditorScene+Gallery.swift` | none | missing | | no check |
| Folder menu: Make a Roll, Back to a Folder | `Scene/EditorScene+Gallery.swift` | `oni-library/src/roll_folder.rs` | partial | Crate only | `oni-library/src/roll_folder.rs::a_roll_folder_takes_its_photos_and_gives_them_back` |
| ONI Trash row | `UI/FolderNavigator.swift:18` | `oni-library/src/trash.rs` | partial | Crate lists; no row | `oni-library/src/trash.rs::the_trash_lists_its_folders_latest_deletion_first` |
| A roll row shows its default stock | `UI/TreeRowChrome.swift` | none | missing | | no check |
| Drop over the tree lands where its strips say | `UI/ListPicking.swift` `TreeDropTarget` | none | missing | Archive moves nothing by hand; project drops only | no check |
| Hide the Library ⌥⌘[ | `UI/EditorCommands.swift:147` | `app.slint` menu bar | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |

## Gallery grid

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Tiles on the Swift grid, columns follow the width | `UI/GalleryLayout.swift`, `UI/GalleryView.swift` | `oni-app/src/gallery.rs`, `library.slint` Grid | done | | `oni-app/src/gallery.rs::the_tile_follows_the_width_and_the_cells_sit_on_the_swift_grid` |
| Only rows in view are asked for | `UI/GalleryLayout.swift:51` overscan | `oni-app/src/gallery.rs:65` | done | | `oni-app/src/gallery.rs::a_scroll_asks_only_for_the_rows_in_view` |
| Scroll by the wheel only | `DesignSystem/DSScrollAdjust` | `oni-app/tests/scroll.rs` | done | | `oni-app/tests/scroll.rs::a_surface_scrolls_by_the_wheel_and_never_by_a_drag` |
| Headings per roll and day | `Pipeline/Film.swift`, `UI/GalleryView.swift` | `oni-library/src/film_group.rs`, `library.slint` headings | done | | `oni-library/src/film_group.rs::rolls_bucket_by_id_and_days_by_local_midnight` |
| ⌘ and ⇧ click selection | `UI/ListPicking.swift` | `library.slint` `tile-clicked` | done | | no check |
| Double-click opens in the editor | `UI/GalleryView.swift` | `library.slint` `tile-opened` | done | | no check |
| ⌘A, arrows, Return, Space | `UI/GalleryEngine.swift:425-439` | none | missing | | no check |
| Zoom the grid ⌘− ⌘+ | `UI/EditorCommands.swift:138-142` | `oni-app/src/app.rs:1129` | partial | Ladder of two widths, not Swift's | no check |
| Thumbnails: 320 px, disk cache, render version 11 | `Pipeline/Thumbnails.swift:15-21` | `oni-app/src/thumbnails.rs`, `app.rs` `paint` | partial | 320 px JPEGs at 0.6 named by fingerprint, drawn only when absent; an unconverted frame renders through the pipeline where Swift shows the embedded scan | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Thumbnails proofed into the export space | `Pipeline/Thumbnails.swift:13` | none | missing | | no check |
| Thumbnail refresh 2 s after an edit | `Pipeline/Thumbnails.swift:21` | `oni-app/src/app.rs` `schedule_redraw` | done | | `oni-app/tests/archive.rs::a_folder_opens_on_the_gallery_and_its_thumbnails_are_drawn_once` |
| Triplet of RGB passes shows its lead | `UI/TileLayers.swift:256` | `oni-library/src/index.rs` | partial | Index only | `oni-library/src/index.rs::a_triplet_draws_as_its_lead_and_its_passes_show_once_it_is_gone` |
| Cell menu: Paste Settings, Reset Settings, Reset to Default… | `UI/GalleryView.swift:354-433` | none | missing | Core gestures exist | no check |
| Cell menu: medals | `UI/GalleryView.swift:354-433` | keys 0-3 | partial | No context menu | no check |
| Cell menu: New Roll from, Ungroup | `UI/GalleryView.swift:354-433` | none | missing | | no check |
| Cell menu: New Project from, Add to, Remove from Project | `UI/GalleryView.swift:354-433` | none | missing | | no check |
| Cell menu: Export…, Show in Finder, Move to ONI Trash | `UI/GalleryView.swift:354-433` | none | missing | | no check |
| Empty states | `UI/GalleryView.swift` | none | missing | | no check |
| Status bar with medal picker | `UI/GalleryView.swift` | none | missing | | no check |

## Labels and filter

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Medal written to the sidecar alone | `Pipeline/Medal.swift`, `Pipeline/Sidecar.swift` | `oni-app/src/app.rs:932`, `oni-settings/src/sidecar.rs` | done | | `oni-settings/tests/real_files.rs::label_trash_and_draft_touch_nothing_else_and_keep_the_revision` |
| Medal names and order | `Pipeline/Medal.swift` | `oni-settings/src/modes.rs` | done | | `oni-settings/src/modes.rs::a_medal_is_written_by_name_and_an_unknown_one_refused` |
| Filter: All / medals, ⌘L | `UI/EditorCommands.swift:187` | `app.rs` `cycle_filter`, `toolbar.slint` filter word | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |

## Info sidebar

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Roll title field | `UI/InfoSidebar.swift` | `library.slint` `roll-title` | partial | Shown, not editable | no check |
| Film Stock, Select Roll Type, stock picker sheet | `UI/InfoSidebar.swift`, `UI/StockPickerSheet.swift` | `library.slint` `stock` | partial | Shown only; 41 stocks in `oni-settings/src/film.rs` | `oni-settings/src/film.rs::the_catalogue_holds_unique_keys` |
| Film stock icons | `DesignSystem/DSIcon` `FilmStockIcon` | none | missing | | no check |
| Shot Date with clear key, clamped typing | `UI/InfoSidebar.swift` | `oni-settings/src/film.rs` | partial | Model only | `oni-settings/src/film.rs::a_shot_date_reads_back_to_the_bit` |
| Camera Body field | `UI/InfoSidebar.swift` | none | missing | | no check |
| Description field, persisted to the roll marker | `UI/InfoSidebar.swift` | `library.slint` `description` | partial | Typed text is never saved | `oni-settings/src/film.rs::a_marker_round_trips_with_its_date_in_reference_seconds` (model only) |
| Project info: name, stocks counted by photo, description | `UI/InfoSidebar.swift` | none | missing | | no check |

## Rolls

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Rolls group by reference id, days by local midnight | `Pipeline/Film.swift` | `oni-library/src/film_group.rs` | done | | `oni-library/tests/roll8.rs::roll_membership_reads_the_reference_id_each_sidecar_names` |
| New Roll from Selection ⇧⌘G (folder on disk, ⌘Z) | `Pipeline/RollFolder.swift`, `Scene/EditorScene+Rolls.swift` | `oni-app/src/app.rs` `new_roll`, `followed`, `oni-library/src/roll_folder.rs` | done | The heading is not offered for renaming | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target`, `oni-library/src/roll_folder.rs::a_roll_folder_takes_its_photos_and_gives_them_back` |
| Ungroup ⌥⇧⌘G | `Scene/EditorScene+Rolls.swift` | none | missing | | no check |
| Fresh roll takes the lowest free number | `Pipeline/Film.swift:144` | `oni-library/src/film_group.rs` | done | | `oni-library/src/film_group.rs::a_fresh_roll_takes_the_lowest_free_number` |
| Reset to Default… reaches whole rolls and sheets | `Pipeline/RollSettings.swift:114` `resetReach` | `oni-core/src/store.rs:602` | partial | Resets targets; reach not computed | no check |
| Paste to Roll writes the whole roll | `Scene/EditorScene+Paste.swift` | `oni-app/src/app.rs` `paste_to_roll`, `oni-core/src/store.rs` | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target`, `oni-core/tests/roll8.rs::paste_to_roll_is_one_editor_step_over_the_whole_roll` |

## Prepare the Roll

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Surface with steps Film, Flat-Field, Base, Flags, Window | `UI/RollReferenceSurface.swift` | none | missing | | no check |
| Steps saved in the frames' sidecars (RollDraft) | `Pipeline/RollDraft.swift` | `oni-settings/src/sidecar.rs` | partial | Field round-trips; nothing writes it | `oni-settings/tests/real_files.rs::label_trash_and_draft_touch_nothing_else_and_keep_the_revision` |
| Frame card in each scan's shape | `UI/RollReferenceSurface.swift` `RollFrameView` | none | missing | | no check |
| Pick and flag frames one click at a time | `UI/RollReferenceSurface.swift` `RollPicking` | none | missing | | no check |
| Clear, Clear Flags, Shift by One File, Previous / Next | `UI/RollReferenceSurface.swift` | none | missing | | no check |
| MEASURE N FRAMES, NEXT / SKIP, Back | `UI/RollReferenceSurface.swift` | none | missing | | no check |
| Base pipette and its radius | `Pipeline/RollReference.swift:22-24`, `Pipeline/Pipette.swift` | none | missing | | no check |
| Roll reference measurement (outvotes a bad frame) | `Pipeline/RollReference.swift` | none | missing | Read and applied, never computed | no check |
| Frame written onto its roll's reference | `Pipeline/RollReference.swift` | `oni-core/src/balance.rs:71` `written` | done | | `oni-core/src/balance.rs::a_frame_written_onto_its_roll_takes_the_rolls_gains_and_windows` |
| Roll previews rendered once | `UI/RollPreviews.swift` | none | missing | | no check |
| Tutorial video per step | `UI/TutorialVideo.swift` | none | missing | | no check |

## Projects

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Archive / Projects surface switch | `OpenNegativeApp.swift` `GallerySurface` | none | missing | | no check |
| Project file, identity, newest-first members | `Pipeline/Project.swift` | `oni-library/src/project.rs` | partial | Crate only | `oni-library/src/project.rs::a_file_round_trips_through_the_disk_in_the_swift_spelling` |
| Project tree, a shared photo once per project | `Pipeline/ProjectTree.swift` | `oni-library/src/project_tree.rs` | partial | Crate only | `oni-library/src/project_tree.rs::a_project_opens_into_its_photos_each_tagged_with_its_project` |
| New, Add to, Remove from, Delete Project | `UI/GalleryView.swift`, `Scene/EditorScene+Gallery.swift` | none | missing | | no check |
| A project rearranges its own photos by drag | `UI/ListPicking.swift` | none | missing | | no check |

## ONI Trash

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Delete marks the sidecar, no file moves | `Pipeline/OniTrash.swift` | `oni-app/src/app.rs` `move_to_trash`, `oni-library/src/trash.rs` | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target`, `oni-library/tests/roll8.rs::the_trash_marks_sidecars_and_never_moves_a_file` |
| Every listing leaves the trash out | `Pipeline/OniTrash.swift` | `oni-app/src/library.rs` `shown_under` | done | | `oni-app/src/library.rs::a_folder_read_twice_reads_its_files_once_and_drops_a_photo_gone`, `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |
| A delete hides at once, one gallery step | `Pipeline/OniTrash.swift` `wiringCheck` | `oni-app/src/app.rs` `gesture`, `oni-library/src/gallery_history.rs` | done | | `oni-app/tests/gestures.rs::the_menus_name_what_they_do_and_the_gestures_reach_every_target` |
| Restore ⇧⌘⌫ | `UI/EditorCommands.swift:131` | `oni-core/src/store.rs:708` | partial | Core and ⌘Z only: no ONI Trash view to select in | no check |

## Contact sheet

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| New Contact Sheet: compose the roll on its window | `Pipeline/ContactSheet.swift` | none | missing | Grid constants `ContactSheet.swift:80-94` absent | no check |
| Sheet sidecar and reference | `Pipeline/RollSettings.swift:67-77` | `oni-settings/src/frame.rs` | partial | Model field only | no check |
| Sharpening and chroma off on a sheet | `UI/DetailsPane.swift:37` | none | missing | | no check |

## Import, captures and RGB passes

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Nothing applied on import | hard rule 1 | `oni-core/src/store.rs` | done | | `oni-core/src/store.rs::an_untouched_frame_gains_no_sidecar_and_a_rested_edit_writes_one` |
| Capture EXIF read in stops against the roll | `Pipeline/CaptureExposure.swift` | none | missing | Stored stops applied; never computed | no check |
| Frame EXIF rides in its sidecar | `Pipeline/Sidecar.swift` `exifChecks` | `oni-library/src/exif.rs` | partial | Read for the date only | `oni-library/src/exif.rs::a_tiff_and_a_jpeg_give_their_original_date_in_either_byte_order` |
| RGB passes: a frame from three passes | `Pipeline/RGBPasses.swift` | `oni-pipeline` rgb passes, `oni-render` kernel | partial | Render only; no pass order read (`RGBPasses.swift:119`) | `oni-render/tests/kernels.rs::rgb_passes` |
| RGB registration: magnification and shift | `Pipeline/RGBRegistration.swift` | none | missing | | no check |
