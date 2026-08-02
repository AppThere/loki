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
scripts/sitting/run.sh anchor         # zoom holds the middle of the page still
ZX=1176 ZY=788 scripts/sitting/run.sh typed  # the typed zoom field
scripts/sitting/run.sh wheelzoom      # Ctrl+wheel zooms, plain wheel scrolls
scripts/sitting/run.sh highlight      # highlight: typed, clicked and custom
```

Shots land in `target/sitting/`. `SETTLE` (default 10s) is how long to wait
after the window maps — lavapipe is slow, and the app's CSS lands on the second
poll, so a short settle photographs a black screen.

Host packages: `xvfb`, `x11-apps` (xwd), `imagemagick` (convert/compare),
`xdotool`, `mesa-vulkan-drivers` (lavapipe), `libxkbcommon-x11-0`.

Some checks are measurements rather than looks. The `anchor` scenario is one:
a landmark 210 px above the viewport centre must sit 210×z px above it after
zooming by z. Reading that off two screenshots is the check — an eyeball
"it still looks about right" would pass for a formula that is merely close.

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

**One wheel notch is two events on X11**, and it is winit's, not ours.
`xinput2_button_input` routes both `XI_ButtonPress` and `XI_ButtonRelease` to
the same arm and the `4..=7` case ignores the press/release state, so every
notch emits two `MouseWheel` events (winit 0.30.13,
`platform_impl/linux/x11/event_processor.rs`). One `xdotool click 4` therefore
moves the zoom by 1.1², not 1.1. It is upstream behaviour on real X11 hardware
too, and it affects the existing scroll path identically — do not "correct" for
it in app code, which would halve the step everywhere else.

## What the sittings have found

**r90 — the Phase 5 close audit, and a feature that was inert.** The mechanical
"decisions with no consumer" count — the same check that stopped the Phase 4
close — found `DisplayKey::from_output` called by nothing but its own test. So
T5.5's per-display calibration was per-*machine*: every read and every write used
`DisplayKey::unidentified`, and the module's own opening paragraph described the
docked-laptop case it did not implement.

The data was one field away. The X11 probe already read the output name and crtc
geometry in the same reply and returned neither — and it only returned *anything*
when the physical size was believable, which is backwards: a display whose
`mm_width` is zero is exactly the one a reader calibrates by hand. Xvfb is such a
display (`name="screen"`, 1280x900, `mm=0x0`), which is how that got measured
rather than reasoned about.

Verified both directions, because only the second is discriminating: the same
display restores its calibration, and a 1600x1000 one restores nothing.

**r89 — one, and it made a whole control dead.** The colour picker's custom
saturation/value square looked entirely correct: drag a colour, the preview
swatch fills, the hex field shows it, the Apply button is enabled. Pressing Apply
did nothing. Its `onclick` resolved from the **typed fields**, which are empty
while the square is the source — that is `custom_source`'s "last edited wins"
rule working as designed — while its `disabled` flag gated on the *preview*. The
enabled state and the action were answers to different questions, so the button
advertised itself as live and was not.

Nothing headless could see it: `resolve(mode, fields)` was correct,
`displayed_hsv` was correct, and the defect was that the button called the first
where it meant the second. It had been shipped one sitting earlier (r83) — that
sitting fixed the *display* half of the same split and stopped one layer short.

**r88 — three, and the third was in code four phases old.** Ctrl+wheel zoom was
built, unit-tested, and wrong in two ways no test could see. `element_
coordinates()` is relative to the event's *target* — a text run several levels
below the element whose handler runs — so the anchor was out by 135 px, the
distance the page's top sat above the window. And two wheel events 0.2 ms apart
both read `scroll_top = 200.0`, because the metrics signal mirrors the DOM's
`onscroll` and lags the command that caused it: the zoom advanced twice while the
scroll moved once.

Both were found by *printing what the code read*, not by inferring it from the
picture — the picture said "the anchor drifts 16 px", which is consistent with
half a dozen causes and identifies none.

Fixing them left a 5 px residual, which turned out not to be a wheel problem at
all: `anchored_scroll`'s model was `a = d·z − s`, and a scroll container's
padding does not scale, so it is `a = p + d·z − s`. The model predicted −5.04 px,
the screen showed −5, and the fix took it to 0. That affected **every** zoom
taken since T5.6 landed, from the buttons and presets as much as the wheel — a
defect the wheel only made visible.

**r79 — four defects, none reachable by any unit test:**

| finding | layer |
| --- | --- |
| no `:focus` styling anywhere — focus moved invisibly (WCAG 2.4.7) | `appthere_ui::focus_ring` |
| Enter/Space never activated a focused control (WCAG 2.1.1) | `patches/blitz-dom` keyboard |
| `tabindex="-1"` was not focusable, so `autofocus` never applied to an overlay | `patches/blitz-dom` element |
| click-focus ran *after* the click handler, undoing that handler's `autofocus` | `patches/blitz-dom` mouse |

**r86 — two, and a probe that stopped a guess.** A digit typed into the zoom
menu's new field showed only the last character. Rather than fixing the
rendering, one probe printed the *signal* beside the field — it held the last
character too, so the bug was never in rendering: an `<input>` inside popover
content has its text re-applied on every render, and `oninput` reports only the
newest keystroke. And Backspace turned out not to be in the popover key
vocabulary at all, so a typo could only be undone by starting over.

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
