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
reset_window_state() {
  rm -f "${XDG_DATA_HOME:-$HOME/.local/share}/AppThere/Loki/window.json" 2>/dev/null
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
      xdotool windowactivate "$WIN" 2>/dev/null
      xdotool windowfocus --sync "$WIN" 2>/dev/null
      xdotool windowraise "$WIN" 2>/dev/null
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
esac
echo "DONE: $1"
