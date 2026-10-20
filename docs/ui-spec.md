# ONI interface in Slint: porting specification

The interface of the Swift app (`Sources/OpenNegative/UI`, `Scene`, `DesignSystem`) inventoried item by
item, each with the Slint construct it maps to and what makes it hard. The target is an identical copy
of the current interface: same layout, same tokens, same gestures, same timings. The Swift code is the
reference; where this document and the Swift code disagree, the Swift code wins.

A proof of concept tested the riskiest pieces first: a keycap slider driving one `Stage47` field, on a
real roll #8 frame, rendered headlessly. `oni-app` now carries each of them; the findings are in section 7.

Sections:

1. [Slint 1.18.1: what it offers](#1-slint-1181-what-it-offers)
2. [Architecture: how the Swift layers map](#2-architecture-how-the-swift-layers-map)
3. [Editor fluidity](#3-editor-fluidity-editorstore-storereader-dsslice)
4. [Inventory](#4-inventory)
5. [Design system: tokens, components, keycaps](#5-design-system)
6. [Risk register](#6-risk-register)
7. [Proof of concept](#7-proof-of-concept)
8. [Decisions](#8-decisions)
9. [Not ported](#9-not-ported)

---

## 1. Slint 1.18.1: what it offers

These findings come from the crate sources (`~/.cargo/registry/src/*/i-slint-*-1.18.1`). They were
read, not taken from memory.

**Version.**
- 1.18.1 is the latest on crates.io.
- It needs Rust 1.92; the local toolchain is 1.97.
- Licence: `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`. ONI's
  `GPL-3.0-or-later` can use it under the GPL-3.0 option, but a binary that links Slint is then
  GPL-3.0-only in practice. The Royalty-free 2.0 option also covers desktop applications, with an
  attribution requirement. Which option ONI takes is the owner's decision, not this document's.

**Features of the `slint` crate.**
- Default features: `std`, `backend-default`, `renderer-femtovg`, `renderer-software`,
  `accessibility`, `compat-1-2`, `system-tray`.
- Optional features: `renderer-skia` (plus `-opengl`, `-vulkan`), `renderer-femtovg-wgpu`,
  `renderer-vello`, `software-renderer-path`, `backend-winit` (plus `-x11`, `-wayland`),
  `unstable-winit-030`, `unstable-wgpu-29`, `unstable-wgpu-30`, `system-testing`.

**Sharing wgpu with oni-render (`unstable-wgpu-30`).**
- `oni-render` pins `wgpu = "30.0.1"`, and Slint's `unstable-wgpu-30` is built against the same major
  version, so one `wgpu::Device` can serve both.
- Two ways to wire it:
  - `slint::BackendSelector::new().require_wgpu_30(WGPUConfiguration::Manual { instance, adapter,
    device, queue })` hands Slint the device that oni-render created.
  - `WGPUConfiguration::Automatic(WGPUSettings)` lets Slint create the device, and
    `Window::set_rendering_notifier` hands it back as `GraphicsAPI::WGPU30 { device, queue, .. }`.
- `slint::Image::try_from(wgpu::Texture)` imports a texture with no copy, but it **accepts only
  `Rgba8Unorm` or `Rgba8UnormSrgb`**. The texture also needs usage `TEXTURE_BINDING |
  RENDER_ATTACHMENT`. Anything else is refused with `TextureImportError::{InvalidFormat, InvalidUsage}`.
  - The canvas therefore needs one last stage that writes 8-bit display values into such a texture.
  - The Swift drawable is `.bgra8Unorm` too (`MetalImageView`), so this matches what the app shows
    today.
  - Every texture upstream stays `rgba32float`. Hard rule 2 holds, because the 8-bit step is the
    display, exactly as in Swift.
- Only `renderer-femtovg-wgpu` and the skia-wgpu renderer draw imported textures.
  - femtovg-wgpu supports only `WGPU30`.
  - The software renderer returns `None` for a wgpu image, so headless tests cannot see an imported
    texture. The PoC goes through `Image::from_rgba8` for that reason (section 7).

**Custom drawing.** Slint has no immediate-mode canvas or per-item shader API. What it does have:
- `Path` with SVG commands or `MoveTo`/`LineTo`/`ArcTo`/`CubicTo` elements.
  - It is filled or stroked; it needs `software-renderer-path` on the software renderer, and it is
    not supported in line-by-line mode.
- `Rectangle` with per-corner radii, `border-width`, and `drop-shadow-*`.
  - Corners are circular arcs, never squircles.
- Gradients: `@linear-gradient`, `@radial-gradient`, `@conic-gradient`.
- `Image` from any RGBA buffer or a wgpu texture, with `image-fit`, `image-rendering`, `colorize`
  and nine-slice.
- `Window::set_rendering_notifier` (`BeforeRendering` / `AfterRendering`) to draw with wgpu under or
  over the scene. It works with femtovg-wgpu and skia-wgpu only.
- Measured gaps in the **software renderer** (read in `i-slint-renderer-software-1.18.1/lib.rs`):
  - `draw_box_shadow` is a TODO, so there are no drop shadows.
  - `combine_clip` ignores the radius, so `clip: true` on a rounded rectangle clips square.
  - `rotate` and `scale` are TODOs, so `transform-*` is not drawn.
  - The GPU renderers draw all three. Headless tests on the software renderer therefore cannot judge
    shadows, rounded clips or transforms.

**Menus.**
- Slint's `MenuBar` (in a `Window`) with `Menu`, `MenuItem` (`title`, `enabled`, `checkable`,
  `checked`, `icon`, `shortcut: @keys(Control + N)`, `activated`) and `MenuSeparator` is rendered
  **natively through muda** on macOS and Windows.
  - `i-slint-backend-selector` enables the winit backend's `muda` feature (muda 0.19), and
    `SLINT_NO_MUDA` turns it off.
- `ContextMenuArea { Menu { … } }` also goes through muda, so context menus are native NSMenus.
  - Shortcuts are refused in context menus: "MenuItem shortcuts are currently only supported in the
    MenuBar".
- A separate `muda` dependency (0.21.2 is current) is needed only for what Slint's `MenuBar` lacks:
  - the macOS application menu's standard items (About, Services, Hide, Quit);
  - the Window and Help menu roles;
  - the system pop-up that `DSPicker` drops (section 5).

**Accessibility.**
- AccessKit through winit: NSAccessibility on macOS, UIA on Windows.
- Per element: `accessible-role` (slider, button, checkbox, tab, list, …), `accessible-label`,
  `-value`, `-value-minimum/-maximum/-step`, `-enabled`, `-checked`, `-description`, and the actions
  `accessible-action-increment/decrement/set-value/default`.
- The Swift app declares about two dozen accessibility modifiers on its custom controls (`DSSlider`'s
  label, value and adjustable action; `DSTrackHandle`'s label and value; reset buttons), and SwiftUI
  supplies the rest. In Slint every custom component must declare all of it by hand, or a screen
  reader sees nothing.

**Text.**
- Fonts are embedded with `import "font.ttf"`; Inter and Enriqueta both load.
- Properties: `font-family`, `-size`, `-weight`, `-italic`, `letter-spacing`, `overflow: elide`,
  `wrap`, plus `StyledText` for markup.
- **There is no OpenType feature setting.** `DS.Typo.value` is `.monospacedDigit()`, and `TreeLayers`
  writes tabular figures. Slint cannot ask Inter for `tnum`.
  - Mitigation: ship an Inter instance with `tnum` frozen into the default figures (`fonttools
    feaLib`/`pyftfeatfreeze`) under a second family name, used by the value face only.
- The renderers rasterise through fontique and parley (femtovg) or skia. Neither is Core Text, so
  glyph spacing and hinting will differ slightly from the Swift app.

**Input.**
- `TouchArea` offers `pointer-event(PointerEvent{button, kind, modifiers})`, `moved`, `clicked`,
  `double-clicked`, `scroll-event(PointerScrollEvent{delta-x, delta-y, modifiers})`, `has-hover`,
  `mouse-x/y`, `pressed-x/y` and `mouse-cursor`.
- What it does not offer:
  - **No click count.** `double-clicked` fires on the second *release*, while AppKit's
    `clickCount == 2` fires on the second *press*.
  - **No `NSEvent.hasPreciseScrollingDeltas` and no `phase`**, so a wheel and a trackpad cannot be
    told apart.
  - **No magnify (pinch) or rotate gesture** reaches a `TouchArea`.
  - **No force touch.**
- `FocusScope` gives `key-pressed`/`key-released` with `KeyEvent{text, modifiers, repeat}`; there is
  no window-wide key monitor.
- Default double-click interval: 500 ms (`Platform::click_interval`).

**Windows.**
- One `Window` component per window, each with its own `show()`.
- No native sheets: a sheet is either an in-window overlay or a second `Window`, and winit has no
  sheet-parent API.
- `PopupWindow` gives in-window popovers.
- Window chrome: `no-frame` hides the frame, and winit's macOS extensions give a transparent title bar
  with full-size content (`WindowAttributesExtMacOS::with_titlebar_transparent`,
  `with_fullsize_content_view`), reachable through `slint::winit_030` (`unstable-winit-030`).

---

## 2. Architecture: how the Swift layers map

| Swift layer | Slint/Rust counterpart | Notes |
|---|---|---|
| `OpenNegativeApp` (`App`, `WindowGroup`, `.commands`) | `fn main`: one `MainWindow` (Slint), `MenuBar` inside it, the console as a second `Window` | `Window` and `MenuBar` live in `.slint`; their callbacks are wired in Rust. |
| `EditorScene` and its `Scene/*` extensions (actions, history, disk, propagation, presets, …) | A Rust `Scene` struct owning the state, with one module per Swift extension | Logic only, no UI. It is ported from Swift and never re-thought. |
| `EditorStore` (`ObservableObject`, `@Published settings`) | A Slint `global Store` whose properties are the fields the panel draws, written only by Rust | See section 3. |
| SwiftUI views (`UI/*`) | `.slint` components, one file per Swift view | Every view below has a row in section 4. |
| `DS*` components and tokens | `ui/ds/*.slint` with an `export global DS` generated from `DSTokens.swift` | Section 5. |
| `MetalImageView` (`MTKView` + `CIContext`) | An oni-render wgpu texture imported as `slint::Image` (femtovg-wgpu) | Section 4.5. |
| AppKit pieces (`NSViewRepresentable`: key monitor, scroll catcher, layer hosts, `NSOpenPanel`) | winit and the Rust side: `rfd` for panels; winit for raw scroll phases; custom rendering for layer grids | Each one is listed with its risk. |

**One rule carries over unchanged: nothing decides on its own.** No Slint `init` callback may read the
image or apply a setting. The automatic keys stay click-only.

---

## 3. Editor fluidity (`EditorStore`, `StoreReader`, `DSSlice`)

The Swift rules (`CLAUDE.md`, *Editor fluidity*; `UI/EditorStore.swift`; `DesignSystem/DSSlice.swift`):

- The open photo's settings live in `EditorStore`, apart from the scene, so a notch re-runs only the
  views that read them.
- `EditorStore` publishes `settings`. It also publishes `Framing`, `CanvasSize`, `LoadProgress` and
  `Plotted` apart, because what changes on its own rhythm is published apart.
- `StoreReader` re-runs only its content when the store changes; the scene's body is not re-run.
- `DSSlice` wraps a block in an `Equatable` key. A block is re-evaluated only when its key changes,
  and the key must name everything the block draws and whatever picks its bindings' targets.
- One store write per notch.
- Judge on the 99th centile and the worst gap, on `make hand`.

**In Slint, most of this comes for free and is replaced by a different hazard.**

- Slint properties are fine-grained and lazily evaluated: a binding re-evaluates only when a property
  it read changes. That is what `DSSlice` hand-builds in SwiftUI, applied per property instead of per
  block.
  - `DSSlice` therefore has **no counterpart**. Its rule becomes: every block reads only the `Store`
    properties it draws, never a whole-settings struct.
- `StoreReader` becomes **a Slint `global Store`** holding one property per drawn field:
  - `exposure`, `contrast`, `levels-red-black`, … as `float`;
  - curves and histograms as `[float]` models or `image`.
  - Rust writes the global once per notch, so the "one store write per notch" rule becomes "one
    `set_*` per changed field per notch".
- **The hazard: a model or struct property.** Setting a Slint struct property or replacing a
  `ModelRc` invalidates every binding that reads any part of it.
  - A `PipelineSettings` struct exported whole into Slint would re-run every block on every notch,
    the exact failure `DSSlice` was written against.
  - Rule: **no `PipelineSettings`-sized struct property**. Scalars per field, and `VecModel::set_row_data`
    for one row of a list.
- Published apart: `Framing`, `CanvasSize`, `LoadProgress` and `Plotted` become separate globals
  (`Framing`, `Canvas`, `Progress`, `Plots`), so a progress tick touches no panel binding.
- **The canvas never travels through a property per notch.**
  - The PoC sets a new `Image` per render because the software renderer needs a CPU buffer.
  - In the app, oni-render writes into a texture that is imported once, and a notch calls
    `window.request_redraw()`.
  - Hold the imported `slint::Image` and re-import only when the size changes.
- **Rendering off the UI thread.**
  - Measured in the PoC: stages 4 to 7 on the CPU for a 1212 × 808 canvas take **110–215 ms**, and a
    synchronous render inside a drag callback freezes the knob for that time. Ten moves took 1.1–1.8 s
    with ten renders.
  - The Swift app encodes on the GPU and never waits on `commit()`.
  - The port must submit oni-render's work from a render thread, coalesce notches (latest value
    wins), and let the knob and readout update at display rate regardless (the `DSSlider` live/hand
    split, ported in the PoC's `track::Hand`).
- `DSGesture` (a hand on a control delays decode-costing work) becomes a Rust counter, incremented
  by each slider's press and decremented on release, read by the decode scheduler.

---

## 4. Inventory

Columns: **Swift**, the source; **Slint**, the construct it maps to; **Risk**, what makes it hard.
Risk tags:
- `[draw]`: custom drawing (Path, image raster or wgpu).
- `[shader]`: needs a GPU pass.
- `[a11y]`: accessibility must be declared by hand.
- `[menu]`: native menus through muda.
- `[text]`: text rendering.
- `[input]`: an input Slint does not deliver.
- `[perf]`: a measured cost to keep.
- `[native]`: an AppKit service with no Slint equivalent.

### 4.1 Windows and scenes

| Swift | What it is | Slint | Risk |
|---|---|---|---|
| `WindowGroup("Open Negative Initiative")` | Editor window: unified toolbar, hidden title, minimum 1100 × 680 | `MainWindow inherits Window` with `min-width: 1100px; min-height: 680px`, title bar transparent and full-size content via winit macOS attributes, the toolbar drawn in-window | Done (`oni-app/src/chrome.rs`): the backend's attributes hook chromes the main window only (transparent, full-size content, hidden title); the traffic lights are centred in the bar and their room read from `NSWindow.standardWindowButton` frames (78 pt), laid again on every resize, none in full screen. The bar's empty room drags the window (`drag_window`) and a second press within the double-click interval does `AppleActionOnDoubleClick`. Windows keeps its native bar. Window tabbing needs checking. |
| Console window (`ConsoleView`) | The diagnostics log, 620 × 420 | Second `Window` with the ds `Scroller` of `Text` rows, `font-family: mono` | Low. |
| Design System playground (`DSPlayground`) | Every token and component, in a window built only with `ONI_DEV` | A `Window` behind a Cargo feature `dev` | Low; its catalogue checks become Rust tests. |
| `Splash` | A new picture per version, held `DS.Motion.splashMinimum` while the gallery loads | Overlay `Rectangle` + `Image` in `MainWindow`, removed in the same turn as the windows show | Low. |
| `WrappedView` ("My Year on Film") | Year summary in Enriqueta 56/44 | Overlay component | `[text]` Large Enriqueta needs checking against Core Text metrics. |
| Modes: editor, gallery, export, lab (Scanner face), roll (Prepare the Roll) | Top-level state of the scene | One `in property <Mode> mode` (Slint `enum`) switching content with `if` | `if` destroys and rebuilds a subtree: keep the gallery alive with `visible: false` instead, so its scroll and layer cache survive (the Swift app keeps it mounted). |

### 4.2 The editor's frame

| Swift | What it is | Slint | Risk |
|---|---|---|---|
| Library column (`FolderNavigator`, `InfoSidebar`, `SidebarLayout`) | Left column: the tab bar, the navigator's header (parent and favourites on one mini plate, then the pill), then beside a photo or on the projects one row of `SidebarKeys` on a mini plate followed by the pill; folders, rolls, projects, then photos as names, two thumbnails a line, or one | `VerticalLayout` with a resizable width (`DSColumnResizer`), `KeyPlate { mini }` of `GlyphKey`s | See 4.6 for the tree. |
| Canvas card (`EditorScene+Card`, `NegativeView`) | The photo on the canvas colour, radius `card` 12, padded, with overlays; flush with the columns and the status bar, inset `cardInset` on four sides on export only | `Card` (darker while cropping) holding the canvas `Image` and overlay components: the read-only badge, the decode progress with no stand-in, the "could not be decoded" line, the column keys (`columnKey`), and a hidden column's edge reveal (`floating(_:)`, `FloatingColumn`) | `[draw]` The rounded clip is ignored by the software renderer (fine on GPU). The squircle (`.continuous`) is not drawn by Slint (section 5). |
| Settings panel (`EditorScene+Panel`, `DSPanel`) | Right column, tabs Roll/Crop/Color/Details, pager between panes | `VerticalLayout` with `DSTabBar` and a ds `Scroller` per pane (`Pane`), the pager's slide animated by `x` | `DSPanel` keeps each pane's scroll offset and measures the outline (`DSPanelSkeleton`); a `Scroller`'s `content-y` must be saved per pane in Rust. |
| Status bar (`DSStatusBar`) | One bar under the viewer: busy state or a line of facts, the seal right after the name, the medal picker at the end, in the gallery and beside a photo | `CardStatus`, a `HorizontalLayout` with the progress rail, `Space.s` around its row | Low. |
| Preset deck (`PresetCarousel`) | Hidden below the bottom edge, rising as the pointer nears it | `ui/presets.slint` `PresetDeck`: the deck's own `TouchArea`, only its strip showing at rest, raises it on `has-hover`; each card reports its hover to Rust, which renders the preview | Done. The first move into a window only enters it, so a check moves the pointer in before aiming at the strip. |
| Column resizer (`DSColumnResizer`) | Drag handle between panes | `TouchArea` with `mouse-cursor: col-resize` | Low. |
| Help layer (`dsHelpLayer`, `DSHelpKey` preference) | A block's note drawn on a layer above the pane, anchored to its "?" | One `PopupWindow`-free overlay at `MainWindow` level, positioned from the "?" key's `absolute-position` | `[draw]` Slint has no preference keys: each "?" reports its absolute position and text to a `global Help` on hover. |

### 4.3 The toolbar (`AppBar`, `EditorScene+Bar`)

Three zones, one `ToolbarItem` per zone, each `.glassless()`. Every gesture there is one a menu already
reaches.

| Zone | Keys | Slint | Risk |
|---|---|---|---|
| Leading (`.navigation`) | EDITOR / GALLERY word key (⌘G), holding the width of the wider face | `Keycap` component with `faces` measured: width = max of both labels' `Text.preferred-width` | `[text]` |
| Principal, editor | Plate: framing (⌘K), compare; contact sheet; plate: undo (⌘Z), redo (⌘⇧Z); plate: copy (⌘⇧C), paste, PASTE TO ROLL; PREPARE ROLL; de-dust; plate: RESET SCAN, trash | `KeycapPlate` = `HorizontalLayout` in a recessed `Rectangle`; icon keys from the Trinil SVGs | `[draw]` Keycap chrome (section 5). A hidden copy of the editor keys holds the zone's width so keys move inside it: Slint's layouts re-measure, so this becomes a fixed `min-width` computed from the editing set. |
| Principal, roll mode | One word key per `RollStep`, the current one inverted, a lamp on done steps | `for step in steps: Keycap { tint, ink, lamp }` | `[draw]` `DSLamp` glow. |
| Trailing (`.primaryAction`) | FEEDBACKS; the export/import/update key (EXPORT, EXPORT n PHOTOS, IMPORT, UPDATE, busy titles ADDING / EXPORTING / UPDATING) | `Keycap` with label from Rust | Low. |
| Help tooltips (`help:`) | System tooltips on every key | Slint has no tooltip element: a hover-delayed overlay `Text` | `[native]` The system tooltip delay (~1 s) and look must be imitated. |

### 4.4 The panel: panes, blocks, controls

Blocks are `DSSection`: a groove separator, a title row with an optional "?" and a reset, and then the
controls. Every slider is `DSSlider` (4.4.1). The labels below are exact strings; they must not change
(the website checks them against the Swift source).

| Pane (`PanelTab`) | Block (`DSSection`) | Controls (Swift file) | Slint | Risk |
|---|---|---|---|---|
| Roll (`SettingsPane`) | Film Type | Mode picker | `DSPicker` | `[menu]` |
| | Source Encoding | "Read as" picker | `DSPicker` | `[menu]` |
| | Measured Roll | `DSWarning` bands, PREPARE THE ROLL…, JOIN / ROLL OF on one filling plate, roll previews (`RollPreviews`) | `WarningBand`, `KeyPlate { fills }`, `Image` row | Previews render through oni-render. |
| | Source Balance / Balance Strength | Strength slider | `DSSlider` | |
| | Density Ceiling | Ceiling slider | `DSSlider` | |
| | Per-Channel Levels | `LevelsView`: histogram, five handles, RGB/R/G/B tabs | `DSTabBar` + histogram `Image` + `DSHandle`s | `[draw]` The histogram is a `Canvas` path in Swift: render it as an RGBA buffer in Rust or as a Slint `Path` from `commands` built in Rust. Path rebuilds cost on the software renderer only. |
| | Dye Separation | Dyes picker, Dye separation slider | `DSPicker`, `DSSlider` | |
| | Flat-Field (`FlatFieldControls`) | Switch then its word at the left, gear at the far right; Mask label above its picker; USE THIS PHOTO AS FLAT-FIELD; the gear reveals Amount, Temperature, Tint, then LOAD / RENAME / FOLDER on one filling plate | `ToggleRow`, `GlyphKey`, `Field` over `LinesPicker`, sliders, `KeyPlate { fills }` | |
| Crop (`FramePane`, `GeometryPanel`) | Crop / Aspect Ratio | COPY, PASTE hugging and PASTE TO ROLL filling one plate; ratio chips (Free, 3:2, …), then the Vertical switch before its word | `KeyPlate { fills }`, `DSTabBar`, `ToggleRow` | |
| | Straighten | Angle slider | `DSSlider` (bipolar) | |
| | Rotate & Mirror | The rotate key, then both mirrors on one plate, the second glyph turned 90°; none ever lit | `IconButton`, `KeyPlate` with `turned` | |
| | Measuring Window | One filling plate: reset glyph, MEASURING WINDOW / APPLY WINDOW (confirm green, the section's note as its tooltip), copy, paste; block grid (`MeasureGridOverlay`) | `KeyPlate { fills }`, overlay `Path` grid | `[draw]` |
| Color (`ColorPane`) | Lighting (`PrintPanel`) | Exposure, Contrast, Shadows, Highlights, Black point, White point; the RGB curve plot | `DSSlider` ×6; plot as `Image` | `[draw]` The `Canvas` plot. |
| | Color | Temperature, Tint (`TemperatureTintPanel`); gains (`GainPanel`: Cyan–Red, Magenta–Green, Yellow–Blue); Color density | Bipolar `DSSlider` with hue tracks (gradient fills) | `[draw]` Track gradients between a hue and its complement: `@linear-gradient`. |
| | Curves (`CurveEditorView`) | Spline over histogram, RGB/R/G/B/luma tabs; drag empty space adds, double-tap removes | Plot `Image` + `TouchArea` + `DSHandle` discs | `[draw]` `[input]` Spline drawn as a `Path` from Rust-built commands. The double-tap must be detected on press (see 4.10). |
| | Effects | Contrast, CHROME, VIGNETTE (`VignettePanel`) | `DSSlider`s, word keys | |
| | Paper Correction | AUTO PAPER CORRECTION on a filling plate, profile chooser | `KeyPlate { fills }`, `DSPicker` | `[native]` ICC proofing via ColorSync stays on the Rust side (lcms2 or ColorSync through objc2). |
| | Saturation Curve / Spectrogram Finishing (`GradePane`, `SaturationCurveView`, `SpectrogramPanel`, `SpectrogramBands`) | Equaliser over the output histogram; colour wheel with one point per band; band tabs with Hue, Saturation, Luminance sliders; Tab key cycles bands | Plot `Image`, wheel as `Path`s + `@conic-gradient`, `DSSlider`s | `[draw]` `[a11y]` Wheel points need focus (`FocusScope`) since "focus is the point's active state". |
| Details (`DetailsPane`) | Chroma Denoise (`ChromaPanel`) | Radius, Amount | Twin `DSSlider`s | |
| | Sharpening (`SharpenPanel`) | Amount, Radius, Halo limit | `DSSlider`s | |
| | Texture | Texture slider | `DSSlider` | |
| De-dust (`DedustPane`, `CorrectionsPanel`) | Dust & Scratches | Brush Size (] and [), corrections list with "Remove this correction" | `DSSlider`, `ListView` rows | Brush cursor on the canvas: a custom circle drawn over the canvas, the system cursor hidden (`mouse-cursor: none`). |
| Scanner face (`EditorScene+Lab`, `UI/Lab`) | Machine | `LabKeyboard` keypad groups, `LabScreen` plot band, `ScannerKey` | `Keycap` grid, plot `Image` | `[draw]` |

Header of a block: the title in `DS.Typo.section` (Inter 13), a "?" (`DSHelp`), and a reset that reads
the latest reset at click time (a title row skipped by its slice keeps an older closure). In Slint the
reset is a callback that reads Rust state when called, so that trap disappears.

#### 4.4.1 `DSSlider`, the panel's one slider (ported in the PoC)

| Behaviour (Swift) | Slint/Rust | Verified in PoC |
|---|---|---|
| The readout row: label in `DS.Typo.label`, value in `DS.Typo.value` (Inter 11, tabular) | `Readout` component | Label yes; tabular figures no (no `tnum`). |
| The whole row grabs; the track is inset 12 pt each end (`handle-thickness / 2`) | A `TouchArea` over the full row; travel = `track.width` | Yes. |
| Relative drag: `start + translation / span`, never a jump to the pointer | Rust `track::asked` | Yes (unit and headless). |
| Option slows the drag to 0.2, read live during the drag | `event.modifiers.alt` read on every `pointer-event(move)` | Yes: a quarter-span drag with Option moves 0.4 EV instead of 2. |
| A bent track (`centre`) maps piecewise | `track::position`/`value` | Unit test with twin. |
| Live value drawn at once; handed to the binding at most 30 times a second; release hands the last live value | `track::Hand` + a single-shot `slint::Timer` for the trailing hand-off | Readout follows every move and release commits (headless); the 30 Hz rate is unit-tested only. |
| Double-click resets to base on the second *press*, and the rest of that press drags nothing; AppKit's `clickCount` keeps counting, so a third quick press resets again | Press timing measured in Rust (`DOUBLE_CLICK` 500 ms), not `double-clicked` | Yes, with the twin (600 ms apart does not reset) and the third press. |
| A drag starts from `fraction`, the shown value's position, never from where a spring has the knob | `State::fraction()` | Yes: a press with no travel during a spring lets it land (the old wiring left the knob at 0.625 for 0.5). |
| A value arriving from elsewhere springs the knob with `settle` (0.38 s, ζ 0.72); never during a drag | Analytic `motion::Spring` sampled by a 16 ms `Timer` | Yes: the knob has not jumped right after the reset and lands at rest. |
| Scroll: along the track only (`abs(along) > abs(across)`, else the panel scrolls); wheel line 10 pt, trackpad 0.25, idle 0.16 s; writes the binding on every event | `scroll-event` → Rust, which answers `accept` or `reject` | Yes: 40 pt of glide moves a quarter of it; a vertical or mostly vertical scroll moves nothing and renders nothing (twin). Wheel/trackpad cannot be told apart in Slint (`[input]`), so only trackpad travel is wired. |
| Hover or grab grows the knob ×1.07 on the `rise` spring | `animate` on a `grow` property scaling `width`/`height` (the software renderer draws no `scale` transform), `ease-out-back` 150 ms | Wired, not judged: the curve is an approximation of the spring. |
| Haptic tick per step (`DSHaptics`) | `NSHapticFeedbackManager` via objc2 on macOS; nothing on Windows | Not ported. |
| Accessibility: the value is the binding's (`format(value)`), not the readout's live one | `accessible-role: slider`, label, `value-text`, increment/decrement | Increment tested (a hundredth of the range). |

### 4.5 The canvas and its overlays

| Swift | What it is | Slint | Risk |
|---|---|---|---|
| `MetalImageView` | `MTKView`, `.bgra8Unorm`, the `CAMetalLayer` tagged with the screen's colour space (P3 fallback), drawn without CPU readback | oni-render writes an `Rgba8Unorm` texture; imported once with `Image::try_from(wgpu::Texture)` on femtovg-wgpu | `[shader]` `[native]` **Colour management is the biggest gap**: neither wgpu nor Slint exposes the `CAMetalLayer.colorspace`. The canvas is proofed into the export space in Swift; the port must either set the layer's colour space through objc2 on the surface winit creates (unsupported territory), or encode for the display profile in the last shader. |
| `NegativeView` | The canvas, the measuring window, framing state, preview disc | Component stacking the canvas `Image` and overlays | |
| `CropOverlay` | Crop frame and measuring window in image fractions, handles, the straighten grid | `Path` lines + `Rectangle` handles, positions from Rust | `[draw]` Rotated crop frame: `Path` with Rust-computed corners, since the software renderer cannot rotate. |
| `MeasureGridOverlay` | Block grid over the measuring window | `Path` | `[draw]` |
| `Loupe` | The Detail stages at the file's own scale | A second imported texture | `[shader]` |
| `CanvasNavigation`, `Viewport` | A wheel zooms; a trackpad pans unless ⌘; pinch zooms; scale relative to full resolution | `scroll-event` for wheel/trackpad; pinch needs winit's `PinchGesture` | `[input]` Slint delivers no pinch and no scroll phase to a `TouchArea`. Route winit's `WindowEvent::PinchGesture` and `MouseScrollDelta::{LineDelta, PixelDelta}` to Rust through `slint::winit_030::WinitWindowAccessor::on_winit_window_event`. |
| `DSBadge` | Pins and floating labels over the photo | `Rectangle` + `Text` | |
| Compare (before/after) | Split or toggle | Two images, a clip `Rectangle` | |

### 4.6 Gallery, tree and library

| Swift | What it is | Slint | Risk |
|---|---|---|---|
| `GalleryView`, `GalleryEngine`, `GalleryLayout`, `TileLayers`, `LayerHostView` | A grid of thousands of tiles drawn as `CALayer`s under one scroll view, since one view per cell re-checks every cell (~5 ms each at twelve columns) | Rust lays and scrolls the document (`Library.scroll`, `content-height`) and hands Slint only the visible tiles; a ds `ScrollKnob` floats over the edge, its drag sent through `Library.scrolled` as the wheel's | `[perf]` Measure landing cost against Swift's count of realised cells; if it fails, draw the grid as one texture per visible band. |
| `RollHeaderContent`, `PinnedGalleryHeader`, `RenameField` | A band's heading over its tiles or held at the grid's top; its double click types over it | ds `TileHeading` and `PinnedHeading` with a `renaming` field (`HeadingField`, a `FocusScope` taking Escape over a ds `TextField`); Rust resolves the click on the grid's one pointer and keeps one field at a time across relayouts, the headings' model updated row by row | |
| `GalleryEngine.land`, `dropDestination` | The grid lands on the open photo each time it mounts; files dropped on the window in gallery mode | `MainWindow.grid-shown` and its `changed` handler calling `Library.gallery-shown`; winit's `DroppedFile` gathered in `main.rs` and handed whole to `Ctl::dropped` | Nothing marks the window as a drop target while files hover. |
| `RollPreviews`, `RollReferenceSurface` | The roll's frames, one decode at a time per kind, superseded requests never start | Image models fed by a Rust decode queue | `[perf]` |
| `FlatFieldBlock` in `RollPanel` | The Flat-Field step: switch and gear, Mask picker, USE THIS FRAME AS FLAT-FIELD (SETTING… while it decodes, dark until a frame is picked), the gear's Amount, Temperature, Tint, then LOAD… (LOADING…) | `prepare.slint` `FlatFieldBlock`: `Toggle`, `IconButton`, `LinesPicker`, `Button`s, `RollSlider`s; `Preparing.take-flat` and `load-flat` | |
| `FolderNavigator`, `TreeEngine`, `TreeLayers`, `TreeRows`, `TreeRowChrome` | The folder tree, rows as layers; chevrons drawn, tabular figures in captions; a click in the chevron's zone folds, elsewhere it rings or opens, with ⌘ or ⇧ it picks; a right click opens the menu on the pick or the row; a double click types over a folder's or project's name; the tree reveals the photo it follows, centred | Rust-laid rows in the ds `Scroller`, a `TouchArea` under them for a click on no row; the Projects tree (`project_tree.slint`) laid the same way, its photos names or thumbnails by each mode's `sidebarLayout`, a `TouchArea` under its rows for `clickEmptyProjects`, a row's press moved past 4 points carrying the pick (`ListRow.dragged`, `released`) onto a ds `DropRing` or before a `DropLine`, a ds `Badge` naming the rows under the pointer; on both trees a ds `PinnedTreeRow` over the `Scroller` holds the open parent row, Rust resolving it on each scroll (`tree-scrolled`, `project-scrolled`); `ListRow.fold-zone` splits the chevron's click (`folded`) from the row's, `ListRow.command` and `.shift` carry the press's modifiers to `clicked`; both trees share the column's `ContextMenu`; the centre key on both tabs | `[perf]` `[text]` tabular figures. |
| `InfoSidebar` | Roll, Film Stock(s) (each count row led by its stock's icon), Camera Body, Shot Date (cleared by a reset glyph key), Description (a fixed-height multi-line box), Project | `DSSection`s with `DSTextField`, `TextArea`, `StockIcon` and pickers, in a ds `Scroller` as Swift's `ScrollView` | |
| `ListPicking` | Click, ⌘-click, ⇧-click selection | Rust, from `pointer-event.modifiers` | |
| `DSMedalPicker`, `DSFoil` | Bronze/silver/gold medals as foil rasters lit per pixel | `Image` from the same Rust raster (the Swift code is a CPU raster already) | `[draw]` Port `DSFoil.render` as is. |
| No drag-and-drop reorganisation (product rule) | | Nothing | Keep it absent. |

### 4.7 Export (`ExportSurface`, `ExportSpaceField`)

| Swift | Slint | Risk |
|---|---|---|
| Export is a mode of the card: one row per photo, fields left, preview right; the count is in the label in all four places | Mode branch in `MainWindow`, rows in the ds `Scroller` (`ExportColumn`) | |
| Space field: four matrix spaces or an installed ICC profile; JPEG dims spaces needing 16 bits | `DSPicker` | `[menu]` `[native]` Enumerating installed ICC profiles: ColorSync on macOS, `WcsEnumColorProfiles` on Windows. |
| `NSSavePanel` / `NSOpenPanel` | `rfd::AsyncFileDialog` | `[native]` rfd uses the native panels on both systems. |

### 4.8 Sheets, dialogs, popovers

| Swift | Slint | Risk |
|---|---|---|
| `DSDialog` (a question as a sheet; `option` choices are neutral and take no Return) | `Sheet`: an in-window modal overlay hung from the window's top (`SheetStyle.margin`), no scrim drawn, a full-size `TouchArea` blocking the content below; leading `section` title (`question`), message, keys right-aligned in one `KeyPlate`; destructive keys red and deaf to Return; the quit alert's Cancel the default on the right | `[native]` Not a real sheet: no slide from the title bar. Imitate with the `reveal` spring. |
| `ReportSheet`, `WelcomeSheet` (with `TutorialVideo`), `NeutralDecodeNotice`, `RootsCatchUp`, `PresetSave`, `PresetEditor`, `StockPickerSheet`, `PaperProfilesSheet` | The same modal overlay, one component each, each at Swift's width with a leading `title`; a card never passes the window less two margins and scrolls inside; the welcome bleeds its 3:2 photo edge to edge under `SplashCaption` at 0.8 | `TutorialVideo` plays a looping movie (`AVPlayerLooper`): Slint has no video element. Decode frames in Rust (or show a still) `[native]`. |
| `dsPopover` (`RootPicker`, `FolderNavigator` ancestors, `WorkingSpaceChip`) | `PopupWindow` positioned under its anchor | `PopupWindow` closes on outside click by default; the Swift popover is resized at once (no easing), which `PopupWindow` matches. |
| `DSPicker` dropping the **system menu** with the chosen item over the field | muda `ContextMenu::show_context_menu_for_nsview` at the field's position, or Slint `ContextMenuArea` opened programmatically | `[menu]` Positioning the chosen item over the field is NSMenu's `popUp(positioning:at:in:)`; muda's context menu does not position on an item. Needs objc2 on macOS. |
| `DSNews`, `WhatsNew` | Release notes with "new" pills cleared on hover | Overlay component | |

### 4.9 Menus and shortcuts (`EditorCommands`)

All in Slint's `MenuBar` (native via muda). The shortcuts are exact. Several titles are computed
(`menus.modeTitle`, `exportTitle`, `undoTitle`, …), so the table names them by role; items without a
shortcut (Read the Folders Again, Show the Selected Photo, Compare, Scanner Emulator, Prepare the
Roll…, Save as Preset…, …) are not listed.

| Shortcut | Item |
|---|---|
| ⌘O | Open Folder |
| ⌘G | Gallery / Editor |
| ⌘E | Export |
| ⌘R | Recheck Originals |
| ⌘⌫ | Move to ONI Trash |
| ⇧⌘⌫ | Restore |
| ⌘− / ⌘+ | Zoom Out / Zoom In |
| ⌥⌘[ / ⌥⌘] | Hide Library / Hide Panel |
| ⌘K | Crop and Straighten |
| ] / [ (bare) | Brush size |
| ⇧⌘M | Mirror |
| ⇧⌘Y | Screen gamut |
| 0, 1, 2, 3 (bare) | Medal: None, Bronze, Silver, Gold |
| ⌘L | Cycle filter |
| ⇧⌘G | New Roll |
| ⌥⇧⌘G | Ungroup |
| ⇧⌘C / ⇧⌘V | Copy Settings / Paste Settings |
| ⌘Z / ⇧⌘Z | Undo / Redo |
| ⇧⌘R | Reset to Imported State |

The other menus are Help (Show Console, What's New in ONI 1.2, its release read from `help::EDITION`, Show What's New Again, Report a Bug /
Give Feedback…, Show My Year on Film), View, Labels, Rolls, and Settings (the preset list, with a
Delete a Preset submenu); Check for Updates… is an item after the application menu's About.

Risks:
- `[menu]` **Bare-key shortcuts** (`]`, `[`, `0`–`3`) in the menu bar must not fire while a text field
  is being edited. NSMenu sends key equivalents before the field sees them. The Swift app guards this,
  and muda gives no per-item validation hook: keep bare keys out of `shortcut:` and route them through
  the key monitor (4.10), showing them in the title only.
- `[menu]` A **dynamic menu** (the preset list, Labels) is rebuilt as a Slint `for` over a model;
  muda rebuilds the NSMenu on change. The Swift app publishes menus "as last published" to avoid
  rebuilding them per notch, so keep the menu model off the per-notch path.
- macOS application-menu items (About, Hide, Quit, Services) and the `QuitGuard` decision come from
  muda's `PredefinedMenuItem` plus winit's close request. `applicationShouldTerminate` has no winit
  equivalent for ⌘Q, so `[native]` objc2 is needed to keep the quit-time check.

**Context menus** (`ContextMenuArea`, native via muda; no shortcuts allowed there, as in Swift):

| Menu | Items |
|---|---|
| Cell (`CellMenu`) | Paste Settings, Reset Settings, Reset to Default…, medals, New Roll from, Ungroup, New Project from, Add to / Remove from Project, Export…, Show in Finder, Move to ONI Trash |
| Folder | Rename…, Make a Roll / Back to a Folder, New Folder Inside, Show in Finder |
| `PresetCarousel` | Preset actions |
| Project | Delete Project |

"Show in Finder" needs `[native]` `NSWorkspace.activateFileViewerSelecting` (objc2) or `explorer /select,`
on Windows.

### 4.10 Keyboard and gestures

`DSKeyboard` / `DSKeyMonitor` is an `NSEvent` local monitor. It acts in the key window only, not while
a text field is being edited, and gives exclusive claimants first refusal.

| Key | Action |
|---|---|
| Arrows | Walk frames; gallery navigation; roll frames |
| Space | Toggle mode (Pro's editor outside de-dust, and the Lab); open in the gallery; the hand tool while held in de-dust |
| Return | Apply the armed measuring window, then leave the crop for the last panel; open in the gallery |
| Esc | Leave crop, popover, Wrapped, lab |
| ⌘A | Select a folder's photos: the gallery's groups, or beside a photo the folders the selection touches |
| Tab | Spectrogram bands |

**Slint mapping**: one `FocusScope` at the root of `MainWindow`, holding focus whenever no text field
does.
- `key-pressed` returns `reject` for unclaimed keys, so the menu bar still sees them.
- A Rust-side claimant stack replaces the exclusive claimants.
- `[input]` A Slint `FocusScope` loses keys whenever focus moves to a focusable child, such as a
  slider or a wheel point. Each focusable component must forward unhandled keys, or the root has to
  re-take focus after each click.
- `[input]` Space held as the hand tool needs `key-released` and repeat filtering (`event.repeat`).

Other gestures:
- **Pinch**: winit `PinchGesture`, routed as in 4.5.
- **Scroll phase**: `MouseScrollDelta` kind from winit.
- **Hover cursors**: `mouse-cursor`.
- **Haptics**: objc2 (`DSHaptics`).
- **Keycap sound** (`KeycapSound`, `AVAudioEngine` with varispeed): `cpal` or `rodio` with a pitch
  shift `[native]`.

---

## 5. Design system

### 5.1 Tokens (`DSTokens.swift`, 296 `static let`/`var`)

Generate one `export global DS` in `ui/ds/tokens.slint` from `DSTokens.swift` with a script, never by
hand. The website does the same with `sync-app-tokens.mjs`, which refuses name collisions:
`Control.knobLine` and `Palette.knobLine` collide.

| Swift enum | Slint | Notes |
|---|---|---|
| `DS.Space` (xxs 2, xs 4, sm 8, md 16, lg 24) | `length` properties | 1 pt = 1 logical px. |
| `DS.Radius` (badge 8, control 16, track 6, card 12) | `length` | Every rounded corner is a squircle in Swift (`.continuous`); Slint draws circular arcs. **Visible at 16 and 12 pt** `[draw]`. Exact copy: draw squircle shapes as `Path`s (superellipse commands built in Rust) for cards and keys; keep circular arcs only where the radius is ≤ 6. |
| `DS.Motion` (springs: grab .22/.62, settle .38/.72, lift .16/.55, rise .11/.62, travel .52/1, reveal .2/1; curtain 0.14 s; recomputedInterval 1/30 s) | Analytic `Spring` in Rust driving properties through a 16 ms `Timer` (PoC `motion.rs`), or Slint `animate` with a `cubic-bezier` approximation where the overshoot does not matter | Slint's `animate` has no spring easing. `ease-out-back` approximates a small overshoot, never the same curve. |
| `DS.Stroke`, `DS.State` (opacities for disabled, pressed, hover) | `length`, `float` | |
| `DS.Typo` (Inter 13 label/section, Inter 11 value tabular, Inter 17 title, 24 heading, 22 mark, 11 chip, Enriqueta 20/26/56/44, mono 11, seal 9.5) | `font-family`/`font-size` pairs; Inter and Enriqueta imported | `[text]` Tabular digits (section 1). |
| `DS.Tint`, `DS.Palette` (panel #282828, canvas #1b1b1b, text α .8471 / .549 / .2471 on white, keycap, separator, accent …) | `color` / `brush` | `Color.primary` resolves per appearance; the app is dark-only, so constants. |
| `DS.Control` (track height 14, handle 24 × 34, panel width, paddings, knob line…) | `length` | |
| `DS.Scanner` | `length`, `color` | |
| `DSShape.rounded` / `round(layer)` | `Squircle` component (Path) | See Radius. |

### 5.2 Components

| Swift | What it is | Slint | Risk |
|---|---|---|---|
| `Keycap`, `KeycapStyle`, `KeycapSurface`, `KeycapButtonStyle`, `KeycapTokens`, `Frustum` | The key: a frustum lit by one light (azimuth 100°, elevation 33°, ambient .25, normaliser .7946), four walls, a dished face, ornaments, press depth, socket shadow, hover lift | **A Rust raster per (size, albedo, style)** as `Image` (PoC `keycap.rs`), cached; depth changes only translate/scale and the socket shadow | `[draw]` Slint cannot shade per pixel; the raster is the only way to keep the Frustum maths exact. Press depth uses `scale(0.95)`/`translate`: drawn by GPU renderers, not the software one. A GPU-rendered variant (a wgpu pass via the rendering notifier) is possible but needs femtovg-wgpu. |
| `KeycapPlate` | The recessed plate grouping keys | `Rectangle` with the inner shadow drawn as gradient strips | `[draw]` Inset shadows do not exist in Slint: build them from gradients (as the PoC's rail does). |
| `KeycapIcon`, `DSIcon`, `Glyph+Trinil` | Trinil SVG icons | `Image { source: @image-url("icons/x.svg"); colorize: … }` | Slint renders SVG through resvg: the strokes should match; verify at 1x. |
| `KeycapSound` | Key down/up sound with varispeed | `rodio`/`cpal` | `[native]` |
| `DSSlider`, `DSHandle` (bar handle) | 4.4.1 | PoC | |
| `DSHandle` (curve point disc) | Disc without hit testing; the container grabs wider | `Rectangle` circle + parent `TouchArea` | |
| `DSPanel` (`DSSection`, pager, `DSSeparator`, `DSColumnResizer`) | Block frame, pane pager | Components (PoC has `Section`, `Separator`) | |
| `DSPanelButton` | The big button of a block | Keycap raster | |
| `DSPanelSkeleton` | Each control's outline remembered per build | Rust geometry cache | |
| `DSTabBar` | Condensed tabs, marks for settings in hidden tabs, hover shows the label | `HorizontalLayout` of tabs, `accessible-role: tab` | `[a11y]` |
| `DSToggle` | A switch: cross and check in a hollow, the held state lit | Component, `accessible-role: checkbox` | `[a11y]` |
| `DSPicker` | Value in a hollow, chevron key, drops the system menu | 4.8 | `[menu]` |
| `DSTextField`, `DSField` | Field in a hollow, lit edge while focused; multiline grows | `TextInput` / `TextEdit` in a recessed `Rectangle` | Text editing, IME and undo come from Slint, which differ from NSTextField's. |
| `DSHelp` | "?" beside a heading; the note drawn by the help layer | 4.2 | |
| `DSHover` | Fill for area elements; state for self-drawn ones | `TouchArea.has-hover` | |
| `DSGesture` | Count of hands on controls | Rust counter (section 3) | |
| `DSHaptics` | One tick per step at constant felt cadence | objc2 on macOS | `[native]` |
| `DSScrollAdjust` | Wheel and trackpad travel on a control | `scroll-event` | `[input]` no wheel/trackpad distinction. |
| `ScrollView`, `List` (default `.automatic` indicators; the preset carousel `.never`) | AppKit's overlay scroller: a knob over the edge only while the content overflows, shown while scrolling, a moment after and on edge hover, no gutter; on the edge it widens over its track, drags, pages a press on the track toward the pointer (repeating while held) and with Option jumps to the spot | ds `Scroller` / `ListView` with `ScrollKnob` (`ScrollStyle`): a `TouchArea` on the `reach` strip emits `dragged(offset)`, which a `Scroller` writes to its `Flickable` and the gallery sends to `Library.scrolled`; the content never drags, a vertical surface never slides sideways; `knob: false` for the deck | Done. The system's default "Jump to the next page" is followed; the legacy style ("Always show scroll bars") and the "Jump to the spot" setting are not read. |
| `DSKeyboard` | Key monitor | 4.10 | |
| `DSHistogram` | Scaled histogram plot (compression exponent shared with the bench) | Rust raster → `Image`, or `Path` | `[draw]` |
| `DSPlotScreen` | A plot behind glass: canvas floor, engraved quarter lines | `Rectangle` + `Path` | |
| `DSProgress` | Progress as the slider's rail, lit along done; sweeping when uncounted | Rail component + animated fill | |
| `DSStatusBar`, `DSBadge`, `DSWarning`, `DSEmptyState`, `DSLCD`, `DSLamp` | Bars, pins, warning band, empty state, LCD text, panel lamp | Plain components | `DSLamp`'s glow: a `drop-shadow-blur` (GPU renderers only). |
| `DSFoil`, `DSMedalPicker` | CPU-rastered foil | Port the raster | |
| `DSReorder` | Chips reordered across fixed slots, written on release | `TouchArea` drag + animated `x` | |
| `DSNews` | Notes and their "new" pills | Component | |
| `DSDialog`, `DSPopover` | 4.8 | | |
| `DSText` | Shared text styles | Components `Label`, `Value`, `Section` | |
| `DSCompat` | macOS 13 fallbacks | Nothing | |
| `DSPlayground` | Live reference page | Dev window | |

---

## 6. Risk register

Ordered by cost to an identical copy.

1. **Display colour management.** Swift tags the `CAMetalLayer` with the screen's colour space and
   proofs into the export space. Slint and wgpu expose neither the layer's colour space nor an
   extended-range surface.
   - Without objc2 access to the winit-created layer, the canvas is shown as sRGB-encoded bytes,
     which a P3 display reinterprets.
   - The PoC encodes to sRGB and could not check this on a real screen.
2. **Squircles.** Every corner in the app is continuous; Slint draws circular arcs. The fix is a
   `Path`-based squircle component, which costs path rasterisation per frame on the software renderer
   and is cheap on the GPU ones.
3. **Keycap rendering.** It must stay a Rust raster or a wgpu pass. The PoC raster matches the Swift
   light at four points (walls .9729, .7530, .4629, face 1 at mid-dish). Two parts are assumptions,
   not measurements:
   - the wall seam blur (SwiftUI's blur radius taken as a Gaussian sigma);
   - the circular corners.
4. **Rendering off the UI thread.** Measured: 110–215 ms per CPU render of 1212 × 808 freezes the slider.
   The GPU path with an imported texture is mandatory, and it is untested here (no display, and
   femtovg-wgpu does not run under the software renderer).
5. **Input gaps.**
   - No pinch, no scroll phase, no precise-delta flag: winit events must be tapped via
     `unstable-winit-030`.
   - Double click on press: measured in Rust (done in the PoC).
6. **Tabular figures.** There is no OpenType feature API. A frozen-`tnum` Inter build is needed.
7. **Sheets and system pop-up menus.** They become in-window overlays, and `DSPicker`'s on-item
   positioning needs objc2.
8. **Performance of the gallery and tree.** Swift draws them with `CALayer`s for measured reasons;
   Slint's `ListView` virtualisation must be measured on a real library before the design is fixed.
9. **Licence.** Slint is GPL-3.0-only under its open-source option (section 1).
10. **Accessibility** has to be declared on every custom component.
11. **Text rasterisation** differs from Core Text (spacing, hinting); judge it side by side at 1x and 2x.
12. **The canvas inherits the decoder** (not a UI risk, but what the eye will judge first): LibRaw's
    pinned decode is 1.28 to 1.57 EV under `CIRAWFilter`'s on roll #8's DSC_6440 (section 7), so every
    sidecar written by the Swift app renders far brighter in the port until `oni-decode` closes it.

---

## 7. Proof of concept

A workspace crate that depended on `oni-render` and `oni-decode` only.

**What it does.**
- Opens roll #8's `DSC_6440.NEF` (the reference roll, well exposed and well scanned):
  - decoded by `oni_decode::decode` (LibRaw, pinned);
  - reduced in `f32` by a box average to 1212 × 808;
  - `captureStops` undone first, as Swift's `CIExposureAdjust` does;
  - converted by `oni_render::cpu::stage47` with **every argument `Pipeline.apply` passes on the Pro
    face for a colour negative**, read from the frame's own sidecar: gains; `Print.reaching` windows
    with `Levels.gamma`; shadow and highlight ordinates; Color density; pedestal and log guard;
    `Separation.rows` (family, dose, `chromaKept`); the print's exposure, contrast, zones, filters,
    Hunt (1) and `huntBase`; `follow` 0; `ChromaDecompression.weights` with the roll reference's
    span ratio and the balance vectors. The Scanner face and the other modes are refused.
- Shows the result in the canvas card of a 1100 × 680 window.
- Beside it, the panel holds one block, Lighting, with one `DSSlider`: Exposure, −4…4 EV, base 0,
  readout `%+.2f EV`.
- The slider writes `Stage47::print_stops`, and the canvas is re-rendered from the float buffer.

Its pieces now live in `oni-app` (`track.rs`, `motion.rs`, the keycap raster) and the pipeline in
`oni-pipeline`; the crate left the workspace once they did.

**Measured results** (`cargo test --release`: 15 tests pass, 0 fail; `cargo clippy --release
--all-targets` reports no warnings).

| Check | Result | Twin (must fail if the code is wrong) |
|---|---|---|
| **Mapping against the Swift render** (`oni-reference/decode` and `render/00008.f32`, 1000 × 667, linear Rec. 2020, after the output clip) | Mean gap **0.00015**, 99th centile **0.00037**; luma 0.1899 for 0.1900 | Hunt at 0: mean 0.0041, p99 0.023; separation off: 0.0014, 0.0085; roll reference ignored: 0.0063, 0.032 (thresholds 1e-3 and 5e-3) |
| LibRaw decode against `CIRAWFilter`'s (measurement) | **−1.28 / −1.35 / −1.57 EV** per channel | — |
| Canvas shows the kernel's output | 4 blocks of 24 × 24 device pixels within 1.5 levels of `display_rgba8(converted)` (threshold 6) | Against the unconverted negative every block differs by far more (threshold > 20) |
| Opening renders once, readout `+1.50 EV` | 1 render | — |
| Knob raster on screen | Grip line pixel (255, 255, 255); face (49, 49, 49) = albedo #313131 at mid-dish, asserted within 2 | Rail 40 pt left of the knob is (134, 134, 134), not white |
| Drag a quarter of the travel | Exposure 1.5 → 3.5, readout `+3.50 EV`, centre brightens from (152, 141, 139) to (209, 202, 201) | Press and release without travel changes nothing, and no render |
| Option during a drag | Same travel moves 0.4 EV (3.5 → 3.1) | — |
| Ten quick moves | Readout follows each one; release hands the last value | The release does render |
| Double click | Value 0 and readout `+0.00 EV` at once; knob not yet at rest; the rest of that press drags nothing; a third quick press resets again; the spring lands at rest within 2 s | Two presses 600 ms apart do not reset |
| Press during a spring | The knob still lands on its value | The old wiring, which stopped the spring on press, leaves it at 0.625 |
| Scroll | 40 pt along the track moves 10 pt of travel (value 0.2817), one render | A vertical and a mostly vertical scroll move nothing and render nothing |
| Accessibility increment | +0.08 EV (a hundredth of the range) | — |
| Unit: hand-off clock | Now, then one trailing hand-off, then wait; a release clears the pending flag | Without the release clear, the next early value waits on a stopped timer |
| Unit: Frustum walls | .9729 / .7530 / .4629 within 5e-4, top > 1.4 | `midtoneMatch = 0` gives different shades |
| Unit: dish | Face shade 1 at mid-dish | `midtoneMatch = 0` does not |
| Unit: knob layout | Face rect (4.2, 4.2, 15.6, 25.6) | The key style's thicker lower wall gives another rect |
| Unit: raster | 96 × 136 at scale 4; walls within .03 of their flat shade | Left wall ≠ right wall; corner alpha 0 |
| Unit: springs | Settle overshoot 3.84 %; both springs settle within 2e-3 | Travel (critically damped) never overshoots |
| Unit: sidecar mapping | Gains, windows with gamma, ordinates, print with the decoder's clamps, Hunt 1, follow 0, decompression weights | A missing window field is refused; the Scanner face is refused |
| Unit: separation | Rows sum to one; identity at dose 0 | The bare dose without `chromaKept` differs |
| Unit: held points | A sum past 0.98 is held back | The unheld points differ |
| Unit: float reduction | 5e-5 survives the reduction | The same value quantised to 8 bits would be 0 |
| Timing | Decode + reduction 2.9–4.8 s; stages 4–7 on the CPU + encoding 110–215 ms; one software frame 14–19 ms | — |

**Against the Swift app's render of the same frame.**
- Fed the Swift app's own decode, the mapping lands on the Swift render: mean gap 0.00015 in linear
  light. The finishing stages after stage 7 are at rest on this frame (identity curves, flat
  saturation, no vignette), so nothing beyond stages 4–7 is missing from this comparison.
- Fed LibRaw's decode, the canvas is far brighter than the app's (linear luma 0.54 against 0.19).
  The whole gap is the decoder: LibRaw's pinned decode sits **1.28 to 1.57 EV under** `CIRAWFilter`'s
  on this NEF, unevenly per channel, and the sidecar's windows were set on `CIRAWFilter`'s densities.
  That is an `oni-decode` matter, not a UI one; `frame.png` therefore does not show the app's picture.

**Not verified.**
- **A real window.** `cargo run` under Xvfb panics: `Library libxkbcommon-x11.so could not be
  loaded`; there is no sudo to install it. winit, the native menu bar, focus and hover cursors were
  not exercised.
- **femtovg-wgpu / skia and the zero-copy texture import.** They were compiled (features enabled) but
  not run. The software renderer cannot draw a wgpu image, so the canvas went through
  `Image::from_rgba8`.
- **Shadows, rounded clipping, scale transforms.** The software renderer skips them: the card's
  rounded clip and the knob's ×1.07 grow could not be judged on screen. The grow animates
  `width`/`height`, not a transform, so the values change, but the pixels were not compared with
  Swift's.
- **Display colour management** (P3 screens, proofing).
- **The keycap raster against a Swift capture.** The wall shades match the Swift maths, but no pixel
  comparison with a screenshot of the Swift knob was possible (no macOS here). The seam blur sigma and
  the circular corners are assumptions.
- **Tabular figures, haptics, wheel/trackpad distinction, pinch.**
- **The 30 Hz hand-off through the window.** Each synchronous render (110–215 ms) outlasts the interval,
  so the integration test cannot see it. The rate is proven by `track`'s unit test with its twin.

---

## 8. Decisions

- **The PoC tests roll #8, not a synthetic gradient**: the frame is `DSC_6440.NEF`, read with its own
  sidecar. Roll #8 is well exposed and well scanned.
- **8-bit happens only at the display.** Decode, reduction and stages 4–7 are `f32`; `display_rgba8`
  is the only quantisation, mirroring the Swift `bgra8Unorm` drawable and Slint's 8-bit-only texture
  import.
- **Box reduction by a whole factor**, not Lanczos, for the canvas copy. The Swift preview side is
  2000 with Core Image's resampling, so the canvas pixels are not Swift's. This does not affect the UI
  questions under test.
- **The `Stage47` mapping is complete for the Pro face on a colour negative** (section 7) and refuses
  the Scanner face and the other modes rather than misreading them. Fields `Print`'s decoder reads with
  `decodeIfPresent` default as Swift does; the windows, gains and doses are required.
- **The display is encoded as sRGB** (Rec. 2020 → sRGB matrix, hard clip, sRGB transfer).
- **The keycap is a Rust raster**, not Slint shapes. Slint cannot shade per pixel, and the software
  renderer has no shadows; the raster keeps the Frustum maths exact and renders identically on every
  renderer.
- **The double click is detected on press in Rust**, since Slint's `double-clicked` fires on release
  and the Swift slider resets on the second press.
- **Springs are evaluated analytically** in Rust and drive properties through a 16 ms timer, because
  Slint's `animate` has no spring easing.
- **Slint's own `MenuBar` and `ContextMenuArea`** are recommended for menus, since they are native
  through muda. A direct muda dependency only for what they lack (section 1).
- **Licensing is flagged, not decided** (section 1).

---

## 9. Not ported

- Every pane but Lighting's Exposure, and every block but Lighting. Section 4 lists them with their
  constructs.
- The toolbar, gallery, tree, library, export surface, sheets, popovers, menus, key monitor, help
  layer, preset deck, canvas overlays (crop, measuring window, loupe), the Scanner face.
- The wgpu canvas path (`Image::try_from(wgpu::Texture)`, rendering notifier), because it was not
  runnable headlessly.
- The `Stage47` mapping for positive and monochrome modes and the Scanner face, and the finishing
  stages after stage 7.
- The squircle, tabular figures, haptics, keycap sound, pinch and scroll-phase input, colour
  management.
- The slider's wheel path. It is wired with trackpad travel only, since Slint does not say which
  device scrolled.
- The token generator from `DSTokens.swift`. The PoC's `DS` global is a hand-copied subset.
