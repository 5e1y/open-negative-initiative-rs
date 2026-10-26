# The design system

Every screen in `ui/` is built from this folder and nothing else. A look changes in one place
and every screen picks it up.

## Where the values come from

| File | What it holds | Who writes it |
|---|---|---|
| `tokens.slint` | `DS`, every constant of the Swift app's `DSTokens.swift`, under flat names | `cargo run -p oni-app --bin oni-tokens`, never by hand |
| `theme.slint` | The semantic groups: `Colour`, `Space`, `Radius`, `Stroke`, `Type`, `State`, `Motion`, then one `*Style` per component (`ButtonStyle`, `SectionStyle`, `RowStyle`, `TileStyle`, `SheetStyle`, `MenuStyle`, `TipStyle`, `CardStyle`, `StatusStyle`, `ShellStyle`) | By hand. Each value is bound to a `DS` token wherever the Swift app has one |
| `theme_fields.slint` | `FieldStyle`, re-exported by `theme.slint` | By hand |
| `theme_sliders.slint` | `SliderStyle` and `PlotStyle`, re-exported by `theme.slint`; their in-out values are the ones the lab tunes live | By hand |
| `theme_tabs.slint` | The selection controls' gauges (`TabsStyle`, `ToggleStyle`, `PickerStyle`, `MedalStyle`), re-exported by `theme.slint`; their `in-out` values are the lab's tunables | By hand |
| `depth.slint` | The three depths every 2.5D element stands on: `Keycap` (Swift's lit frustum, rasterised by oni-keycap through the `Raster` global), `Outward` (a raised surface) and `Inward` (a sunk well), and `Depth`, their tunables | By hand. The 2.5D lab (`crates/oni-ui/lab`, `scripts/ds-lab`) re-exports it and tunes `Depth` live |
| `theme_surfaces.slint` | `SurfaceStyle` (the glass's and the lamp's glow, the foil's light, the progress's phosphor: in-out, so the lab tunes them live), `SealStyle` and `Foil`, `DSFoil`'s ramps; re-exported by `theme.slint` | By hand, the lab's approved values |
| `base.slint`, `badge.slint`, `controls.slint`, `fields.slint`, `outside.slint` (the press beside a field or a menu), `containers.slint`, `collections.slint`, `marks.slint` (discs, rings, traces), `surfaces.slint` (the lamp, the popover), `crt.slint` (the scanner's glass, `CrtStyle`'s phosphor: the Scanner face's screen and the measure's bank) | The components | By hand. They read only `theme.slint` and `depth.slint` |
| `ds.slint` | The single import screens use | By hand |

The test `the_committed_tokens_are_the_swift_ones` fails when `tokens.slint` no longer matches the
Swift source, so the generator stays the only source of `DS`.

## Changing a token

- **Restyle one kind of element** (all buttons, all sliders): edit its group in `theme.slint`, for example
  `ButtonStyle.radius` or `SliderStyle.track`.
- **Change a value the Swift app also uses** (a space, a colour of the palette): change it in
  `DSTokens.swift`, then run `cargo run -p oni-app --bin oni-tokens`.
- **One name per value**: a group never renames another's value (a field's height is
  `ButtonStyle.height`, a socket `Colour.well`); it names only a role of its own that borrows a step
  of a scale (`SectionStyle.gap: Space.s`).
- **A value with no Swift token** (a channel hue, a medal foil, the tooltip ground): it is written once
  in `theme.slint`, with a comment saying why.

To see the result, run `scripts/ds-lab`: the 2.5D lab (`crates/oni-ui/lab`) shows every family in its
states, reloads on each save and tunes `Depth` live; `scripts/ds-lab --shot FILE PAGE WxH` renders a page.
The app holds no showcase of its own, so its interface compiles without one. `probes.slint` holds the
pieces the window checks click at fixed places (fields, a scrolled page, a keycap).

## Keycaps

Every `Button` but the ghost is a `Keycap`: a raster of Swift's `KeycapStyle` that Rust draws when the
window asks `Raster.keycap(KeycapLook)`. `oni_app::keycaps::serve` answers it on a window (the `Shell`
does it for the main window and the console; a check building a window alone calls it);
an unserved window, as slint-viewer's, shows each cap's flat stand-in. One cache by the look's numbers
(oni-keycap's `look::Cache`, shared with oni-lab) draws a still key once and a pressed one once a frame.

- **Sizes** (`ButtonSize`): `regular` is `DSPanelButton`'s standard 40 pt key, `plate` its 52 pt `.plate`
  key (the panels' and footers' plates), `small` the same small
  variant at 24, `mini` the 26 pt mini key of the sections and the header plates, `key` the scanner's
  46 pt key; a stepped slider's keys are `key`s at `SliderStyle.step-key` with a larger `legend-size`.
- **Colour**: a kind's tint or `fill` is the cap's albedo, else the keycap grey; `on` makes a plain key
  pale (`DSPanelButton(pale:)`) and rings a tinted one. The legend inks dark on a light cap, light on a
  dark one (`KeyInk.light`); a disabled key fades its legend alone.
- **Press** (`KeycapPress`): the hand sinks the cap `Depth.hover-press`, a press sinks it whole in
  `Depth.press-time`, held down `Depth.min-hold` however short the tap, risen in `Depth.release-time`.
- **Plates**: `ButtonGroup { plate: true }` sets its keys flush on the black socket (`KeycapPlate`), as
  tall as they are, its corner the cap's outer one (`Depth.corner` × the variant's factor), `mini` for a
  plate of mini keys; without `plate` it is a spaced row, `Space.s` apart as Swift's `HStack` of keys.

## Fields

Every text field is Swift's `dsTextField`: an `Inward { hollow: true }` floored in the canvas, a card's
corner, `FieldStyle.across` either side and `down` above and below a multiline one, its rim lit while it
holds the focus (`focused-look` forces it). Disabled, the words fade and the chrome stays.

- **`TextField`**: one entry; `suffix` sits at the hollow's end, `lines-height` is the text's height inside
  the padding. Entered by a click, Tab or its owner, it selects its value; then a click places the caret,
  a double click selects a word, a triple click the line (the `text_fields` window check).
- **`RenameField`**: a `TextField` that takes the focus at once; `role: title` and `outdent` for a title,
  the hollow led out by its padding so the name typed stays where the title stood.
- **`SizeFields`, `DateFields`**: two or three centred entries in one hollow a keycap tall, lit while any
  of them holds the focus; the size boxes fill their row, the date boxes hug theirs.
- **`Field`** lays a label above (`stacked`, with its help mark), beside (`column`) or spread before it.

## Sliders and plots

- **Rail** (`DSRail` in `dsRecessed`): the track grey or the bipolar gradient, the fill on its own round, then
  `Inward`'s shade, the black edge and the lit rim (`SliderStyle.rim`) over an opaque `under`; `vertical` runs low to high upwards.
- **Knob** (`DSBarHandle`): a `Keycap` at `SliderStyle.knob-factor`, level walls, no shadow, edged in black, two
  white grip lines centred on its face. It grows by a scale on the rise spring, so its raster is drawn once.
- **Slider**: the rail and the knob move together on the settle spring for a value from elsewhere, never under
  the hand; the stepped slider's knob carries its grip too.
- **Plot** (`dsPlotScreen`): the canvas, the quarters engraved, the content, then `Inward { hollow }` and the rim;
  `screen: false` is the panel's bare histogram. `CurvePoint` is `DSHandle`, the disc a curve point shows.

## Selection controls

The tabs, the switch, the picker and the medals stand on the depths as the lab's Tabs page shows them.

- **`Tabs`** (`DSTabBar`): an `Inward` hollow at `TabsStyle.track-shade`, lighter than a field's, and one
  `Outward` pill sliding on the grab spring (`TabsStyle.slide`); hover lights a label alone. `marked`
  adds the accent dot, `swatches` draws colours for words, `dimmed` fades a tab that changes something else.
- **`Toggle`** (`DSToggle`): a hollow holding a cross and a check, the white `Outward` thumb on the state that holds.
- **`Picker`**, field face (`DSPicker`): the value in a hollow, the chevron on a mini `Keycap` that sinks
  `Depth.hover-press` under the hand and whole on a press; the pill and chip faces keep the hover ground.
- **`Medal`** (`DSFoil.medal`): spun metal drawn by gradients; `MedalPicker` shrinks a pressed medal on the spring.

## Surfaces

What stands out is an `Outward` (Swift's `dsFloating`), what sinks an `Inward` (`dsRecessed`):

- **Raised**: every lift on `Outward`, its lit rim, its face lighter at the top, its inner hairline and its
  cast shade; `sheet` lengthens the shadow (`Outward.lift` 1). The sheets, the notices, the banners, the
  slid columns and the popups stand on it. **Popover** hangs a `Raised` under its tail (`DSPopoverShape`),
  `tail-x` below zero centring it; `PopupPanel` draws one, a menu without a tail.
- **Inward**: `Inset`, `Progress` (its fill glowing), `LcdReadout` (its ink glowing) and the passes'
  reorder hollow. Its children lie under the shade, as Swift's overlay; it clips to its round, so a
  gallery's many tiles keep their flat well.
- **Past the bounds**: an `Inward`'s lit lip reaches `Depth.lip-reach` under it and an `Outward`'s shadow
  falls around it, as Swift's. A `Section` clips only while folded; a scrolled column ending on a well
  leaves `Depth.lip-reach` under it.
- **Cards**: `PresetFace` (`lifted` lengthens its shadow while the deck raises it), `WrappedCard` and
  `TipCard` (`DSHelpBubble`, on the canvas's ground) are `Outward`s; the canvas card stays flat, as Swift's.
- **Divider** is `DSSeparator`'s engraved groove and lit edge, two lines; `hairline` or `engraved: false`
  (a menu's rule) keeps a flat line.
- **Seals** (`Badge { tone: seal }`) print `DSGoldSealView`: brushed foil in layers rounded as the capsule (no
  clip, no layer), the rule, the embossed ink; the glint sweeps once on `glinting` or a hovered note, clipped
  only while it crosses; `glint-at` holds it still. **Lamp** is `DSLamp`, at a key's legend's top right.

## The rule: screens use only the design system

A screen imports from `ds/ds.slint` and nowhere else. It may lay things out with layouts, `Rectangle`
as a plain box, `TouchArea`, `Scroller` and tokens. It never writes a literal value, and never styles
a primitive itself.

`oni-app/tests/window/design_system.rs` enforces this, using `tests/window/strict.rs` beside it. It reads every `.slint` file outside
`ui/ds/` (comments and strings blanked) and refuses:

- a literal length other than zero (`px`, `phys-px`, `rem`, `pt`, `%`…), a literal duration, or an angle;
- a colour literal: `#…`, `rgb()`, `rgba()`, `hsl()`, `hsla()`, a gradient, `Colors.…` or a named CSS colour;
- typography: `font-size`, `font-weight`, `font-family`, `letter-spacing` and the window's `default-font-*`;
- styling: `background`, `color`, `colorize`, `border-*`, `drop-shadow-*`;
- an `opacity` written as a number other than 0 or 1;
- the primitives the design system wraps: `Text`, `TextInput`, `Image`, `PopupWindow`, `Path`, `Window`,
  `Flickable`, `ScrollView` (use `Label`, `TextField`, `Icon`/`Picture`, `Menu`, `AppWindow`, `Scroller`);
- importing `std-widgets.slint`: the design system draws its own widgets, none from the std styles.

## Scrolled surfaces

Every surface that scrolls is a `Scroller` (or a `ListView`, the same with its `for` rows virtualised).
It scrolls by the wheel, the trackpad and its knob, never by a drag of the content, and its `ScrollKnob`
copies AppKit's overlay scroller: no gutter, drawn only while the content overflows, lit while it
scrolls, `ScrollStyle.linger` after and while the pointer is on the edge strip, where it widens, drags,
pages a press on the track and jumps to the spot with Option. Set `content-height` at every site (the
alias drops the Flickable's own default); `horizontal: true` reads `content-width` instead, and
`knob: false` hides the knob as Swift's `.scrollIndicators(.never)`.

Each refusal has a twin. The same text planted in a copy of a screen must fail the check, and the
same text planted in `ui/ds/` must pass it. `strict::ALLOW` is the list of exceptions. It is empty;
any entry added to it must give its reason.
