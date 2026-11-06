# UX bench

The UX bench rates ONI in use: a task from its first step to its last, the counts and timings along
the way, the behaviour between two screens, and the words. How one still screen looks is the UI
bench's ([ui-bench.md](ui-bench.md)); where a criterion borders on one of its categories it names
it, so nothing is rated twice.

Each criterion says what it judges, what to look at, and the two ends of its scale.

## 1. Task fit

Each key task takes the shortest path its decisions allow: import a roll, prepare its window,
convert, adjust, carry a setting across the roll, de-dust, crop, compare, export, archive.

- Steps, decisions and mode changes per task, and the time to the first right image.
- The concepts are a photographer's (roll, frame, negative or positive, the roll's window), never
  the code's (stages, caches, sidecars).
- The structure follows the work: the sidebar's sections, the panel's order and grouping of settings and
  the steps of each flow match the sequence a photographer decides in, whatever the app did before.
- **Excels:** no step could go without losing a decision. **Fails:** a detour through an unrelated
  screen, or a concept learnt for the app's sake.

## 2. Obvious without a tutorial

A newcomer with a first scanned roll finds the next action alone. This is the product's stated
goal, so it is a knock-out criterion.

- Signifiers (what clicks, drags or edits), labels, the first launch, every empty state, shortcuts
  shown where their action lives.
- **Excels:** a first converted frame reached without help. **Fails:** an action reached only
  through a hidden gesture, a modifier or a menu nothing points to.

## 3. Expert efficiency

What an action repeated on every frame costs someone who converts ten rolls a week.

- Keyboard paths, actions on a selection, settings copied and pasted, slider precision (fine step,
  typed value, reset), travel between the image and the controls.
- **Excels:** an action on 36 frames costs one key per frame, or one for the roll. **Fails:** the
  same clicks again on every frame, or an exact value reached only by dragging.

## 4. Feel

How fast and how steadily the app answers, measured rather than estimated.

- Input-to-photon latency, frame-time consistency (no spike, no deep buffering), a drag that
  follows the hand, the time to launch and to open a frame or a roll.
- The anchors are the performance targets of `CLAUDE.md`: while a parameter slides, the image
  follows each render's landing, at most one a 120 Hz frame; the rest of the interface runs at
  120 fps at least; low latency beats a high but loose frame rate.
- **Excels:** every gesture lands within a frame, every time. **Fails:** a hitch, a stall, an image
  still moving once the hand rests.

## 5. Trust

The person always knows what the app is doing, and nothing happens behind their back.

- The state shows at every moment: loading, modified, saved, the active mode and its way out.
- Every edit undoes, nothing irreversible goes unannounced; the export writes what the screen shows;
  originals are never touched.
- Nothing is hidden: every automatic step and every value of the conversion stays visible and
  reachable, even while an automatic mode sets it.
- A destructive action stands apart from the harmless ones around it, never on their plate, so the hand
  cannot take one for the other.
- **Excels:** any step interrupted loses nothing and surprises no one. **Fails:** a silent wait, a
  mode with no visible exit, an edit undo skips, an export unlike the screen.

## 6. Hierarchy in use

The image stays the subject while one works, and the interface steps back.

- At each step the eye lands first on what the step needs; while a hand drags, nothing else moves
  or calls for attention.
- Not here: the rank of elements on one still screen (UI · Hierarchy), the surround's neutrality
  (UI · Color and contrast).
- **Excels:** the interface fades while one works on the image. **Fails:** a control, a badge or a
  movement competing with the photo.

## 7. Consistency of behaviour

The same gesture does the same thing everywhere, and the platform's conventions hold.

- Double-click, right-click, drag, scroll, selection with ⇧ and ⌘, Escape and Return, the reach of
  undo.
- macOS conventions on macOS (menus, ⌘ shortcuts, window behaviour), Windows' on Windows.
- Drag belongs to parameters and the image; menus, lists and panels scroll.
- Not here: whether components look alike (UI · Consistency and polish).
- **Excels:** a gesture learnt once works everywhere. **Fails:** a double-click that resets here and
  opens there.

## 8. Micro-interactions

The mechanics of each control under the hand.

- Hover and press response, focus, cursors, hit target sizes, drag threshold, snapping; the
  sliders' and the knob's mechanics (fine step, double-click reset, wheel); the gallery's selection
  kept in step with the sidebar.
- **Excels:** every control answers the instant it is touched, as the hand expects. **Fails:** a
  target under 24 × 24 pt (WCAG 2.2), a click read as a drag, a selection that drifts.

## 9. Robustness

The experience at the edges.

- A roll of 500 frames, originals offline, an unreadable RAW, a slow or read-only disk, a tiny or a
  huge window, a long name, French and Spanish texts longer than English.
- **Excels:** each edge says what happened and what to do, and the rest keeps working. **Fails:** a
  crash, a hang, a blank screen, a clipped text.

## 10. Language

The words a photographer reads.

- Terms right and constant for a photographer (density, film base, window), buttons that are verbs,
  units shown, errors that say what to do next, the same meaning in English, French and Spanish.
- Not here: whether a screen's meaning reads at a glance (UI · Clarity).
- **Excels:** every screen reads without meeting a word of the code. **Fails:** a code word on
  screen, two names for one thing, an error with no way out.

## 11. Accessibility

The app used by everyone.

- Keyboard-only use with a visible focus in a sensible order, VoiceOver names and roles, reduced
  motion respected, no state told by colour alone.
- Not here: contrast ratios (UI · Color and contrast).
- **Excels:** every task done without a pointer, every control named. **Fails:** a control the
  keyboard never reaches.

## 12. Craft in use

The finish from one step to the next.

- Alignment and spacing that hold through every state, one visual language from screen to screen;
  no flicker, no layout jump, no half-drawn state, no text popping in; transitions that show where
  things came from and went.
- Not here: the polish of one still screen (UI · Spacing and density, Consistency and polish).
- **Excels:** nothing ever jumps. **Fails:** a panel that jumps when its content lands, a flash of
  an empty state.

## 13. Play

Handling the app is a pleasure of its own, like a camera's mechanics in the hand.

- Press and release, travel, springs, detents and clicks on keys, sliders and levers; small rewards for
  touching things, an invitation to fiddle.
- The play never slows the work nor hides a state.
- **Excels:** the controls are fun to touch and make the work quicker. **Fails:** dead controls with no
  feedback, or gimmicks that delay the hand.
