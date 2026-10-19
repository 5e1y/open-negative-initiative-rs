# The design system

Every screen in `ui/` is built from this folder and nothing else. A look changes in one place
and every screen picks it up.

## Where the values come from

| File | What it holds | Who writes it |
|---|---|---|
| `tokens.slint` | `DS`, every constant of the Swift app's `DSTokens.swift`, under flat names | `cargo run -p oni-app --bin oni-tokens`, never by hand |
| `theme.slint` | The semantic groups: `Colour`, `Space`, `Radius`, `Stroke`, `Type`, `State`, `Motion`, then one `*Style` per component (`ButtonStyle`, `SliderStyle`, `SectionStyle`, `TabsStyle`, `ToggleStyle`, `FieldStyle`, `RowStyle`, `TileStyle`, `SheetStyle`, `MenuStyle`, `TipStyle`, `PlotStyle`, `CardStyle`, `StatusStyle`, `ShellStyle`) | By hand. Each value is bound to a `DS` token wherever the Swift app has one |
| `base.slint`, `controls.slint`, `containers.slint`, `collections.slint` | The components | By hand. They read only `theme.slint` |
| `ds.slint` | The single import screens use | By hand |
| `catalogue.slint` | Every component in each variant and state | By hand |

The test `the_committed_tokens_are_the_swift_ones` fails when `tokens.slint` no longer matches the
Swift source, so the generator stays the only source of `DS`.

## Changing a token

- **Restyle one kind of element** (all buttons, all sliders): edit its group in `theme.slint`, for example
  `ButtonStyle.radius` or `SliderStyle.track`.
- **Change a value the Swift app also uses** (a space, a colour of the palette): change it in
  `DSTokens.swift`, then run `cargo run -p oni-app --bin oni-tokens`.
- **A value with no Swift token** (a channel hue, a medal foil, the tooltip ground): it is written once
  in `theme.slint`, with a comment saying why.

To check the result, run `cargo run --release -p oni-app --bin oni -- --catalogue` (or set
`ONI_CATALOGUE=1`). It opens the catalogue, which shows every component in each state on one page.

## The rule: screens use only the design system

A screen imports from `ds/ds.slint` and nowhere else. It may lay things out with layouts, `Rectangle`
as a plain box, `TouchArea`, `Scroller` and tokens. It never writes a literal value, and never styles
a primitive itself.

`tests/design_system.rs` enforces this, using `src/strict.rs`. It reads every `.slint` file outside
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
It scrolls by the wheel and the trackpad only, and its `ScrollKnob` copies AppKit's overlay scroller:
no gutter, drawn only while the content overflows, lit while it scrolls, `ScrollStyle.linger` after and
while the pointer is on the edge strip, never taking a click. Set `content-height` at every site (the
alias drops the Flickable's own default); `horizontal: true` reads `content-width` instead, and
`knob: false` hides the knob as Swift's `.scrollIndicators(.never)`.

Each refusal has a twin. The same text planted in a copy of a screen must fail the check, and the
same text planted in `ui/ds/` must pass it. `strict::ALLOW` is the list of exceptions. It is empty;
any entry added to it must give its reason.
