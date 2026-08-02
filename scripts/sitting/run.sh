#!/usr/bin/env bash
# Phase 4 screen sitting harness (Spec 08 r79).
#
# Brings up loki-text on a virtual X display with a software Vulkan adapter,
# drives it with synthetic key/pointer events, and captures the framebuffer at
# each step. This is the instrument the phase's remaining acceptance rows were
# waiting on: "does autofocus land" and "is the focus ring visible" are questions
# no headless unit test can reach.
#
# Usage: .sitting/run.sh <scenario>
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SHOT_DIR="${SHOT_DIR:-$ROOT/target/sitting}"
BIN="${BIN:-$ROOT/target/debug/loki-text-desktop}"
DISPLAY_NUM=99
export DISPLAY=":${DISPLAY_NUM}"
# lavapipe: software Vulkan, so wgpu gets a real adapter with no GPU present.
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
export WGPU_BACKEND=vulkan
export LIBGL_ALWAYS_SOFTWARE=1
export RUST_LOG="${RUST_LOG:-warn,appthere_ui=debug,loki_text=debug}"

mkdir -p "$SHOT_DIR"

shot() { # shot <name>
  xwd -root -silent 2>/dev/null | convert xwd:- -depth 8 "$SHOT_DIR/$1.png" 2>/dev/null
  echo "  [shot] $1"
}

start_x() {
  pkill -f "Xvfb :${DISPLAY_NUM}" 2>/dev/null
  rm -f "/tmp/.X${DISPLAY_NUM}-lock"
  Xvfb ":${DISPLAY_NUM}" -screen 0 1280x900x24 -nolisten tcp >/dev/null 2>&1 &
  XVFB_PID=$!
  for _ in $(seq 1 40); do xdpyinfo >/dev/null 2>&1 && return 0; sleep 0.25; done
  echo "Xvfb did not come up"; return 1
}

start_app() { # start_app [env assignments...]
  "$@" "$BIN" > "$SHOT_DIR/app.log" 2>&1 &
  APP_PID=$!
  # Wait for a mapped, non-root window rather than sleeping a fixed time.
  for _ in $(seq 1 120); do
    WIN=$(xdotool search --onlyvisible --name "." 2>/dev/null | tail -1)
    if [ -n "${WIN:-}" ]; then
      # No window manager on Xvfb, so nothing assigns the X input focus and
      # `xdotool key` would go to no client at all. Set it explicitly — this is
      # the harness's own precondition, not the app's behaviour.
      xdotool windowactivate "$WIN" 2>/dev/null
      xdotool windowfocus --sync "$WIN" 2>/dev/null
      xdotool windowraise "$WIN" 2>/dev/null
      sleep "${SETTLE:-10}"   # first frame: lavapipe is slow, and CSS lands on the second poll
      xdotool search --onlyvisible --name "." getwindowname %@ 2>/dev/null | sed "s/^/  [win] /"
      return 0
    fi
    kill -0 "$APP_PID" 2>/dev/null || { echo "app exited early"; tail -20 "$SHOT_DIR/app.log"; return 1; }
    sleep 0.5
  done
  echo "no window appeared"; tail -20 "$SHOT_DIR/app.log"; return 1
}

stop() {
  kill "$APP_PID" 2>/dev/null; wait "$APP_PID" 2>/dev/null
  kill "$XVFB_PID" 2>/dev/null
}
trap stop EXIT

key() { xdotool key --clearmodifiers "$1"; sleep 0.6; }

# ---------------------------------------------------------------------------
case "${1:-smoke}" in

smoke)
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE=pointer=fine || exit 1
  shot 01-home-fine
  echo "  window: $(xdotool getactivewindow getwindowgeometry 2>/dev/null | tr '\n' ' ')"
  ;;

coarse)
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE=pointer=coarse || exit 1
  shot 01-home-coarse
  ;;

keyboard)
  # The Phase 4 sitting proper: open the row menu with the keyboard, walk it,
  # Escape out, and check where focus went.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE=pointer=fine || exit 1
  shot 10-home
  echo "== Tab to the first focusable =="
  for i in $(seq 1 "${TABS:-8}"); do key Tab; shot "11-tab-$i"; done
  ;;

menu)
  # Tab to a row's ⋮ (TABS stops), open it, walk it, Escape, and see where
  # focus lands. This is the row Phase 4 could not close on.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE=pointer=fine || exit 1
  shot 20-home
  for i in $(seq 1 "${TABS:-11}"); do key Tab; done
  shot "21-on-trigger"
  key Return;        shot 22-menu-open
  key Down;          shot 23-down-1
  key Down;          shot 24-down-2
  key Escape;        shot 25-after-escape
  ;;

editor)
  # Regression cover for the mousedown focus move (r79): open a document, click
  # into the canvas, type, and check the glyphs land. Nothing here is about
  # overlays — it is the path the focus change could most easily have broken.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE=pointer=fine || exit 1
  shot 30-home
  key Tab; key Tab; key Tab; key Tab; key Tab; key Tab; key Tab
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot 31-opened
  xdotool mousemove 640 500 click 1; sleep 1
  shot 32-clicked
  xdotool type --delay 120 "ZZQQ"; sleep 2
  shot 33-typed
  ;;

zoom)
  # T5.4: the zoom control in the status bar — step out, open the preset menu,
  # walk it with the keyboard, and choose a preset.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  shot 40-home
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot 41-opened
  # The status bar sits at the bottom; click the readout to raise the menu.
  # The readout's position depends on what else the status bar holds, so it
  # is a parameter rather than a constant — a stale coordinate silently
  # clicks the gap beside the control and the run reads as "the menu does
  # not open" (the harness's own lie, again).
  xdotool mousemove "${ZX:-1176}" "${ZY:-788}" click 1; sleep 2
  shot 42-zoom-menu
  key Down;  shot 43-zoom-down
  key Down;  shot 44-zoom-down2
  key Return; sleep 2
  shot 45-zoom-picked
  # End: the last row, which is Actual Size when the density is known.
  xdotool mousemove "${ZX:-1176}" "${ZY:-788}" click 1; sleep 2
  key End; shot 46-zoom-end
  key Return; sleep 2
  shot 47-zoom-actual
  ;;

calibrate)
  # T5.5: Actual Size on an uncalibrated display must OFFER calibration, not
  # hide itself. Opens the zoom menu, picks the last row, and measures.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  shot 50-home
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot 51-opened
  xdotool mousemove "${ZX:-1176}" "${ZY:-788}" click 1; sleep 2
  shot 52-menu
  key End;    shot 53-on-actual
  key Return; sleep 2
  shot 54-dialog
  xdotool type --delay 120 "${MEASURED:-80}"; sleep 1
  shot 55-typed
  # Apply: the density becomes 96 * 85.6 / measured, and Actual Size follows.
  xdotool mousemove 784 517 click 1; sleep 3
  shot 56-applied
  ;;

picker)
  # T5.2: the saturation/value square and hue strip. Format tab -> a colour
  # trigger -> the panel docks above the ribbon.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  shot 60-home
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot 61-opened
  # Format tab, then the font-colour trigger.
  xdotool mousemove "${FX:-98}" "${FY:-696}" click 1; sleep 2
  shot 62-format
  xdotool mousemove "${CX:-640}" "${CY:-740}" click 1; sleep 2
  shot 63-picker
  # Drag inside the saturation/value square: press at the middle, move down-left,
  # release. The preview and the handle must follow.
  xdotool mousemove "${SX:-511}" "${SY:-491}" mousedown 1; sleep 1
  shot 64-sv-press
  xdotool mousemove "${SX2:-470}" "${SY2:-530}"; sleep 1
  xdotool mouseup 1; sleep 1
  shot 65-sv-drag
  # And the hue strip.
  xdotool mousemove "${HX:-643}" "${HY:-520}" click 1; sleep 1
  shot 66-hue
  ;;

anchor)
  # T5.6: zooming from the control must hold the middle of the page still.
  # Scroll into the document first, so there is something to hold.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  # Put the pointer over the canvas and wheel down a few notches.
  xdotool mousemove 640 400
  for _ in 1 2 3 4 5 6; do xdotool click 5; done
  sleep 2
  shot 70-scrolled
  # Zoom in one preset from the control.
  xdotool mousemove "${PLUSX:-1225}" "${PLUSY:-788}" click 1; sleep 3
  shot 71-zoomed
  ;;

typed)
  # T5.4: a digit typed at the open zoom menu starts the typed field; Enter
  # applies it. This is the keyboard route to a control Tab cannot reach.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  xdotool mousemove "${ZX:-1176}" "${ZY:-788}" click 1; sleep 2
  shot 80-menu
  key 3; sleep 1
  shot 81-field
  xdotool type --delay 120 "40"; sleep 1
  shot 82-typed
  # Backspace must reach the field — without it a typo can only be undone by
  # starting over (Spec 08 T5.4, r86).
  key BackSpace; sleep 1
  shot 84-erased
  key 0; sleep 1
  key Return; sleep 2
  shot 83-applied
  ;;

wheelzoom)
  # T5.6: Ctrl+wheel zooms about the pointer, and a plain wheel still scrolls.
  #
  # The whole point of the sitting is that no unit test can see this: the
  # question is whether a wheel gesture *reaches* Dioxus at all, which depends
  # on three patched crates cooperating, and every one of them compiles fine
  # while dropping the event.
  #
  # Ordered so the control comes first. A run that only showed Ctrl+wheel
  # changing the zoom would not distinguish "Ctrl+wheel zooms" from "any wheel
  # zooms" — and the second is a defect that would make the document
  # unscrollable.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  xdotool mousemove "${WX:-640}" "${WY:-400}"; sleep 1
  shot 90-opened
  # 1. Plain wheel down: the page must scroll, the zoom must not move.
  for _ in 1 2 3 4 5 6; do xdotool click 5; done
  sleep 2
  shot 91-plain-scrolled
  # 2. Ctrl+wheel up: the zoom must rise.
  xdotool keydown ctrl
  for _ in 1 2 3 4; do xdotool click 4; sleep 0.3; done
  xdotool keyup ctrl
  sleep 3
  shot 92-ctrl-in
  # 3. Ctrl+wheel down: and fall again.
  xdotool keydown ctrl
  for _ in 1 2 3 4 5 6 7 8; do xdotool click 5; sleep 0.3; done
  xdotool keyup ctrl
  sleep 3
  shot 93-ctrl-out
  # The status-bar readout is the instrument that reports the zoom as a number;
  # crop it so the comparison is about the zoom and not about the page having
  # also moved.
  for s in 90-opened 91-plain-scrolled 92-ctrl-in 93-ctrl-out; do
    convert "$SHOT_DIR/$s.png" -crop "${ZCROP:-120x24+1120+776}" +repage \
      "$SHOT_DIR/$s-readout.png" 2>/dev/null
  done
  echo "== readout diffs (0 = the zoom did not change) =="
  echo "  plain wheel:  $(compare -metric AE "$SHOT_DIR/90-opened-readout.png" \
    "$SHOT_DIR/91-plain-scrolled-readout.png" null: 2>&1)"
  echo "  ctrl wheel in:  $(compare -metric AE "$SHOT_DIR/91-plain-scrolled-readout.png" \
    "$SHOT_DIR/92-ctrl-in-readout.png" null: 2>&1)"
  echo "  ctrl wheel out: $(compare -metric AE "$SHOT_DIR/92-ctrl-in-readout.png" \
    "$SHOT_DIR/93-ctrl-out-readout.png" null: 2>&1)"
  echo "== canvas diffs (0 = the page did not move) =="
  echo "  plain wheel:  $(compare -metric AE "$SHOT_DIR/90-opened.png" \
    "$SHOT_DIR/91-plain-scrolled.png" null: 2>&1)"
  ;;

highlight)
  # T5.3: a highlight may be any colour. Both routes, in one run.
  #
  # The named route and the custom route store different model properties
  # (`highlight_color` vs `background_color`) and reach the page by different
  # paths, so exercising only one would leave half the feature unobserved — and
  # the half nobody checks is the one that ships dead. That is not a guess: the
  # custom route's Apply button read the *typed* fields while its enabled state
  # read the square, so it looked live and did nothing (r89).
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot n0-plain
  # Select the heading, then Format -> Highlight.
  xdotool mousemove "${TX:-375}" "${TY:-172}" click 1; sleep 1
  xdotool key --clearmodifiers shift+End; sleep 1
  xdotool mousemove "${FX:-98}" "${FY:-696}" click 1; sleep 1
  xdotool mousemove "${HX:-108}" "${HY:-738}" click 1; sleep 2
  shot n1-panel
  # 1. A named swatch (Yellow) — stored as `HighlightColor::Yellow`.
  xdotool mousemove "${YX:-43}" "${YY:-473}" click 1; sleep 3
  shot n2-named
  # 2. A custom colour: drag the saturation/value square, then Apply.
  #    Re-open the picker; the panel now carries a Recent group, which shifts
  #    the custom column right — so the Apply coordinate is NOT the same as it
  #    would be on a first open. Read it off n3 rather than reusing one.
  xdotool mousemove "${TX:-375}" "${TY:-172}" click 1; sleep 1
  xdotool key --clearmodifiers shift+End; sleep 1
  xdotool mousemove "${HX:-108}" "${HY:-738}" click 1; sleep 2
  xdotool mousemove "${SX:-616}" "${SY:-450}" mousedown 1; sleep 1
  xdotool mouseup 1; sleep 1
  shot n3-picked
  xdotool mousemove "${AX:-549}" "${AY:-655}" click 1; sleep 3
  shot n4-custom
  echo "== page diffs (0 = nothing painted) =="
  echo "  named:  $(compare -metric AE "$SHOT_DIR/n0-plain.png" "$SHOT_DIR/n2-named.png" null: 2>&1)"
  echo "  custom: $(compare -metric AE "$SHOT_DIR/n2-named.png" "$SHOT_DIR/n4-custom.png" null: 2>&1)"
  ;;

esac
echo "DONE: $1"
