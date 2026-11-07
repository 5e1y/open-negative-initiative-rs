# The design system

Every screen in `ui/` is built from this folder and nothing else. A look changes in one place
and every screen picks it up.

## Where the values come from

| File | What it holds | Who writes it |
|---|---|---|
| `tokens.slint` | `DS`, every constant of the Swift app's `DSTokens.swift`, under flat names | `cargo run -p oni-app --bin oni-tokens`, never by hand |
| `theme.slint` | The semantic groups: `Colour`, `Space`, `Radius`, `Stroke`, `State`, `Motion`, then one `*Style` per component (`ButtonStyle`, `SectionStyle`, `RowStyle`, `TileStyle`, `SheetStyle`, `MenuStyle`, `TipStyle`, `CardStyle`, `StatusStyle`, `ShellStyle`) | By hand. Each value is bound to a `DS` token wherever the Swift app has one |
| `theme_type.slint` | `Type`, the type scale, re-exported by `theme.slint` | By hand |
| `theme_fields.slint` | `FieldStyle`, re-exported by `theme.slint` | By hand |
| `theme_sliders.slint` | `SliderStyle`, `DialStyle` and `PlotStyle`, re-exported by `theme.slint`; their in-out values are the ones the lab tunes live | By hand |
| `theme_tabs.slint` | The selection controls' gauges (`TabsStyle`, `ToggleStyle`, `MedalStyle`), re-exported by `theme.slint`; their `in-out` values are the lab's tunables | By hand |
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
pieces the window checks click at fixed places (fields, a scrolled page, a keycap), `probe_states.slint`
the states they read off the drawn window (out of reach, a destructive key, a latch, a tile's hover).

## Words

`Label` sets every word in Inter on one scale (`Type`): `micro` 9.5 pt for a seal or a mini key's legend,
`caption` 11 for a caption, a value or a key's legend, `label` 13 for the words read, `section` 15
semibold for a block's title, `title` 17 bold for a mode's or a sheet's, `heading` 24; `Roles` gives a role's
size and weight to a label and to a field typed over it alike. A value never outsizes its block's title; a
note is a label's size in the secondary ink, its lines opened (`Type.leading`); the tertiary ink stands
at AA's 4.5:1 on the panel's ground. `fonts/Inter.ttf` maps
its digits and its + − × to their tabular forms (the cmap edited once, offline), so a value keeps its
width as it changes, on screen and on a contact sheet's numbers.

## Keys

Every `Button` is flat: a face of the keycap's grey laid flat (`ButtonStyle.face`) inside its own edge, the
dividers' line (`ButtonStyle.edge`), its glyph and word on it, so every key shows its extent at rest and two
keys side by side read apart before the hand reaches either. A `Keycap` stands only where the use site sets
`keycap: true`: the mode switch (EDITOR, GALLERY), a screen's or a sheet's commit (EXPORT, ADD FOLDER…,
PREPARE ROLL, the prepare footer's NEXT, never its Skip, every DONE, GOT IT, GET STARTED, SEND), the
automatic actions run on a click (AUTO WHITE BALANCE, AUTO PAPER CORRECTION, DETECT, REMOVE) and the Scanner
face's step keys, − and + beside each of its rails. A question's answers and a passing notice's OK stay
flat. The cap is a raster of Swift's `KeycapStyle` that Rust draws when the window asks
`Raster.keycap(KeycapLook)`; `oni_app::keycaps::serve` answers it on a window (the `Shell` does it for the
main window and the console; a check building a window alone calls it); an unserved window, as slint-viewer's,
shows each cap's flat stand-in. One cache by the look's numbers (oni-keycap's `look::Cache`, shared with
oni-lab) draws a still key once and a pressed one once a frame.

- **Sizes** (`ButtonSize`): `regular` the 32 pt flat key (40 pt as a keycap), `mini` the 24 pt key of a
  section's header, a row or the canvas's header. A worded key is never narrower than tall.
- **Kinds** (`ButtonKind`): `plain`; `accent`, the one primary of a view, its face the accent sunk into the
  keycap's grey (`ButtonStyle.primary`), calm beside the photo, and grey while out of reach; `destructive`,
  a plain key whose glyph or word takes a light red (`Colour.destructive-ink`) while the hand or the keyboard
  is on it, the plain ink at rest and out of reach, so no red stands beside the photo; `ghost`, a glyph alone on
  no face. Colour says a meaning, never which key it is: no other tint.
- **States**: the hand lights the face (`ButtonStyle.veil`), a press a step more and, on a keycap, sinks
  the cap (`KeycapPress`: `Depth.hover-press` under the hand, the whole way in `Depth.press-time`, held
  `Depth.min-hold`, risen in `Depth.release-time`); `on` lights the face as a selected row is lit, never
  tinted (`ButtonStyle.on`), and keeps the key's glyph, or rings a swatch (`fill`). A `latch` key, a mode's
  as the crop, compare, De-dust and the measuring window, carries a `Lamp` after its word on its middle
  line, dark glass in a light rim at rest (over 3:1 on the face, `SurfaceStyle.lamp-rim`) until `on` lights
  it in the LCD's phosphor (`ButtonStyle.latch`), so it tells it latches before the press. Out of reach, a
  flat key fades whole, face and glyph, to `State.disabled` (0.25), well under an idle tab's label
  (`State.idle`), so a key that cannot act never reads as one merely not chosen; a keycap keeps its relief
  and fades its legend alone, so a pair of keycaps stays a pair while one waits; a missing
  original, a folder away or a frame left out fades less (`State.missing`), its picture and name still read.
- **Groups**: related keys stand in a `ButtonGroup`, `ButtonStyle.group-gap` (`Space.s`) apart as Swift's
  `HStack` of keys, each on its own face; `align: fill` shares the row's width out. A key unrelated to its
  neighbour, or one that removes, resets or empties beside one that keeps, stands `ButtonStyle.apart`
  (twice the group's gap) from it: an `AskSheet` sets its destructive answer so. The bar has the same two
  gaps alone, a group's within and `ShellStyle.bar-gap` between groups; its Reset and Trash are a group each,
  last of the centre, Reset naming the photos it reaches and Trash wearing its can.
- **Plates**: `ButtonGroup { plate: true }` is a socket of keycaps alone, the one place keys share a face:
  its caps flush on one face rounded as a key, as the panel's AUTO WHITE BALANCE with the crop's warning
  over it. Flat keys never stand on a plate.
- **Footers**: every sheet and the prepare and export panels end on one `SheetFooter`: the flat key that
  leaves (Cancel) at the left with what the sheet adds beside it, then apart at the right a step back and
  the commit alone, the accent keycap, a flat key while it writes nothing (Skip, a failure's OK). A key that
  removes (`remove`, a preset's Delete) stands alone at the left end, what the sheet adds then beside the
  commit. Every DONE is that accent keycap, a tool's in the panel as a sheet's, the bar's export laying its
  accent down while a tool is open. A question's answers (`AskSheet`) keep their own row.

## Keyboard

One model for every control, as macOS draws it with full keyboard access:

- **The photos hold the keyboard** (`KeyMonitor`): the grid's or the open photo's keys, every key a control
  lets through. Tab passes over it.
- **Tab walks the controls in reading order** (`KeyScope`): every key, the tabs as one control (← → pick), every
  slider (← → ↑ ↓ one step, ⇧ `big`), the switch, the picker (Space, Return or ↓ drops it), each medal, an
  editable title, the grid (its photo ringed) and the open photo. Space or Return presses a key or flips a switch.
- **One ring** (`FocusRing`, `FocusStyle`): the accent around what holds the keyboard, a gap off its edge;
  a control shows it once Tab gives it the keyboard, a field while it is typed in (`FieldFace { lit }`).
- **A click gives the keyboard to a folder's row alone**: any other press gives it back to the photos, a field's
  own aside (the window's pointer hook counts each press in `Outside`), and a press on the grid or the open photo
  hands it to them itself; a slider dragged or typed into leaves it with them; Escape on a control gives it back too.
- **Tab from the photo enters the editor's panel** at its tab bar (`Keyboard.inward`), the short way to its
  sliders; the walk goes on from there through the window. A key whose holder vanished goes to the monitor
  (the window's key hook, `oni_app::outside::keyed`), and a new screen or a sheet takes the keyboard back.
- **A sheet keeps Tab inside it**: the monitor's two walls, Tab stops only while `Keyboard.covered`, hand the
  keyboard to the sheet's first control going forward and to its last going back.
- **A sheet's default key holds the keyboard** (`Button { default }`): a footer's commit, a question's
  confirming answer, a bar's exit, so Return presses it and an assistive tool lands on it; a field the sheet
  opens on keeps it, a question whose key removes has none, nor has a panel's footer or the welcome's, read
  from its top (`default-commit: false`), as a sheet scrolls to what holds the keyboard. Escape reaches the
  sheet from any of its keys. Behind a sheet the window's bar (`in-bar`) is out of reach.
- **`Keyboard.held`**: the keyboard is the window's, not a field's; the menus' bare keys (the medals, the
  brush) answer only then.

## Fields

Inputs and selects are flat, no well and no relief: every text field is Swift's `dsTextField` laid flat on a
`FieldFace`, the canvas's dark at a card's corner, `FieldStyle.across` either side and `down` above and below a
multiline one, its edge lit and ringed while it holds the focus (`focused-look` forces it). Disabled, the words
fade and the chrome stays. The picker's field wears the same face.

- **`TextField`**: one entry; `suffix` sits at the face's end, `lines-height` is the text's height inside
  the padding. Entered by a click, Tab or its owner, it selects its value; then a click places the caret,
  a double click selects a word, a triple click the line (the `text_fields` window check).
- **`RenameField`**: a `TextField` that takes the focus at once; `role: title` and `outdent` for a title,
  the face led out by its padding so the name typed stays where the title stood.
- **`SizeFields`, `DateFields`**: two or three centred entries on one face a field tall, lit while any
  of them holds the focus; the size boxes fill their row, the date boxes hug theirs.
- **`Field`** lays a label above (`stacked`, with its help mark and the mark's card), beside (`column`) or spread
  before it. A help mark lays out at its glyph's size and catches the hand over a small key's square, so a line
  keeps its height.

## Sliders and plots

- **Rail** (`DSRail` in `dsRecessed`): 8 pt, round at its ends, thin so a bipolar rail keeps its full hues
  beside the photo; `SliderStyle.ground`, 3:1 against the panel, or the bipolar gradient, the fill
  (`SliderStyle.lit`) on its own round, then `Inward`'s shade, the black edge and the lit rim (`SliderStyle.rim`)
  over an opaque `under`; `vertical` runs low to high upwards.
- **Knob** (`DSBarHandle`): a `Keycap` at `SliderStyle.knob-factor`, 20 by 26 pt, level walls, no shadow, edged in
  black, two grip lines centred on its face, white, or the on-light ink on a light cap (`grip-turn`). It grows by
  a scale on the rise spring, so its raster is drawn once.
- **Slider**: the rail and the knob move together on the settle spring for a value from elsewhere, never under
  the hand; the stepped slider's knob carries its grip too, its − and + keycaps as tall as the knob
  (`SliderStyle.step-key`), each in the colour of the rail's end beside it, a step a press and three with ⇧;
  a continuous slider has none. AppKit's press, continuous or stepped: the knob pressed is
  grabbed where it stands, a press on the rail lands it under the pointer, ⌥ keeps it put; a press gives it
  the keyboard unringed, Tab or an arrow acting on it rings it. The hand on the row grows the knob and lights
  the value; the rail's tooltip tells its double click back to rest. The value is a field: its box, a field
  tall, shows the field's face and a text cursor under the hand, and a click types a number over it
  (`Readouts.steps`, answered by Rust), reached by the steps an arrow takes. Its unit stands in a column
  `SliderStyle.unit` wide (`Readouts.unit`, what follows the last digit), so the digits of every row end at one edge.
- **Dial** (`Slider { dial }`, `Dial`): a value that is truly a turn, the straighten's angle, as a camera's command
  dial seen edge on: a black-anodised ring in an `Inward` slot, its scale in the value's unit (`lowest` to `highest`)
  foreshortened and shaded toward the ends, a fixed index at its middle. The scale moves with the hand, so a drag
  right brings the lower figures under the index, and a press never jumps it; the ring turns `DialStyle.turn` a
  unit. Every other behaviour is the slider's.
- **Plot** (`dsPlotScreen`): the canvas, the quarters engraved, the content, then `Inward { hollow }` and the rim;
  `screen: false` is the panel's bare histogram. `CurvePoint` is `DSHandle`, the disc a curve point shows.

## Selection controls

The tabs, the switch and the medals stand on the depths as the lab's Tabs page shows them; the picker's
field lies flat, as a text field.

- **`Tabs`** (`DSTabBar`): an `Inward` hollow at `TabsStyle.track-shade`, lighter than a plot's, and one
  `Outward` pill sliding on the grab spring (`TabsStyle.slide`); an unchosen label rests at `TabsStyle.idle`,
  5.6:1 on the hollow, hover lights it alone, a tab out of reach fades to `State.disabled`. A tab is its label
  and an equal share of the rest, `TabsStyle.room` either side where the bar holds it, never under `margin`. `marked`
  adds the accent dot, `swatches` draws colours for words; `ordered` leads each of a walk's steps with its
  number in a disc, the accent check there once `marked` says it is done.
- **`Toggle`** (`DSToggle`): the slider's `Knob` in a slot of its own (`ToggleStyle.slot`), an `Inward`
  hollow cut to the canvas as the tab bar's, never a slider's grey rail; on, the knob stands one
  `ToggleStyle.travel` along on the grab spring. What it uncovers names the state by the lamps' rule: on, the
  slot lit up to it in a lit lamp's phosphor (`ToggleStyle.lit`); off, a `Lamp` at rest in the hollow. The knob
  grows under the hand.
- **`Picker`**, field face (`DSPicker`): the value on a text field's flat `FieldFace`, a chevron at its end, lit
  under the hand as a flat key's face; the pill and chip faces keep the hover ground. A pill that only names
  what its column lists is `pressable: false`, never disabled, so its words keep the primary ink.
- **`Medal`**: a foil disc of its metal, an icon's size on a tile and in the rating filter, its podium place sunk
  in the middle as the key that lays it (gold 1, silver 2, bronze 3), in the ruled ring's ink over a lip lit a rule
  below, so no medal is told by its colour alone; `blank`, one not given, its rim lit on top and its figure alone in
  its metal, brighter `reached`. `MedalPicker` leads the status bar's line, by the photo's name: "Rating", then the
  podium at a small key's size, the given medal struck, the others blank and lit under the hand, a pressed one shrunk
  on the spring, each catching the hand over a small key's square.

## Surfaces

What stands out is an `Outward` (Swift's `dsFloating`), what sinks an `Inward` (`dsRecessed`):

- **Raised**: every lift on `Outward`, its lit rim, its face lighter at the top, its inner hairline and its
  cast shade; `sheet` lengthens the shadow (`Outward.lift` 1). The sheets, the notices, the banners, the
  slid columns and the popups stand on it. **Popover** hangs a `Raised` under its tail (`DSPopoverShape`),
  `tail-x` below zero centring it; `PopupPanel` draws one, a menu without a tail.
- **Inward**: `Inset`, `Progress` (its fill glowing), `LcdReadout` (a figure the app reports: the crop's and the
  export's sizes, the export's weight, the version, a count; in the words' own ink) and
  the passes' reorder hollow. Its children lie under the shade, as Swift's overlay; it clips to its round, so a
  gallery's many tiles keep their flat well.
- **Past the bounds**: an `Inward`'s lit lip reaches `Depth.lip-reach` under it and an `Outward`'s shadow
  falls around it, as Swift's. A `Section` clips only while folded; a scrolled column ending on a well
  leaves `Depth.lip-reach` under it.
- **Cards**: `PresetFace` (`lifted` lengthens its shadow while the deck raises it), `WrappedCard` and
  `TipCard` (a tooltip) are `Outward`s; `HelpCard` is an `Inset`; the canvas card stays flat, as Swift's.
  The deck opens on `PresetTab`, a flat outline the canvas's edge cuts open, lit while the deck is open.
- **Divider** is `DSSeparator`'s engraved groove and lit edge, two lines; `hairline` or `engraved: false`
  (a menu's rule) keeps a flat line. Every title, a `Section`'s, a column's, a sheet's, is `Engraved`
  alike, a camera's marking in its own case: the groove's shade a line above its letters, the lit edge a line
  below. Capitals belong to the keycaps alone. On a short row a `Section`'s title yields, so the reset and
  fold keys keep to the column's edge; the reset stands there always, dimmed at rest, a step apart from the
  fold key. A help mark, named with its block, opens its card in the block's flow under the title, pushing the
  block down, one card at a time; its note comes in short parts a blank line apart.
- **GuideLine** draws a guide over a photo, the crop's thirds and the compare split: its light line edged a
  line either side by the contrast shade, so it reads on a bright sky as on a shadow.
- **Seals** (`Badge { tone: seal }`) print `DSGoldSealView`: brushed foil in layers rounded as the capsule (no
  clip, no layer), the rule, the embossed ink; the glint sweeps once on `glinting` or a hovered note, clipped
  only while it crosses; `glint-at` holds it still. **Lamp** is a key's or a switch's state, dark glass in a light rim at rest, glowing its colour when lit.

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
