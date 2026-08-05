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
  # SCREEN is a parameter because some scenarios are *about* a narrow window —
  # the ribbon overflow menu only exists when the strip does not fit.
  Xvfb ":${DISPLAY_NUM}" -screen 0 "${SCREEN:-1280x900}x24" -nolisten tcp >/dev/null 2>&1 &
  XVFB_PID=$!
  for _ in $(seq 1 40); do xdpyinfo >/dev/null 2>&1 && return 0; sleep 0.25; done
  echo "Xvfb did not come up"; return 1
}

# loki-text persists its window geometry, so one WINSIZE run leaves every later
# run at that size — a leak between runs that are supposed to be independent,
# and one no scenario can see: the app is behaving correctly. It cost two
# scenarios' results before it was spotted, and both looked like regressions in
# the code under test. Cleared here so each run starts from the app's default.
#
# **The recent-documents list is the same leak** and cost the same kind of wrong
# answer (r94): a path left by an earlier run put a row on the Home screen that
# every later `menu` run then Tabbed onto, and opening it produced a router error
# page instead of the ⋮ menu the scenario is about. It reads as "the menu is
# broken". Every file the app persists belongs in here, not just the one that
# has bitten so far.
#
# **This changes what a `TABS` count lands on**, and that is the intended
# direction: with the recent list cleared every run, the Home tab order is the
# same every run. It was previously a function of whatever earlier sittings had
# opened, so two runs of one scenario could focus different controls with nothing
# in the output to say why. Scenario `TABS` values are calibrated against the
# empty list.
reset_window_state() {
  local data="${XDG_DATA_HOME:-$HOME/.local/share}/AppThere/Loki"
  rm -f "$data/window.json" "$data/recent.json" 2>/dev/null
}

start_app() { # start_app [env assignments...]
  reset_window_state
  "$@" "$BIN" > "$SHOT_DIR/app.log" 2>&1 &
  APP_PID=$!
  # Wait for a mapped, non-root window rather than sleeping a fixed time.
  for _ in $(seq 1 120); do
    WIN=$(xdotool search --onlyvisible --name "." 2>/dev/null | tail -1)
    if [ -n "${WIN:-}" ]; then
      # No window manager on Xvfb, so nothing assigns the X input focus and
      # `xdotool key` would go to no client at all. Set it explicitly — this is
      # the harness's own precondition, not the app's behaviour.
      # NO_ACTIVATE: a pointer-only scenario does not need X input focus, and
      # on this Xvfb the activate call takes the server down outright (measured
      # while building the T7.0 probe: the app logs "X connection ... broken"
      # and every later xdotool call fails against a dead display). Scenarios
      # that send keys still activate; they have no choice.
      if [ -z "${NO_ACTIVATE:-}" ]; then
        xdotool windowactivate "$WIN" 2>/dev/null
        xdotool windowfocus --sync "$WIN" 2>/dev/null
        xdotool windowraise "$WIN" 2>/dev/null
      fi
      # **A smaller screen does not make a smaller window.** There is no window
      # manager on Xvfb, so the app keeps whatever size it asked for and simply
      # extends past the screen edge — which photographs as a clipped ribbon and
      # reads as "the strip overflows", while the app still measures its
      # original width. Measured: `SCREEN=560x900` with the collapse cascade
      # still reading 1268. A scenario that is *about* a narrow window sets
      # WINSIZE; everything else keeps the app's own default geometry, which is
      # what every scenario's coordinates were calibrated against.
      if [ -n "${WINSIZE:-}" ]; then
        xdotool windowsize --sync "$WIN" "${WINSIZE%x*}" "${WINSIZE#*x}" 2>/dev/null
        sleep 1
      fi
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

# Click, and **complain if nothing happened**.
#
# The harness's most expensive lie, four times over (r80, r86, r90, r91): a
# coordinate goes stale, the click lands on the gap beside a control, and the run
# reads as "the feature does not work". Every one of those cost a wrong
# conclusion, and one of them was reported before it was caught.
#
# A click that changes no pixel anywhere did not hit anything interactive. That
# is not a perfect test — a control whose action is invisible would trip it — but
# it is the difference between a silent wrong answer and a loud question.
click_at() { # click_at <x> <y> <what>
  local x="$1" y="$2" what="${3:-click}"
  # **Move first, then snapshot.** Blitz tints a hovered control from
  # `onmouseenter`, so a snapshot taken before the pointer moved differs from the
  # one after *whether or not the click landed on anything* — which masked a
  # click that missed its trigger entirely on the first version of this helper.
  # Separating the move from the click makes the comparison about the click.
  xdotool mousemove "$x" "$y"
  sleep "${HOVER_SETTLE:-1}"
  xwd -root -silent 2>/dev/null | convert xwd:- -depth 8 "$SHOT_DIR/.pre-click.png" 2>/dev/null
  xdotool click 1
  sleep "${CLICK_SETTLE:-2}"
  xwd -root -silent 2>/dev/null | convert xwd:- -depth 8 "$SHOT_DIR/.post-click.png" 2>/dev/null
  local d
  d=$(compare -metric AE "$SHOT_DIR/.pre-click.png" "$SHOT_DIR/.post-click.png" null: 2>&1)
  if [ "$d" = "0" ] && [ -z "${IDEMPOTENT:-}" ]; then
    echo "  [!] click on '$what' at $x,$y changed NOTHING — stale coordinate?"
  fi
}

# Crop helper: echoes the path so a `compare` can be written on one line.
crop() { # crop <shot> <geometry>
  convert "$SHOT_DIR/$1.png" -crop "$2" +repage "$SHOT_DIR/$1-crop-${2//[^0-9]/_}.png"
  echo "$SHOT_DIR/$1-crop-${2//[^0-9]/_}.png"
}


# Wheel `n` notches in `dir` (4 = up, 5 = down) at the current pointer position.
#
# `xdotool click 4/5` is how X carries a vertical wheel notch (6/7 horizontal);
# winit turns it into `MouseScrollDelta::LineDelta` and `blitz-shell` converts to
# CSS pixels.
#
# **One notch moves ~40 CSS px here — measured, not derived.** Reading the code
# gives 20 (`LineDelta * 20.0`), but three notches over the P1 probe's 40 px rows
# advanced it by three whole rows, so whatever X and winit agree a notch is, it
# is not one line. The notch counts below are calibrated against the measurement;
# do not re-derive them from the multiplier.
wheel() { # wheel <dir 4|5> <count>
  local dir="$1" n="$2"
  for _ in $(seq 1 "$n"); do xdotool click "$dir"; sleep 0.12; done
  sleep "${WHEEL_SETTLE:-2}"
}

# `compare -metric AE` between the same crop of two shots, echoed as a number.
band() { # band <geometry> <shotA> <shotB>
  compare -metric AE "$(crop "$2" "$1")" "$(crop "$3" "$1")" null: 2>&1
}

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
  CLICK_SETTLE=1 click_at "640" "500" "canvas"
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
  CLICK_SETTLE=2 click_at "${ZX:-1176}" "${ZY:-788}" "zoom readout"
  shot 42-zoom-menu
  key Down;  shot 43-zoom-down
  key Down;  shot 44-zoom-down2
  key Return; sleep 2
  shot 45-zoom-picked
  # End: the last row, which is Actual Size when the density is known.
  CLICK_SETTLE=2 click_at "${ZX:-1176}" "${ZY:-788}" "zoom readout"
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
  CLICK_SETTLE=2 click_at "${ZX:-1176}" "${ZY:-788}" "zoom readout"
  shot 52-menu
  key End;    shot 53-on-actual
  key Return; sleep 2
  shot 54-dialog
  xdotool type --delay 120 "${MEASURED:-80}"; sleep 1
  shot 55-typed
  # Apply: the density becomes 96 * 85.6 / measured, and Actual Size follows.
  CLICK_SETTLE=3 click_at "784" "517" "calibrate Apply"
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
  CLICK_SETTLE=2 click_at "${FX:-98}" "${FY:-696}" "Format tab"
  shot 62-format
  CLICK_SETTLE=2 click_at "${CX:-36}" "${CY:-738}" "font-colour trigger"
  shot 63-picker
  # Drag inside the saturation/value square: press at the middle, move down-left,
  # release. The preview and the handle must follow.
  xdotool mousemove "${SX:-511}" "${SY:-491}" mousedown 1; sleep 1
  shot 64-sv-press
  xdotool mousemove "${SX2:-470}" "${SY2:-530}"; sleep 1
  xdotool mouseup 1; sleep 1
  shot 65-sv-drag
  # And the hue strip.
  CLICK_SETTLE=1 click_at "${HX:-670}" "${HY:-490}" "hue strip"
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
  CLICK_SETTLE=3 click_at "${PLUSX:-1225}" "${PLUSY:-788}" "zoom + button"
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
  CLICK_SETTLE=2 click_at "${ZX:-1176}" "${ZY:-788}" "zoom readout"
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
  # T5.3: a highlight may be any colour. All three routes, in one run.
  #
  # The named and custom routes store different model properties
  # (`highlight_color` vs `background_color`) and reach the page by different
  # paths, so exercising only one leaves half the feature unobserved — and the
  # half nobody checks is the one that ships dead. That is not a guess: the
  # custom route's Apply button read the *typed* fields while its enabled state
  # read the square, so it looked live and did nothing (r89).
  #
  # **The order is load-bearing, not arbitrary.** Every pick adds a Recent
  # swatch, and the Recent group shifts the custom column right — so a
  # coordinate that is correct on a first open is wrong on the next one. The
  # harness has told exactly this lie three times now (r80, r86, r90): a stale
  # click lands beside the control and the run reads as "the feature does not
  # work". The typed route therefore runs FIRST, while the panel is at its
  # narrowest, and the custom route last, with the coordinates its own layout
  # has. `push_recent` deduplicates, so the typed and clicked yellows leave one
  # entry between them, not two.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot n0-plain

  select_and_open() {   # heading -> Format -> Highlight
    IDEMPOTENT=1 CLICK_SETTLE=1 click_at "${TX:-375}" "${TY:-172}" "heading"
    xdotool key --clearmodifiers shift+End; sleep 1
    # Already the active tab from the second open onward.
    IDEMPOTENT=1 CLICK_SETTLE=1 click_at "${FX:-98}" "${FY:-696}" "Format tab"
    CLICK_SETTLE=2 click_at "${HX:-108}" "${HY:-738}" "Highlight trigger"
  }

  # 1. A named colour, TYPED as a hex. First, so the panel has no Recent group.
  select_and_open
  shot n1-panel
  CLICK_SETTLE=1 click_at "${EX:-499}" "${EY:-623}" "hex field"
  xdotool type --delay 120 "#FFFF00"; sleep 2
  CLICK_SETTLE=3 click_at "${AX:-494}" "${AY:-655}" "picker Apply"
  shot n2-typed

  # 2. The SAME colour, CLICKED as a swatch. The swatch grid is left of the
  #    Recent group, so its coordinate does not move.
  select_and_open
  CLICK_SETTLE=3 click_at "${YX:-43}" "${YY:-473}" "Yellow swatch"
  shot n3-named

  # 3. A colour that is NOT one of the sixteen: drag the square, then Apply.
  #    One Recent entry now exists, so the custom column sits further right.
  select_and_open
  xdotool mousemove "${SX:-616}" "${SY:-450}" mousedown 1; sleep 1
  xdotool mouseup 1; sleep 1
  CLICK_SETTLE=3 click_at "${AX2:-549}" "${AY:-655}" "picker Apply"
  shot n4-custom

  for s in n2-typed n3-named n4-custom; do
    convert "$SHOT_DIR/$s.png" -crop "${LCROP:-700x50+340+145}" +repage \
      "$SHOT_DIR/$s-line.png" 2>/dev/null
  done
  echo "== page diffs (0 = nothing painted) =="
  echo "  typed:  $(compare -metric AE "$SHOT_DIR/n0-plain.png" "$SHOT_DIR/n2-typed.png" null: 2>&1)"
  echo "  custom: $(compare -metric AE "$SHOT_DIR/n3-named.png" "$SHOT_DIR/n4-custom.png" null: 2>&1)"
  echo "== the routing claim (0 = a typed hex and a clicked swatch agree) =="
  echo "  typed vs clicked: $(compare -metric AE "$SHOT_DIR/n2-typed-line.png" \
    "$SHOT_DIR/n3-named-line.png" null: 2>&1)"
  ;;
ribbonoverflow)
  # I-28: the ribbon overflow ("More") menu. Its controls were dead while it was
  # raised — the menu rendered in place inside `Router`, the dismiss backdrop was
  # a root sibling, and Blitz hit-tests siblings only, so the backdrop won.
  #
  # A narrow window is the whole precondition: the menu does not exist until the
  # strip cannot fit its groups. 720 px overflows the Write tab.
  #
  # The assertion is **clicking a control inside the menu does something**, which
  # is exactly what the defect prevented. A menu that merely appears would have
  # looked identical before the fix.
  export WINSIZE="${WINSIZE:-560x900}"
  SCREEN="${SCREEN:-560x900}" start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot r0-opened
  # Put the caret in the document so a formatting control has something to act on.
  CLICK_SETTLE=1 click_at "${TX:-360}" "${TY:-172}" "heading"
  xdotool key --clearmodifiers shift+End; sleep 1
  shot r1-selected
  # The More button is the last thing in the ribbon strip.
  CLICK_SETTLE=2 click_at "${MX:-512}" "${MY:-845}" "More button"
  shot r2-menu
  # Click a control INSIDE the menu — the Document group's Save. This is the
  # assertion: the defect was that the root backdrop was hit-tested before a
  # menu rendered inside `Router`, so every control in here was dead. A menu
  # that merely appears looked identical before the fix.
  CLICK_SETTLE=3 click_at "${CX:-394}" "${CY:-618}" "menu Save"
  shot r3-clicked
  # **The assertion is the focus ring landing on that button.** The patched
  # shell focuses whatever the pointer hit, so a ring on a control inside the
  # menu is proof the hit test reached it — and reaching it is precisely what
  # the root backdrop used to prevent. The menu stays open, so this is not the
  # menu closing under a click that missed.
  #
  # Not "the document changed": the Write tab's Save does not clear the tab's
  # dirty dot from the *normal* ribbon either (measured — see the control run),
  # so an assertion on that would fail here for a reason that has nothing to do
  # with this menu.
  for s in r2-menu r3-clicked; do
    convert "$SHOT_DIR/$s.png" -crop "${BTNCROP:-60x60+364+588}" +repage \
      "$SHOT_DIR/$s-btn.png" 2>/dev/null
  done
  echo "== diffs =="
  echo "  menu opened:      $(compare -metric AE "$SHOT_DIR/r1-selected.png" "$SHOT_DIR/r2-menu.png" null: 2>&1)"
  echo "  control was hit:  $(compare -metric AE "$SHOT_DIR/r2-menu-btn.png" \
    "$SHOT_DIR/r3-clicked-btn.png" null: 2>&1)"
  ;;

ribbonclearance)
  # **Does the overflow menu cover the button that opened it?** (r93.)
  #
  # `present` grows a floored overlay to the usable minimum, and grew it
  # *downward* whatever side it was placed on. The ribbon is at the window
  # bottom, so its menu is placed `Above` — and downward growth is growth
  # straight across the trigger. The unit suite could not see it: the
  # no-overlap invariant was asserted against `place`, and every consumer
  # reaches geometry through `present`.
  #
  # # The height is the whole experiment, and it has a computable threshold
  #
  # `place` measures room above the anchor as `anchor.y - viewport.y - gap -
  # margin` (4 + 8 here), and only returns a short overlay when that is under
  # `MIN_ANCHORED_MENU_PX` (88). With the More button ~55 px above the window
  # bottom, the floor binds at roughly **H < 155** — which is a landscape phone
  # with the keyboard up, the case the unit fixture models at 150.
  #
  # So HEIGHT is a parameter and the run is meant to be repeated across it: a
  # tall window exercises the ordinary path (regression cover for the same
  # change) and a very short one exercises the floor. The script does not
  # decide which it got — it prints the room it measured so the reader can.
  #
  # # What the sweep actually reached, measured (r93)
  #
  # | HEIGHT | menu rows | height | reading |
  # | --- | --- | ---: | --- |
  # | 420 | 103..326 | 224 | ordinary: fits, no clamp |
  # | 200 | 5..120 | 116 | clamped by `place` to the room; floor not reached |
  # | 160 | 0..92 | 93 | at the floor |
  # | 150 | 0..92 | 93 | at the floor; **anchor top is 96, so the 4 px gap holds** |
  # | 140 | — | — | **no More button**: the ribbon drops its control row first |
  #
  # Built against the pre-fix `present`, HEIGHT=150 gives rows **5..98** — six
  # pixels lower, past the anchor's top edge. That is the whole discriminating
  # band this harness can produce, and it is a sliver of the trigger's box rather
  # than the trigger's glyph. The dramatic case (a menu across the whole control)
  # needs room well under 84 px, and the ribbon's own responsive collapse removes
  # the More button before a desktop window gets that short. **The reachable form
  # of this defect is the landscape-phone-with-IME viewport the unit fixture
  # models at 150 px, and no X11 harness can produce it** — there is no soft
  # keyboard and `current_safe_area` is Android-only, so `viewport.y` is always 0
  # here. Treat this scenario as regression cover for the ordinary path plus a
  # 6 px confirmation, not as the reading that settles the floor.
  #
  # # Measured with the pointer parked, both times
  #
  # Blitz tints a hovered control, and `click_at` leaves the pointer on the More
  # button. Comparing a hovered button against an unhovered one reports the tint
  # and calls it an overlap. Both snapshots below are taken with the pointer
  # moved away, so the only thing that can differ in that crop is what is
  # painted over it.
  H="${HEIGHT:-420}"
  W="${WIDTH:-560}"
  export WINSIZE="${W}x${H}"
  SCREEN="$WINSIZE" start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot c0-opened
  # The More button: last in the strip, and the strip sits above the status bar.
  # Both offsets are from the window bottom, so they follow HEIGHT.
  MX="${MX:-$((W - 48))}"
  MY="${MY:-$((H - 55))}"
  echo "  [geom] window ${W}x${H}, More button at ${MX},${MY}"
  echo "  [geom] room above the anchor = $((MY - 12)) px against an 88 px floor"
  BTNCROP="${BTNCROP:-56x56+$((MX - 28))+$((MY - 28))}"
  # Park the pointer, then photograph the trigger with the menu closed.
  xdotool mousemove "$((W / 2))" "$((H / 3))"; sleep 1
  shot c1-closed
  CLICK_SETTLE=2 click_at "$MX" "$MY" "More button"
  shot c2-menu-hovered
  # Park again: the menu stays open (nothing dismisses on a move — the overflow
  # request carries no `on_outside_move`), so this is the same scene minus the
  # hover tint.
  xdotool mousemove "$((W / 2))" "$((H / 3))"; sleep 1
  shot c3-menu-parked
  # **A control crop, taken from inside the menu.** Without it a near-zero on the
  # trigger says nothing: an instrument that cannot report occlusion anywhere
  # reports none everywhere (evidence rule 3). This box sits one menu-row above
  # the trigger — derived from the same coordinate, so the two cannot go stale
  # apart — and the menu is over it by construction when the menu is open.
  # Clamped: at a very short HEIGHT one menu-row above the trigger is off the top
  # of the window, and a negative crop offset silently becomes a box somewhere
  # else. The clamp keeps it inside the frame; whether it is inside the *menu* is
  # what the printed number reports, which is the point of having it.
  CTL_Y=$((MY - 84)); [ "$CTL_Y" -lt 4 ] && CTL_Y=4
  CTLCROP="56x56+$((MX - 28))+${CTL_Y}"
  for st in c1-closed c3-menu-parked; do
    convert "$SHOT_DIR/$st.png" -crop "$BTNCROP" +repage "$SHOT_DIR/$st-btn.png" 2>/dev/null
    convert "$SHOT_DIR/$st.png" -crop "$CTLCROP" +repage "$SHOT_DIR/$st-ctl.png" 2>/dev/null
  done
  echo "== diffs (of 3136 px per crop) =="
  echo "  menu opened at all:   $(compare -metric AE "$SHOT_DIR/c1-closed.png" \
    "$SHOT_DIR/c3-menu-parked.png" null: 2>&1)"
  # **Fuzzed, and the fuzz is the finding.** A dismissible popover renders a
  # window-wide backdrop, which darkens every pixel by ~4/255 — including the
  # trigger's. A plain `AE` therefore reported 2552 of 3136 "changed" on a
  # trigger that was demonstrably untouched, which is the instrument answering a
  # question adjacent to the one asked. 2% clears the tint and nothing else: an
  # opaque panel over this box moves it to 3136, as the control below shows.
  echo "  trigger overpainted:  $(compare -metric AE -fuzz 2% "$SHOT_DIR/c1-closed-btn.png" \
    "$SHOT_DIR/c3-menu-parked-btn.png" null: 2>&1)  <- near 0 = trigger clear"
  echo "  control, inside menu: $(compare -metric AE -fuzz 2% "$SHOT_DIR/c1-closed-ctl.png" \
    "$SHOT_DIR/c3-menu-parked-ctl.png" null: 2>&1)  <- must be ~3136, or the"
  echo "                        measurement cannot see occlusion at all"
  # Contrast collapse is the second reading of the same thing: a box the menu
  # covers becomes the panel's flat surface.
  echo "  contrast  trigger:    $(convert "$SHOT_DIR/c1-closed-btn.png" -format '%[fx:maxima-minima]' info:) -> $(convert "$SHOT_DIR/c3-menu-parked-btn.png" -format '%[fx:maxima-minima]' info:)"
  echo "  contrast  control:    $(convert "$SHOT_DIR/c1-closed-ctl.png" -format '%[fx:maxima-minima]' info:) -> $(convert "$SHOT_DIR/c3-menu-parked-ctl.png" -format '%[fx:maxima-minima]' info:)"
  ;;

save)
  # **What the ribbon's Save does on the document this harness can open.**
  #
  # Added to settle a claim I got wrong: a throwaway script "showed" that Save
  # never clears the tab's dirty dot, and that was reported as an open question
  # about the save path. Two things were wrong with it. The script used a stale
  # copy of this file's prelude, so it ran against a persisted narrow window and
  # its click never landed on Save at all — no save was ever requested. And the
  # behaviour it was reaching for is correct anyway.
  #
  # ESTABLISHED here: the click reaches Save (`click_at` fails loudly otherwise),
  # and the dot does **not** clear. NOT a defect — the only document this harness
  # can open is created from a template and is therefore untitled
  # (`untitled-1-tpl-screenplay`), and `use_ctrl_s_save` routes an untitled
  # document to Save As, whose file picker cannot appear on a headless Xvfb.
  # An untitled document is also dirty by definition (`use_dirty_tracking`), so
  # the dot staying is the specified behaviour.
  #
  # NOT ESTABLISHED: that Save clears the dot for a document that *has* a path.
  # WHAT WOULD SETTLE IT: opening a titled document, which needs either a file
  # picker (unavailable here) or a path argument to the binary (loki-text takes
  # none). Either is a change outside this harness.
  start_x || exit 1
  start_app env LOKI_DEVICE_PROFILE="${PROFILE:-pointer=fine}" || exit 1
  for i in $(seq 1 "${TABS:-7}"); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot v0-opened
  CLICK_SETTLE=3 click_at "${SX:-36}" "${SY:-738}" "ribbon Save"
  shot v1-saved
  echo "  tab strip changed (0 = still dirty, which is correct for untitled): \
$(compare -metric AE "$(crop v0-opened 200x40+0+0)" "$(crop v1-saved 200x40+0+0)" null: 2>&1)"
  ;;

nestedscroll)
  # ── Probe P1 (Spec 08 T7.0) — does a nested scroll container ROUTE input? ──
  #
  # Gates T7.3, which wants an oversized element to expand into its own
  # horizontal scroll container inside the vertically scrolling document. The
  # scene is `appthere-ui/examples/nested_scroll_probe.rs`; see its module docs
  # for why the failure this looks for is in routing rather than geometry.
  #
  # THE INSTRUMENT. Four bands, cropped from the same window position in every
  # shot, compared pairwise with `compare -metric AE`:
  #   static      — must never change; separates "a container consumed this"
  #                 from "the whole window moved"
  #   outer_above — outer rows above the inner box
  #   inner_in    — the inside of the inner box, excluding its border
  #   outer_below — outer rows below the inner box
  #
  # Each reading is a PAIR of numbers, and the pair is the answer. "inner_in
  # changed" alone proves nothing: the inner box moves when the outer scrolls,
  # so its crop would change either way. Containment is `inner_in != 0 AND
  # outer_above == 0`.
  #
  # NO `windowactivate`. There is no window manager here and the activate call
  # takes the X server down mid-run (measured: the app logs "X connection to :99
  # broken" and every later xdotool fails). This scenario drives the pointer
  # only, which needs no X input focus.
  # Assigned outright, not `${BIN:-…}`: the top of this file already defaulted
  # BIN to loki-text-desktop, so a defaulting assignment here never fires. This
  # scenario is about a different binary, and PROBE_BIN is the override hook.
  BIN="${PROBE_BIN:-$ROOT/target/debug/examples/nested_scroll_probe}"
  NO_ACTIVATE=1
  ST="880x40+10+14"; OA="880x150+10+68"; II="790x70+58+240"; OB="880x140+10+335"
  IX="${IX:-450}"; IY="${IY:-280}"     # inside the inner box
  OX="${OX:-450}"; OY="${OY:-400}"     # an outer row below the inner box

  start_x || exit 1
  start_app || exit 1
  shot n0-rest

  echo "== R1 control: wheel over an OUTER-only row =="
  # Established first, and deliberately: if the wheel reaches nothing at all,
  # "the inner did not scroll" is a null result from a dead instrument rather
  # than a finding about nesting.
  xdotool mousemove "$OX" "$OY"; sleep 1
  wheel 5 3
  shot n1-outer-wheeled
  echo "  static      : $(band "$ST" n0-rest n1-outer-wheeled)   (want 0)"
  echo "  outer_above : $(band "$OA" n0-rest n1-outer-wheeled)   (want >0 — the wheel reaches the outer)"

  echo "== R2 containment: wheel over the INNER box, from rest =="
  # Restart so R2 starts from the same state R1 did; otherwise it would be
  # measuring against an outer that R1 already moved.
  stop; sleep 1
  start_x || exit 1
  start_app || exit 1
  shot n2-rest
  xdotool mousemove "$IX" "$IY"; sleep 1
  wheel 5 3
  shot n3-inner-wheeled
  echo "  static      : $(band "$ST" n2-rest n3-inner-wheeled)   (want 0)"
  echo "  inner_in    : $(band "$II" n2-rest n3-inner-wheeled)   (want >0 — the inner consumed it)"
  echo "  outer_above : $(band "$OA" n2-rest n3-inner-wheeled)   (want 0 — the outer did NOT move)"
  echo "  outer_below : $(band "$OB" n2-rest n3-inner-wheeled)   (want 0 — the outer did NOT move)"

  echo "== R3 bubbling: keep wheeling past the inner's end =="
  # The inner holds 8 rows of 40 px in an 80 px view: 240 px of range, 12 notches
  # at 20 px each. R2 spent 3, so 20 more is comfortably past the end.
  wheel 5 20
  shot n4-inner-exhausted
  echo "  static      : $(band "$ST" n3-inner-wheeled n4-inner-exhausted)   (want 0)"
  echo "  outer_above : $(band "$OA" n3-inner-wheeled n4-inner-exhausted)   (want >0 — the remainder bubbled)"
  echo "  outer_below : $(band "$OB" n3-inner-wheeled n4-inner-exhausted)   (want >0 — the remainder bubbled)"

  echo "== R4 the T7.3 configuration: vertical wheel over a HORIZONTAL-only inner =="
  # This is the reading that actually gates T7.3. R1-R3 nest two vertical
  # scrollers, where the inner *can* use the gesture. T7.3 ships the other
  # shape: a wide table in its own horizontal scroller inside the vertically
  # scrolling document. A vertical gesture there is one the inner container
  # cannot use — and if it swallows it anyway, the document stops scrolling
  # wherever the pointer happens to rest, which is worse than the sideways
  # scrolling T7.3 exists to remove.
  stop; sleep 1
  start_x || exit 1
  PROBE_SCENE=horizontal start_app env PROBE_SCENE=horizontal || exit 1
  shot n5-h-rest
  xdotool mousemove "$IX" "$IY"; sleep 1
  wheel 5 3
  shot n6-h-vwheel
  echo "  static      : $(band "$ST" n5-h-rest n6-h-vwheel)   (want 0)"
  echo "  outer_above : $(band "$OA" n5-h-rest n6-h-vwheel)   (want >0 — vertical bubbled past the horizontal inner)"

  echo "== R5 horizontal wheel over the horizontal inner =="
  # X buttons 6/7 are the horizontal wheel. If winit does not deliver them the
  # inner will not move, and that is a real answer about this instrument rather
  # than about Blitz — which is why R4 above, not this, is the gating reading.
  stop; sleep 1
  start_x || exit 1
  PROBE_SCENE=horizontal start_app env PROBE_SCENE=horizontal || exit 1
  shot n7-h-rest
  xdotool mousemove "$IX" "$IY"; sleep 1
  wheel 7 3
  shot n8-h-hwheel
  echo "  static      : $(band "$ST" n7-h-rest n8-h-hwheel)   (want 0)"
  echo "  inner_in    : $(band "$II" n7-h-rest n8-h-hwheel)   (>0 = the inner took it)"
  echo "  outer_above : $(band "$OA" n7-h-rest n8-h-hwheel)   (want 0 — the outer did NOT move)"
  ;;

statusoverflow)
  # ── T7.1: does the status bar drop items by measured width, keep the page
  # indicator and zoom, and offer the rest behind a "More" popover? ──
  #
  # Two runs at two widths, because the claim is comparative: the same document
  # in a wide window and a narrow one must produce different bars. One width
  # alone would photograph a bar and say nothing about *why* it looks that way.
  #
  # The reading is the bottom strip. `STATUS` crops the bar's full width at the
  # window bottom; `STATUS_L` and `STATUS_R` are its two halves, so "the left
  # end still shows a page label" is separable from "the right end changed".
  start_x || exit 1
  WINSIZE="${WIDE:-1200x760}" start_app env LOKI_DEVICE_PROFILE=pointer=fine || exit 1
  for _ in $(seq 1 7); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot s0-wide
  echo "  wide window:   $(xdotool getactivewindow getwindowgeometry 2>/dev/null | tr '\n' ' ')"

  stop; sleep 1
  start_x || exit 1
  WINSIZE="${NARROW:-420x760}" start_app env LOKI_DEVICE_PROFILE=pointer=fine || exit 1
  for _ in $(seq 1 7); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-12}"
  shot s1-narrow
  echo "  narrow window: $(xdotool getactivewindow getwindowgeometry 2>/dev/null | tr '\n' ' ')"
  echo "  Read the two shots: s0-wide has the full bar; s1-narrow should show the"
  echo "  page indicator, the zoom control, and a … trigger for the rest."
  ;;

domreflow)
  # ── ADR-0017 step 2: the DOM reflow view, beside the canvas one ──
  #
  # Same document, same window, same view mode — twice, once per rendering
  # path. The comparison is the point: a single shot of the DOM path would show
  # that it renders, which is not the question. The question is whether it
  # renders the same document.
  #
  # `WIDE` is deliberately narrow enough that Reflow is the default view mode,
  # so neither run needs to toggle it and the two differ only in `LOKI_REFLOW_DOM`.
  start_x || exit 1
  WINSIZE="${SIZE:-900x800}" start_app env LOKI_DEVICE_PROFILE=pointer=fine || exit 1
  for _ in $(seq 1 7); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-14}"
  # Toggle to Reflow explicitly. The width-based default moved between runs
  # once already, and a scenario that assumes it silently photographs the wrong
  # view — which is exactly what this comparison must not do.
  CLICK_SETTLE=4 click_at "${VX:-659}" "${VY:-787}" "view-mode chip"
  shot d0-canvas

  stop; sleep 1
  start_x || exit 1
  WINSIZE="${SIZE:-900x800}" start_app env LOKI_DEVICE_PROFILE=pointer=fine LOKI_REFLOW_DOM=1 \
    || exit 1
  for _ in $(seq 1 7); do key Tab; done
  key Return
  sleep "${OPEN_SETTLE:-14}"
  CLICK_SETTLE=4 click_at "${VX:-659}" "${VY:-787}" "view-mode chip"
  shot d1-dom
  echo "  Compare d0-canvas (painted) against d1-dom (DOM). They render the same"
  echo "  document; they are not expected to be pixel-identical — see ADR-0017."
  ;;

styledlinebreak)
  # ── ADR-0017 §5.3 step 3: the styled document's line breaks, measured ──
  #
  # Renders the screenplay template through the *real* DOM reflow view at every
  # width `styled_linebreak_sweep` reports a transition at, all in one row, and
  # measures each column's height. Pair the output with the sweep's counts.
  #
  #   cargo run -p loki-text --example styled_linebreak_sweep    # layout side
  #   scripts/sitting/run.sh styledlinebreak                     # DOM side
  #
  # The screen is as wide as the row: one shot rather than one launch per width,
  # and one set of conditions rather than one per width.
  #
  # `LB_FIXTURE` selects the document (both halves read it): `screenplay`, or
  # `mixed`, or `mixed:<case>` for one of weight/italic/size/family/charstyle/
  # spacing. Run the six cases in a loop to attribute a disagreement:
  #
  #   for c in weight italic size family charstyle spacing; do
  #     LB_FIXTURE=mixed:$c scripts/sitting/run.sh styledlinebreak; done
  BIN="$ROOT/target/debug/examples/styled_linebreak_probe"
  SWEEP="$ROOT/target/debug/examples/styled_linebreak_sweep"
  if [ ! -x "$BIN" ] || [ ! -x "$SWEEP" ]; then
    echo "build them first: cargo build -p loki-text \\"
    echo "    --example styled_linebreak_probe --example styled_linebreak_sweep"
    exit 1
  fi
  # The widths and the calibration come from the layout half, not from a list
  # kept here: a transcribed list is a second copy of the sweep's answer, and
  # the first thing it loses is which fixture it came from.
  echo "  [sweep] ${LB_FIXTURE:-screenplay}"
  SWEEP_OUT="$SHOT_DIR/slb-sweep.txt"
  "$SWEEP" > "$SWEEP_OUT" 2>/dev/null || { echo "sweep failed"; exit 1; }
  # `STYLED_WIDTHS`/`CALIBRATE` override the sweep, for locating a DOM-side
  # transition exactly: pass a contiguous range and read where the count steps.
  WIDTHS="${STYLED_WIDTHS:-$(sed -n 's/^probe-widths //p' "$SWEEP_OUT")}"
  CAL="${CALIBRATE:-$(sed -n 's/^calibrate //p' "$SWEEP_OUT")}"
  if [ -z "$WIDTHS" ]; then echo "sweep found no transitions"; exit 1; fi
  # Row width: each column is its content width plus the view's 48 px of
  # padding, plus an 8 px gap, and 64 px of slack for the UA body margin and
  # the window frame.
  TOTAL=$(echo "$WIDTHS" | awk -F, '{s=0; for(i=1;i<=NF;i++) s+=$i+56; print s+64}')
  echo "  [sweep] $(echo "$WIDTHS" | awk -F, '{print NF}') columns, calibrate=$CAL, row=${TOTAL}px"
  # One knob: the window and the screen are the same height, because a column
  # taller than the window is clipped by the view's scroll container and would
  # measure as a shorter document. Raise it for a fixture with more paragraphs.
  PROBE_HEIGHT="${PROBE_HEIGHT:-900}"
  SCREEN="${SCREEN:-${TOTAL}x${PROBE_HEIGHT}}" start_x || exit 1
  # NO_ACTIVATE: nothing is typed at this probe, and activating a window on this
  # Xvfb takes the server down (see `start_app`).
  NO_ACTIVATE=1 start_app env STYLED_WIDTHS="$WIDTHS" PROBE_HEIGHT="$PROBE_HEIGHT" || exit 1
  sleep "${OPEN_SETTLE:-20}"
  shot slb
  cp "$SHOT_DIR/app.log" "$SHOT_DIR/slb.log"
  stop
  "$ROOT/scripts/sitting/linebreak_bands.py" \
    --shot "$SHOT_DIR/slb.png" --log "$SHOT_DIR/slb.log" --calibrate "$CAL"
  ;;

advances)
  # ── ADR-0017 §5.6: per-run advances, DOM against canvas ──
  #
  # §5.5 left a residual of order 0.1 % of a line's advance. This measures that
  # quantity rather than the break it flips: one row per resolved style, each in
  # a span emitting the reflow view's own CSS plus a red background, whose
  # rectangle *is* the run's advance in CSS px.
  #
  #   LB_FIXTURE=advances:1 scripts/sitting/run.sh advances
  #   LB_FIXTURE=advances:8 scripts/sitting/run.sh advances
  #
  # Two scales, because a rounding difference is a fixed number of pixels and a
  # metrics difference is a fraction of one.
  BIN="$ROOT/target/debug/examples/advance_probe"
  LINES="$ROOT/target/debug/examples/styled_linebreak_lines"
  if [ ! -x "$BIN" ] || [ ! -x "$LINES" ]; then
    echo "build them first: cargo build -p loki-text \\"
    echo "    --example advance_probe --example styled_linebreak_lines"
    exit 1
  fi
  export LB_FIXTURE="${LB_FIXTURE:-advances:1}"
  # The canvas half at a width nothing can wrap at, so each paragraph is one
  # line and that line's advance is the run's advance.
  LB_WIDTH="${LB_WIDTH:-9000}" "$LINES" > "$SHOT_DIR/adv-canvas.txt" 2>/dev/null \
    || { echo "canvas half failed"; exit 1; }
  # Wide enough for the largest scale this scenario is run at; the measuring
  # script refuses a band that reaches the last column rather than reporting a
  # clipped run as a short one.
  SCREEN="${SCREEN:-6000x2000}" start_x || exit 1
  NO_ACTIVATE=1 start_app env LB_FIXTURE="$LB_FIXTURE" || exit 1
  sleep "${OPEN_SETTLE:-20}"
  shot adv
  cp "$SHOT_DIR/app.log" "$SHOT_DIR/adv.log"
  stop
  "$ROOT/scripts/sitting/advance_bands.py" \
    --shot "$SHOT_DIR/adv.png" --log "$SHOT_DIR/adv.log" \
    --canvas "$SHOT_DIR/adv-canvas.txt"
  ;;
esac
echo "DONE: $1"
