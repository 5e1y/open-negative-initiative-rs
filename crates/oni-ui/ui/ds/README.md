# The design system

Every screen in `ui/` is built from this folder and nothing else. A look changes in one place
and every screen picks it up.

## Where the values come from

| File | What it holds | Who writes it |
|---|---|---|
| `tokens.slint` | `DS`, every constant of the Swift app's `DSTokens.swift`, under flat names | `cargo run -p oni-app --bin oni-tokens`, never by hand |
| `theme.slint` | The semantic groups: `Colour`, `Space`, `Radius`, `Stroke`, `Type`, `State`, `Motion`, then one `*Style` per component (`ButtonStyle`, `SliderStyle`, `SectionStyle`, `TabsStyle`, `ToggleStyle`, `FieldStyle`, `RowStyle`, `TileStyle`, `SheetStyle`, `MenuStyle`, `TipStyle`, `PlotStyle`, `CardStyle`, `StatusStyle`, `ShellStyle`) | By hand. Each value is bound to a `DS` token wherever the Swift app has one |
| `depth.slint` | The three depths every 2.5D element stands on: `Keycap` (Swift's lit frustum, rasterised by oni-keycap through the `Raster` global), `Outward` (a raised surface) and `Inward` (a sunk well), and `Depth`, their tunables | By hand. The 2.5D lab (`crates/oni-ui/lab`, `scripts/ds-lab`) re-exports it and tunes `Depth` live |
| `base.slint`, `badge.slint`, `controls.slint`, `containers.slint`, `collections.slint`, `marks.slint` (discs, rings, traces) | The components | By hand. They read only `theme.slint` and `depth.slint` |
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

- **Sizes** (`ButtonSize`): `regular` is `DSPanelButton`'s standard 40 pt key, `small` the same small
  variant at 24, `mini` the 26 pt mini key of the sections and the header plates, `key` the scanner's
  46 pt key; a stepped slider's keys are `key`s at `ScannerStyle.step-key` with a larger `legend-size`.
- **Colour**: a kind's tint or `fill` is the cap's albedo, else the keycap grey; `on` makes a plain key
  pale (`DSPanelButton(pale:)`) and rings a tinted one. The legend inks dark on a light cap, light on a
  dark one (`KeyInk.light`); a disabled key fades its legend alone.
- **Press** (`KeycapPress`): the hand sinks the cap `Depth.hover-press`, a press sinks it whole in
  `Depth.press-time`, held down `Depth.min-hold` however short the tap, risen in `Depth.release-time`.
- **Plates**: `ButtonGroup { plate: true }` sets its keys flush on the black socket (`KeycapPlate`),
  `small` for a plate of mini keys.

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
