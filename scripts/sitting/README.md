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

2. **A probe ordered so it could not lose.** Testing whether `:focus-visible`
   is supported, the first attempt put `:focus` *after* it — equal specificity,
   so `:focus` won whether or not `:focus-visible` matched, and the result
   looked like "unsupported" without being evidence of it. Re-run with the
   order flipped, `:focus` still won: genuinely unsupported.

## What the first sitting found (r79)

Four defects, none of which any unit test could have reached:

| finding | layer |
| --- | --- |
| no `:focus` styling anywhere — focus moved invisibly (WCAG 2.4.7) | `appthere_ui::focus_ring` |
| Enter/Space never activated a focused control (WCAG 2.1.1) | `patches/blitz-dom` keyboard |
| `tabindex="-1"` was not focusable, so `autofocus` never applied to an overlay | `patches/blitz-dom` element |
| click-focus ran *after* the click handler, undoing that handler's `autofocus` | `patches/blitz-dom` mouse |
