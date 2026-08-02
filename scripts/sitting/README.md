# The screen sitting, as an instrument

`run.sh` brings up `loki-text` on a virtual X display with a software Vulkan
adapter, drives it with synthetic key and pointer events, and captures the
framebuffer at each step.

It exists because Spec 08 Phase 4 could not close on unit tests. Two of its
acceptance rows — "Home fully keyboard-navigable" and "Open discoverable
touch-only and with a mouse" — are claims about what a person sees, and the
suite had 2 883 passing tests while the app had **no focus indicator at all** and
**no way to press a button from the keyboard**. Neither defect is subtle. Both
were invisible to every test that does not paint.

## Running it

```
cargo build -p loki-text --bin loki-text-desktop
scripts/sitting/run.sh smoke      # Home renders, fine pointer
scripts/sitting/run.sh coarse     # the visible-label branch
scripts/sitting/run.sh keyboard   # Tab walk, one shot per stop
TABS=12 scripts/sitting/run.sh menu   # open a row menu, walk it, Escape
scripts/sitting/run.sh editor         # open a document, click, type
ZX=1176 ZY=788 scripts/sitting/run.sh zoom   # the status-bar zoom control
scripts/sitting/run.sh calibrate      # Actual Size -> measure-your-screen dialog
CX=36 CY=740 scripts/sitting/run.sh picker   # the colour picker's SV square
```

Shots land in `target/sitting/`. `SETTLE` (default 10s) is how long to wait
after the window maps — lavapipe is slow, and the app's CSS lands on the second
poll, so a short settle photographs a black screen.

Host packages: `xvfb`, `x11-apps` (xwd), `imagemagick` (convert/compare),
`xdotool`, `mesa-vulkan-drivers` (lavapipe), `libxkbcommon-x11-0`.

## Reading the results

`compare -metric AE a.png b.png null:` gives a changed-pixel count, which is the
cheap assertion: *did this keypress do anything at all*. It is what caught every
defect below. A zero where you expected a change is the finding; a montage of
the crops is how you work out which change it was.

## Two ways this instrument lied before it worked

Both are worth knowing, because both produced a confident, wrong "the app is
broken" reading (evidence rule 3 — an instrument whose own setup silences its
subject):

1. **No window manager on Xvfb, so nothing assigns the X input focus.**
   `xdotool key` went to no client at all, and eight Tab presses changed
   nothing. The app was fine. `run.sh` now sets input focus explicitly after
   the window maps.

2. **A stale click coordinate.** The `zoom` scenario clicks the status-bar
   readout by position. When the control's width changed, the click landed in
   the gap beside it and the run read as "the menu does not open" — the app was
   fine. Coordinates are parameters (`ZX`/`ZY`) for this reason; re-measure from
   a screenshot after any layout change rather than trusting the default.

3. **A probe ordered so it could not lose.** Testing whether `:focus-visible`
   is supported, the first attempt put `:focus` *after* it — equal specificity,
   so `:focus` won whether or not `:focus-visible` matched, and the result
   looked like "unsupported" without being evidence of it. Re-run with the
   order flipped, `:focus` still won: genuinely unsupported.

## What the sittings have found

**r79 — four defects, none reachable by any unit test:**

| finding | layer |
| --- | --- |
| no `:focus` styling anywhere — focus moved invisibly (WCAG 2.4.7) | `appthere_ui::focus_ring` |
| Enter/Space never activated a focused control (WCAG 2.1.1) | `patches/blitz-dom` keyboard |
| `tabindex="-1"` was not focusable, so `autofocus` never applied to an overlay | `patches/blitz-dom` element |
| click-focus ran *after* the click handler, undoing that handler's `autofocus` | `patches/blitz-dom` mouse |

**r83 — one:** a colour dragged in the saturation/value square filled the
preview swatch and left the `#` field blank, so the reader had nowhere to read
or copy what they had picked. The same run also *confirmed* what the design
rested on — `linear-gradient` really does paint in this engine — which is the
cheaper thing a sitting does: settling a premise before the code built on it
grows.

**r82 — three, all in one dialog:** the instructions read "85.5999984741211 mm"
(`f64::from(85.6f32)` — correct arithmetic, unreadable prose); the text field was
not focused, so typing went nowhere; and calibrating did not apply the Actual
Size the reader had asked for, so the page did not move after they fetched a
ruler. Every unit test passed for all three.

**r80 — one:** the new zoom control's buttons carried `min-height: TOUCH_MIN`
(44 px) in a 24 px status bar, so they overflowed **upward and painted over the
ribbon**. Every unit test passed; the control was correct and in the wrong place.
It now uses the bar's existing `height: 100%` convention, with the WCAG bound
that forces stated on the component.
