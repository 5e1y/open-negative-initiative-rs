# Reviews: slop, memory, Swift parity, the phone

Twenty-seven read-only reviewers (12 slop, 5 memory, 8 Swift parity, 2 phone) read the workspace.
Every finding acted on was checked in the code first; this file keeps what is true, what was wrong,
and what is left, most noticed first.

## Done

| Finding | Task |
|---|---|
| Dead items: `max_params`, `THUMB_SIDE`, `zoom_over_fit`, `resting_position`, `circle`, `archive_paths`, `is_roll_field`, the Gpu area wrappers, `missing_paths`, `is_listing`, `maker_crop`, `tab_title`, `beyond_working`, `rotated_bounding_size`, `ANCHOR_CLAMP`, `HUNT_REACH`, `announce`, `cert_pem`/`key_pem`, `covering` | #92 |
| TIFF doc said unassociated alpha, wrote associated | #92 |
| Compare's before clip had no place (centred by Slint), its guide followed a hovering pointer, the kept before ignored the photo | #94 |
| Report a Bug never sent: typing broke the message's binding | #94 |
| Icon-only keys two points left of centre: a gap for an empty word | #94 |
| Eight tabs bound one way lost their binding at the first click (export space, format, crop ratio, roll film, capture, film type, encoding, dye family) | #94 |
| `set_label` read every file as a RAW mosaic: a TIFF's sidecar could be rewritten wrong | #94 |
| Decode threads piled up per frame walked (0.4-0.7 GB each); full builds duplicated; the base loupe's full scan unbounded | #93 |
| The editor's full frame, fitted copy and measures held in the gallery; Compare's before never freed; the console read the log forever | #97 |
| The base loupe's full scan waited behind every roll tile | #91 |
| The library column had no thumbnails, no layout key, never re-centred | #95 |

## False or not worth it

- `render::Previews` "unused": it is the open frames' preview cache, imported by `app.rs`.
- `track::DOUBLE_CLICK` "dead": the editors and the export card use it.
- `Levels::resting_mid` returning 0.67 in both arms: a faithful copy of Swift's `restingMid`.
- The thumbnail cap of 2,500 tiles: Swift's `memoryLimit`; bounded, not a leak (see the plan for bytes).
- `Notches` growing: about 4 MB an hour of continuous dragging.

## Left, by how much a user notices

### Data (touches the user's files: adversarial review before merging)

1. RAW orientation never applied (`user_flip = 0`): a portrait RAW opens sideways and its crop
   fractions land elsewhere than in Swift. Moves renders of rotated files: owner's call.
2. A failed sidecar rename in `roll_folder::move_items` leaves a photo without its settings.
3. Corrupt `favourites.json`, `roots.json`, `projects.json` or the flat-field index read as empty, then
   overwritten.
4. `set_label`, `set_exif`, `set_roll_draft` overwrite an unreadable sidecar (also in Swift).
5. EXIF capture dates read only from TIFF-like and JPEG files (CR3, HEIC, RAF, PNG get none), so
   fingerprints differ from Swift's on those formats.

### Editor behaviour (Swift wins)

1. Black point readout sign flipped; Dye Separation reads 0.60 instead of 60 %.
2. Source Balance double-click goes to the roll's stops instead of 0.
3. B&W frames keep the colour ranges on Contrast, White and Black points; the density ceiling's track is
   the RAW range on every file.
4. A never-balanced frame does not open with its measuring window proposed; the window is never
   disarmed (leaving Crop, auto balance, Return).
5. Return does nothing in Crop; ⌘K toggles instead of opening Crop; the last panel is recorded in
   fewer places.
6. De-dust does not leave the Scanner face; correction numbers never restart per photo; the brush
   size is not remembered.
7. Space does not leave the Lab; ⌘A does nothing in the editor; the Scanner Emulator menu item is
   always dark.
8. A preset with automatic balance asks the strategy question Swift never asks.

### Gallery and column

1. A click on a folder, roll or project row folds it instead of opening its first photo; no
   chevron-only fold target, no rename in place, no tree context menu, no pinned header.
2. The gallery does not land centred on the open photo when coming back from the editor.
3. Band headers cannot be renamed in place.
4. Dropping files from the Finder onto the gallery does nothing.

### Prepare the Roll

1. "Use This Frame as Flat-Field" and "Load…" are disabled.
2. The measure and the previews skip a custom flat-field mask and the dust corrections.
3. The remembered flat-field sliders are neither applied nor kept.
4. A flagged frame that fails to decode is still written.

### Proofing and export

1. The Custom space lists only profiles found by walking folders, not ColorSync's: it may land on
   "pending" on a Mac whose printer profiles live elsewhere.
2. Preset and profile lists sort accented names after "Z".

### The phone

1. Import from the phone (LocalSend) is not wired on the desktop: the phone's Send stays dark.
2. Previews are never sent to the phone.
3. A phone pairing on a spent code believes it is paired.
4. The QR code carries one address; a DHCP change needs a new pairing.
5. `host.key` and `trusted.txt` written without atomic rename or owner-only permissions.
6. The iPhone build clone is behind two FFI renames.

### Slop worth a pass

- Errors swallowed with `let _ =` on preference and index writes; a failed texture import leaves the
  canvas blank with no message.
- Copies: three hex encoders in the link crates, `move_out` in two crates, the stem helpers, the
  geometry window code in `oni-measure` and `oni-settings`, the matrices in `oni-decode` and
  `oni-colour`.
- The oni-library timing test (`the_day_is_computed_once_per_frame`) flakes under load.
