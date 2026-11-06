# UI bench

The UI bench rates what one still screen of ONI shows, in one of its states: empty, loading, filled,
failed; hovered, pressed, disabled, selected. How the app behaves over a task is the UX bench's
([ux-bench.md](ux-bench.md)); where a category borders on one of its criteria it names it, so
nothing is rated twice.

Each category says what it judges, what to look at, and the two ends of its scale.

## 1. Typography

- One type scale from the design system's tokens: few sizes and weights, each with one role.
- Legible at the panels' sizes on 1× and 2× screens; numbers in tabular figures and aligned on their
  units, so a readout does not jitter as its digits change.
- One case style per role (labels, buttons, headers); no clipped or ellipsed text in English,
  French or Spanish.
- **Excels:** the type alone tells a title from a label from a value. **Fails:** six sizes for three
  roles, proportional digits in a readout, a French label cut off.

## 2. Hierarchy

- The order of attention matches the order of importance: the image, the active tool, its main
  action, the secondary ones, the chrome.
- One primary action per group; groups read by proximity, size and weight before any border.
- **Excels:** a squint shows the screen's structure. **Fails:** everything at the same weight, two
  primary actions competing.

## 3. Spacing and density

- Every gap from the design system's tokens, on one grid, with a steady rhythm.
- A density fit for a professional tool: compact, never cramped, the image given its room.
- Alignment exact to the pixel, and optical where the eye asks for it (icons, round shapes).
- **Excels:** every edge lines up with another for a reason. **Fails:** a one-off gap, two rows that
  almost align, a panel filled to its edges.

## 4. Color and contrast

- The interface stays neutral around the image: no tinted grey and no saturated colour beside the
  photo, which would bias the eye judging it.
- Colour carries meaning and is spent sparingly: one accent, states, warnings.
- Contrast at WCAG AA at least: 4.5:1 for text, 3:1 for large text, controls and their states
  (WCAG 1.4.3 and 1.4.11).
- The overlays on the image (crop frame, measuring window, de-dust veil and marks) read on any
  photo, light or dark.
- **Excels:** the photo is the only colour on screen that matters. **Fails:** a grey text under
  4.5:1, a coloured surface beside the photo, an overlay lost on a light photo.

## 5. Iconography

- One family: one grid, one stroke, one corner radius, sizes from the tokens.
- Each icon reads without its label, or carries one; one metaphor per concept, kept everywhere.
- Sharp at 1× and 2×.
- **Excels:** an icon learnt once reads everywhere. **Fails:** mixed strokes, two icons for one
  action, a blurred icon.

## 6. Consistency and polish

- The same component looks the same everywhere: buttons, sliders, tabs, menus, fields.
- No size, colour or radius outside the design system's tokens.
- Every state drawn for every component: hovered, pressed, focused, disabled, selected.
- The depth effects (keycap, outward, inward) applied by the same rule everywhere.
- Nothing stray: no clipped text, broken border, misaligned baseline or leftover placeholder.
- **Excels:** no detail looks unfinished. **Fails:** two buttons for one role that differ, a state
  left undrawn, a border off by a pixel.

## 7. Clarity

- At a glance: what the screen is for, what is selected, what can be done, what state it is in.
- Every label, icon and value unambiguous, each value with its unit; nothing decorative competes
  with the meaning.
- Not here: the wording itself (UX · Language).
- **Excels:** someone looking over a shoulder can say what is going on. **Fails:** a value with no
  unit, a control whose purpose takes a click to learn.

## 8. Art direction

- ONI is a digital tool and owns it, while its look nods to the analogue medium it serves: keys, levers and
  dials, metal, film canisters, playful like a film camera.
- Large surfaces stay flat; texture belongs to small pieces only, such as labels and medals.
- One analogue vocabulary, held with the same care everywhere; the nods never compete with the photo, never
  tint it and never blur what a control says.
- **Excels:** handling it feels like holding a well-made film camera. **Fails:** generic flat software, or
  kitsch that clutters the screen or biases the photo.

## 9. Overall

- The screen as a whole: one coherent visual language, a professional tool made for photographers,
  a screen a demanding designer would ship.
- A judgement of the whole, not the mean of the categories above.
- **Excels:** it stands beside the best photo tools without apology. **Fails:** it reads as a
  prototype.
