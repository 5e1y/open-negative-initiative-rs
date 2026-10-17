# Parity: export and colour management

Swift paths are under `Sources/OpenNegative/` of the Swift app; Rust paths are under `crates/`.
Statuses are defined in [README.md](README.md). `oni-app` depends on neither `oni-export` nor
`oni-colour`, so every row below stops at its crate.

## Export

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Export is a mode of the card, target and count in four labels | `UI/ExportSurface.swift`, `Scene/EditorScene+Export.swift` | none | missing | | no check |
| Full-resolution render for export | `Pipeline/Export.swift` | `oni-export` `converted`, `oni-pipeline` | partial | Crate only | `oni-export/tests/roll8.rs::a_frame_exports_with_its_date_and_none_of_the_scans_fields` |
| TIFF 16-bit, ZIP lossless, profiled | `Pipeline/Export.swift` | `oni-export/src/tiff.rs` | partial | Crate only | `oni-export/tests/files.rs::the_tiff_is_zip_compressed_lossless_and_profiled` |
| TIFF 8-bit | `Pipeline/Export.swift` | `oni-export/src/tiff.rs` | partial | Crate only | `oni-export/tests/files.rs::an_eight_bit_tiff_is_written_as_well` |
| JPEG with its quality | `Pipeline/Export.swift` | `oni-export` | partial | Crate only | `oni-export/tests/files.rs::the_jpeg_quality_is_honoured` |
| HDR gain map (Brighter on HDR screens) | `Pipeline/GainMapContainer.swift` | `oni-export/src/gain_map.rs` | partial | Crate only | `oni-export/tests/files.rs::an_independent_reader_lifts_every_pixel_by_four` |
| Size: long edge, two fields riding one edge | `Pipeline/Export.swift` | `oni-export/src/resize.rs`, `settings.rs` | partial | Crate only | `oni-export/tests/settings.rs::the_two_size_fields_ride_the_edge_they_measure` |
| Lanczos resize, opaque to the last pixel | `Pipeline/Export.swift` | `oni-export/src/resize.rs` | partial | Crate only | `oni-export/src/resize.rs::lanczos_matches_an_independent_lanczos3_inside_the_frame` |
| Presets Instagram, Display, Print | `Pipeline/ExportPresets.swift` | `oni-export` presets | partial | Crate only | `oni-export/tests/settings.rs::every_key_writes_a_distinct_complete_state` |
| Lit preset key read off the state | `UI/ExportSurface.swift` | `oni-export` | partial | Crate only | `oni-export/tests/settings.rs::the_lit_key_is_read_off_the_state` |
| Per-photo Name, Camera, Lens, Film, ISO, Date, Artist, Copyright | `UI/ExportSurface.swift` | `oni-export/src/metadata.rs` | partial | Crate only | `oni-export/tests/settings.rs::the_metadata_keeps_the_swift_field_names` |
| Only the metadata ONI sets; no scan EXIF or serial | `Pipeline/Export.swift` | `oni-export` | partial | Crate only | `oni-export/tests/files.rs::the_jpeg_carries_the_stated_fields_only_and_its_profile` |
| Scan's EXIF read to prefill the box | `Pipeline/Export.swift` | `oni-export` | partial | Crate only | `oni-export/tests/roll8.rs::the_scans_own_exif_is_read_for_the_box` |
| Names cannot fabricate a path; overwrite flagged | `Pipeline/Export.swift` | `oni-export` settings | partial | Crate only | `oni-export/tests/settings.rs::names_cannot_fabricate_a_path` |
| Source never overwritten | `Pipeline/Export.swift` | `oni-export` | partial | Crate only | `oni-export/tests/roll8.rs::the_source_is_never_overwritten` |
| Size estimate | `UI/ExportSurface.swift` | `oni-export` estimate | partial | Crate only | `oni-export/tests/files.rs::the_estimate_weighs_the_same_bytes_as_the_file` |
| Export previews in the card | `Pipeline/ExportPreviews.swift` | `oni-export` preview | partial | Crate only | no check |
| Settings saved as a preference | `Pipeline/Export.swift:428` | `oni-export/src/settings.rs:108` | partial | Crate only | `oni-export/tests/settings.rs::the_preference_text_is_the_swift_encoders` |
| ProPhoto in TIFF only | `Pipeline/Export.swift` | `oni-export` | partial | Crate only | `oni-export/tests/settings.rs::prophoto_is_written_in_tiff_only` |
| Paper Correction laid by the export on a paper profile | `Pipeline/PaperCorrection.swift` | `oni-colour/src/paper.rs` | partial | Crate only | `oni-colour/tests/checks.rs::paper_correction_is_laid_on_a_paper_alone` |
| Export through CGImageDestination | `Pipeline/Export.swift` | own TIFF and JPEG writers | done | Different writer by design; files read back by independent readers | `oni-export/tests/files.rs::the_tiff_carries_its_structure_and_the_stated_fields_only` |

## Colour spaces and ICC

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Space: sRGB, Display P3, Adobe RGB, ProPhoto | `Pipeline/Export.swift:18`, `UI/ExportSpaceField.swift:27` | `oni-colour/src/space.rs:36` | partial | Crate only | `oni-colour/tests/oracle.rs::the_matrix_spaces_encode_as_lcms2_converts` |
| Every format declares its space's profile | `Pipeline/Export.swift` | `oni-export` | partial | Crate only | `oni-export/tests/files.rs::every_format_declares_the_profile_of_its_space` |
| Working space Rec. 2020 | `Pipeline/RawDecode.swift:11` | `oni-colour/src/working.rs` | done | | `oni-colour/tests/oracle.rs::the_working_space_is_rec2020_on_the_pcs` |
| Load a Profile…, installed profiles listed | `UI/ExportSpaceField.swift`, `Pipeline/OutputProfile.swift` | `oni-colour/src/profile.rs:175-199` | partial | Discovery only; no picker | `oni-colour/tests/checks.rs::a_profile_gone_is_reported_not_replaced` |
| ICC conversion with own BPC and clip | `Pipeline/OutputProfile.swift` | `oni-colour/src/legs.rs`, `icc_lut.rs` | partial | Crate only | `oni-colour/tests/oracle_tables.rs::the_export_legs_agree_with_lcms2_s_black_point_compensation` |
| v2 and v4 LUT tables | `Pipeline/OutputProfile.swift` | `oni-colour/src/icc_lut.rs` | partial | Crate only | `oni-colour/tests/oracle_tables.rs::v4_tables_run_their_five_elements_in_the_icc_order` |
| Colour-only toggle (screen black) | `UI/ExportSpaceField.swift` | `oni-colour` | partial | Crate only | `oni-colour/tests/checks.rs::colour_only_drops_the_paper_black_to_the_screen_s` |
| A profile picked is the profile kept and shown | `UI/ExportSpaceField.swift` | none | missing | | no check |
| Working Space chip | `UI/WorkingSpaceChip.swift` | none | missing | | no check |

## Proof, paper and screen

| Feature | Swift | Rust | Status | Gap | Check |
|---|---|---|---|---|---|
| Soft proof cube (33) into the export space | `Pipeline/Proof.swift:53-54` | `oni-colour/src/proof.rs:37` | partial | Crate only | `oni-colour/tests/checks.rs::the_proof_clips_what_the_export_space_cannot_hold` |
| Paper proof shows the paper white | `Pipeline/Proof.swift` | `oni-colour/src/proof.rs` | partial | Crate only | `oni-colour/tests/checks.rs::a_bluish_paper_shows_its_white_as_printed` |
| Paper Correction block and Auto key | `UI/ColorPane.swift:82` | `oni-colour/src/paper.rs` | partial | Crate only | `oni-colour/tests/checks.rs::the_automatic_key_solves_the_paper_s_white` |
| Get Paper Profiles… sheet: install, uninstall, maker's page | `UI/PaperProfilesSheet.swift`, `Pipeline/PaperSource.swift` | `oni-colour/src/paper_source.rs` | partial | Crate only | `oni-colour/tests/checks.rs::a_link_s_install_is_recorded_and_undone` |
| No paper profile shipped, links only | product rule | `oni-colour/src/paper_source.rs` | done | | `oni-colour/src/paper_source.rs::the_sources_are_links_and_a_single_offers_itself` |
| Screen gamut stripes, lime, 9 px, 2 s | `Pipeline/ScreenGamut.swift:14-29` | `oni-colour/src/gamut.rs:11-29` | partial | Crate only | `oni-colour/tests/checks.rs::the_stripe_tint_is_the_key_s_lime` |
| Display matrix and dither to the screen | `UI/MetalImageView.swift` | `oni-app/src/display.rs` | done | | `oni-app/src/display.rs::the_kernel_writes_what_its_cpu_reference_writes` |
| Export, proof and stripes hold on real frames | `Pipeline/Proof.swift`, `ScreenGamut.swift` | `oni-colour/tests/roll8.rs` | partial | Crate only; properties, not Swift's output | `oni-colour/tests/roll8.rs::every_frame_of_roll_8_exports_proofs_and_stripes_as_measured` |
