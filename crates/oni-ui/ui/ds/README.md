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
| `keyboard.slint` | The keyboard: `Keyboard` (who holds it, a sheet covering the window), `FocusRing`, `KeyScope` and `KeyMonitor` | By hand |
| `base.slint`, `badge.slint`, `controls.slint`, `fields.slint`, `outside.slint` (the press beside a field or a menu, the window's size), `window.slint` (`AppWindow`), `menu.slint` (menus and their submenus), `picker.slint`, `containers.slint`, `collections.slint`, `marks.slint` (discs, rings, traces), `surfaces.slint` (the lamp, the popover), `crt.slint` (the scanner's glass, `CrtStyle`'s phosphor: the Scanner face's screen and the measure's bank) | The components | By hand. They read only `theme.slint` and `depth.slint` |
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

## Words

`Label` sets every word in Inter: a block's title (`section`) a step heavier than the words under it, a
note in the secondary ink, the tertiary ink at AA's 4.5:1 on the panel's ground. `fonts/Inter.ttf` maps
its digits and its + − × to their tabular forms (the cmap edited once, offline), so a value keeps its
width as it changes, on screen and on a contact sheet's numbers.

## Keys

Every `Button` is flat: a face of the keycap's grey laid flat (`ButtonStyle.face`), its glyph and word on
it. A `Keycap` stands only where the use site sets `keycap: true`: the mode switch (EDITOR, GALLERY), a
screen's commit (EXPORT, ADD FOLDER…, PREPARE ROLL, the prepare footer's NEXT, every DONE) and the
automatic actions run on a click (AUTO WHITE BALANCE, AUTO PAPER CORRECTION, DETECT, REMOVE). The cap
is a raster of Swift's `KeycapStyle` that Rust draws when the window asks `Raster.keycap(KeycapLook)`;
`oni_app::keycaps::serve` answers it on a window (the `Shell` does it for the main window and the
console; a check building a window alone calls it); an unserved window, as slint-viewer's, shows each
cap's flat stand-in. One cache by the look's numbers (oni-keycap's `look::Cache`, shared with oni-lab)
draws a still key once and a pressed one once a frame.

- **Sizes** (`ButtonSize`): `regular` the 32 pt flat key (40 pt as a keycap), `plate` a panel's or a
  footer's at the same height, `mini` the 24 pt key of a section's header, a row or the canvas's header.
  A worded key is never narrower than tall.
- **Kinds** (`ButtonKind`): `plain`; `accent`, the one primary of a view, its face the accent; `destructive`,
  a plain key whose glyph or word takes a light red (`Colour.destructive-ink`); `ghost`, a glyph alone on no face. Colour says a
  meaning, never which key it is: no other tint.
- **States**: the hand lights the face (`ButtonStyle.veil`), a press a step more and, on a keycap, sinks
  the cap (`KeycapPress`: `Depth.hover-press` under the hand, the whole way in `Depth.press-time`, held
  `Depth.min-hold`, risen in `Depth.release-time`); `on` lights the face with the accent sunk into the
  ground (`ButtonStyle.on`) and keeps the key's glyph, or rings a swatch (`fill`); out of reach, the whole
  key fades, face and glyph.
- **Plates**: `ButtonGroup { plate: true }` sets its keys flush on one face rounded as a key, so a key
  under the hand or on lights alone; flat keys beside a keycap stand on the plate's middle. Without
  `plate` it is a spaced row, `Space.s` apart as Swift's `HStack` of keys.

## Keyboard

One model for every control, as macOS draws it with full keyboard access:

- **The photos hold the keyboard** (`KeyMonitor`): the grid's or the open photo's keys, every key a control
  lets through. Tab passes over it.
- **Tab walks the controls in reading order** (`KeyScope`): every key, the tabs as one control (← → pick), every
  slider (← → ↑ ↓ one step, ⇧ `big`), the switch, the picker (Space, Return or ↓ drops it), each medal, an
  editable title, the grid (its photo ringed) and the open photo. Space or Return presses a key or flips a switch.
- **One ring** (`FocusRing`, `FocusStyle`): the accent around what holds the keyboard, a gap off its edge;
  a control shows it once Tab gives it the keyboard, a field while it is typed in (`Inward { ringed }`).
- **A click gives the keyboard to a slider alone**: any other press gives it back to the photos, a field's own
  aside (the window's pointer hook counts each press in `Outside`); Escape on a slider gives it back too.
- **Tab from the photo enters the editor's panel** at its tab bar (`Keyboard.inward`), the short way to its
  sliders; the walk goes on from there through the window. A key whose holder vanished goes to the monitor
  (the window's key hook, `oni_app::outside::keyed`), and a new screen or a sheet takes the keyboard back.
- **A sheet keeps Tab inside it**: the monitor's two walls, Tab stops only while `Keyboard.covered`, hand the
  keyboard to the sheet's first control going forward and to its last going back.
- **`Keyboard.held`**: the keyboard is the window's, not a field's; the menus' bare keys (the medals, the
  brush) answer only then.

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
- **`Field`** lays a label above (`stacked`, with its help mark), beside (`column`) or spread before it. A help
  mark lays out at its glyph's size and catches the hand over a small key's square, so a line keeps its height.

## Sliders and plots

- **Rail** (`DSRail` in `dsRecessed`): `SliderStyle.ground`, 3:1 against the panel, or the bipolar gradient
  between soft hues (`hue-chroma`, `hue-light`), the fill (`SliderStyle.lit`) on its own round, then
  `Inward`'s shade, the black edge and the lit rim (`SliderStyle.rim`) over an opaque `under`; `vertical` runs low to high upwards.
- **Knob** (`DSBarHandle`): a `Keycap` at `SliderStyle.knob-factor`, 20 by 26 pt, level walls, no shadow, edged in
  black, two white grip lines centred on its face. It grows by a scale on the rise spring, so its raster is drawn once.
- **Slider**: the rail and the knob move together on the settle spring for a value from elsewhere, never under
  the hand; the stepped slider's knob carries its grip too, its − and + flat keys as tall as the knob, the
  rail beside them showing each end's colour. AppKit's press, continuous or stepped: the knob pressed is
  grabbed where it stands, a press on the rail lands it under the pointer, ⌥ keeps it put. The hand on the
  row grows the knob and lights the value; a double click on the value types a number over it (`Typing`,
  answered by Rust), reached by the steps an arrow takes.
- **Plot** (`dsPlotScreen`): the canvas, the quarters engraved, the content, then `Inward { hollow }` and the rim;
  `screen: false` is the panel's bare histogram. `CurvePoint` is `DSHandle`, the disc a curve point shows.

## Selection controls

The tabs, the switch, the picker and the medals stand on the depths as the lab's Tabs page shows them.

- **`Tabs`** (`DSTabBar`): an `Inward` hollow at `TabsStyle.track-shade`, lighter than a field's, and one
  `Outward` pill sliding on the grab spring (`TabsStyle.slide`); hover lights a label alone. `marked`
  adds the accent dot, `swatches` draws colours for words.
- **`Toggle`** (`DSToggle`): a hollow holding a cross and a check, the white `Outward` thumb on the state that holds,
  on the middle of a small key's height so the hand finds it as a key.
- **`Picker`**, field face (`DSPicker`): the value in a hollow, a chevron at its end, the hollow lit under the
  hand as a flat key's face; the pill and chip faces keep the hover ground.
- **`Medal`**: a flat disc of its metal's colour; `MedalPicker` lights the medal under the hand, shrinks a
  pressed one on the spring and catches the hand over a small key's square round each.

## Surfaces

What stands out is an `Outward` (Swift's `dsFloating`), what sinks an `Inward` (`dsRecessed`):

- **Raised**: every lift on `Outward`, its lit rim, its face lighter at the top, its inner hairline and its
  cast shade; `sheet` lengthens the shadow (`Outward.lift` 1). The sheets, the notices, the banners, the
  slid columns and the popups stand on it. **Popover** hangs a `Raised` under its tail (`DSPopoverShape`),
  `tail-x` below zero centring it; `PopupPanel` draws one, a menu without a tail.
- **Inward**: `Inset`, `Progress` (its fill glowing), `LcdReadout` (a count in the words' own ink) and
  the passes' reorder hollow. Its children lie under the shade, as Swift's overlay; it clips to its round, so a
  gallery's many tiles keep their flat well.
- **Past the bounds**: an `Inward`'s lit lip reaches `Depth.lip-reach` under it and an `Outward`'s shadow
  falls around it, as Swift's. A `Section` clips only while folded; a scrolled column ending on a well
  leaves `Depth.lip-reach` under it.
- **Cards**: `PresetFace` (`lifted` lengthens its shadow while the deck raises it), `WrappedCard` and
  `TipCard` (`DSHelpBubble`, on the canvas's ground) are `Outward`s; the canvas card stays flat, as Swift's.
- **Divider** is `DSSeparator`'s engraved groove and lit edge, two lines; `hairline` or `engraved: false`
  (a menu's rule) keeps a flat line.
- **GuideLine** draws a guide over a photo, the crop's thirds and the compare split: its light line edged a
  line either side by the contrast shade, so it reads on a bright sky as on a shadow.
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
