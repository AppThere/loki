// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `AtCalibrateDialog` — measure this display against a real ruler (Spec 08
//! T5.5, D-04).
//!
//! # It draws a line and asks how long it is
//!
//! The alternative — "enter your display's DPI" — asks the reader for a number
//! about their hardware that most people cannot look up and many vendors state
//! wrongly. Drawing a line the app *believes* is a known length and asking for
//! its true length asks for the measurement the number is made of, which anyone
//! with a ruler can answer correctly. `loki_app_shell::display_density` turns the
//! two lengths into a density and refuses answers it cannot believe.
//!
//! # The reference is a bank card as well as a ruler
//!
//! Not everyone has a ruler to hand; almost everyone has a card, and ISO/IEC
//! 7810 ID-1 fixes its width at 85.6 mm to a tolerance far finer than this
//! measurement needs. So the line is drawn at exactly that width and labelled
//! both ways: a reader with a ruler types what they measure, a reader with a card
//! holds it against the line and adjusts until they match.
//!
//! # Offered on first use, never at first run (D-04)
//!
//! This component does not decide when it appears — the caller does, on the first
//! use of Actual Size for an uncalibrated display. Mount it at a boundary
//! (ADR-0013):
//!
//! ```rust,ignore
//! {calibrating().then(|| rsx! {
//!     AtCalibrateDialog { labels, assumed_css_ppi, on_measured, on_cancel }
//! })}
//! ```
//!
//! # Touch target
//!
//! Both buttons and the field are at least [`TOUCH_MIN`] × [`TOUCH_MIN`]
//! (44 × 44 logical px, WCAG 2.5.8) — this is a dialog, not the 24 px status
//! bar, so the full target is available and taken.

use dioxus::prelude::*;

use crate::tokens::colors::{
    COLOR_ACCENT_PRIMARY, COLOR_BORDER_CHROME, COLOR_SURFACE_1, COLOR_TEXT_ON_CHROME,
    COLOR_TEXT_ON_CHROME_SECONDARY,
};
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_2, SPACE_3, SPACE_4, TOUCH_MIN};
use crate::tokens::typography::{FONT_SIZE_BODY, FONT_SIZE_MD, FONT_WEIGHT_SEMIBOLD};

/// The reference length drawn, in millimetres: the ISO/IEC 7810 ID-1 width that
/// every bank, credit and ID card shares.
pub const REFERENCE_MM: f32 = 85.6;

/// Width of the dialog card, in logical pixels.
///
/// Wide enough to hold the reference line at the CSS assumption (85.6 mm is
/// ~323 CSS px at 96 ppi) plus padding, so the line never has to be scaled to
/// fit — a scaled reference is a reference that measures the wrong thing.
const DIALOG_WIDTH_PX: f32 = 400.0;

/// Translated prose. Passed in, because `appthere_ui` is i18n-agnostic.
#[derive(Clone, PartialEq)]
pub struct AtCalibrateLabels {
    /// Dialog heading.
    pub title: String,
    /// Explains what to do — measure the line, or match a card to it.
    pub instructions: String,
    /// Label of the measured-length field.
    pub field_label: String,
    /// Label of the button that accepts the measurement.
    pub apply: String,
    /// Label of the button that closes without measuring.
    pub cancel: String,
    /// Shown when the typed measurement cannot be believed.
    pub rejected: String,
}

/// Props for [`AtCalibrateDialog`].
#[derive(Props, Clone, PartialEq)]
pub struct AtCalibrateDialogProps {
    /// Translated prose.
    pub labels: AtCalibrateLabels,
    /// The density the drawn line is drawn at — what the app currently believes.
    pub assumed_css_ppi: f32,
    /// The reader's measurement, in millimetres — **and whether it was
    /// believed**.
    ///
    /// # It returns a `bool` because this dialog must not own the second copy
    ///
    /// Believability is `loki_app_shell::display_density::calibrated_css_ppi`'s
    /// to decide: it refuses a measurement implying a density ratio outside
    /// `0.5..=2.0`. This crate cannot call it (`appthere_ui` sits below
    /// `loki_app_shell`), and the first version therefore kept its own rule —
    /// accept any positive number — and fired `on_measured` as an
    /// `EventHandler<f32>` with nowhere to say no.
    ///
    /// The two rules agreed everywhere except where it mattered (rule 4).
    /// Typing centimetres for millimetres is the mistake the dialog's own prose
    /// warns about, and `8.56` is positive: the dialog cleared its rejection
    /// notice, the caller's `let … else { return }` swallowed the value, and
    /// **Apply did nothing at all, silently** — the dead-control class, in the
    /// one control whose whole job is to tell the reader their measurement was
    /// not believed.
    ///
    /// Returning the verdict is what makes the refusal unavoidable rather than
    /// documented (L08-043): a caller cannot drop it, because the type will not
    /// let it, and this dialog cannot get the answer wrong because it never
    /// forms one. What stays here is the *parse* — "is this text a number" is
    /// genuinely the field's own question.
    pub on_measured: Callback<f32, bool>,
    /// Dismissed without measuring.
    pub on_cancel: EventHandler<()>,
}

/// Reads the field as a length in millimetres.
///
/// **The whole of what this dialog decides**, and deliberately only a parse: a
/// comma decimal separator is the same number a full stop is (most of Europe
/// writes `85,6`), and a negative or non-finite length is not a measurement
/// anybody made. Whether the number is *believable* is a different question with
/// a different owner — see [`AtCalibrateDialogProps::on_measured`], which is
/// where the two used to be conflated.
#[must_use]
fn parse_measurement(text: &str) -> Option<f32> {
    let mm = text.trim().replace(',', ".").parse::<f32>().ok()?;
    (mm.is_finite() && mm > 0.0).then_some(mm)
}

/// The calibration dialog.
#[component]
pub fn AtCalibrateDialog(props: AtCalibrateDialogProps) -> Element {
    let mut typed = use_signal(String::new);
    let mut rejected = use_signal(|| false);

    // The reference line's on-screen width at the assumed density. If the
    // assumption is right the line really is 85.6 mm and the reader types 85.6;
    // the error in the assumption is exactly what the measurement recovers.
    let line_px = (REFERENCE_MM / 25.4 * props.assumed_css_ppi)
        .round()
        .max(1.0);

    let submit = move |_| {
        // Two questions, and only the first is this dialog's: whether the text is
        // a number, and whether the number is believable. The second is asked of
        // the caller and its answer is what sets the notice — see `on_measured`.
        let accepted =
            parse_measurement(&typed.read()).is_some_and(|mm| props.on_measured.call(mm));
        // Not accepted, and *said so* rather than silently ignored: a button that
        // does nothing reads as a broken dialog, and the commonest bad input here
        // (centimetres for millimetres) looks perfectly reasonable to the person
        // who typed it — which is exactly the case that used to clear this notice
        // and then discard the value.
        rejected.set(!accepted);
    };

    rsx! {
        div {
            style: format!(
                "position: absolute; inset: 0; background: rgba(0,0,0,0.5); \
                 display: flex; align-items: center; justify-content: center; \
                 z-index: 50; ",
            ),
            onclick: move |_| props.on_cancel.call(()),
            div {
                // The card swallows clicks so measuring inside it does not
                // dismiss the thing being measured.
                onclick: move |e| e.stop_propagation(),
                style: format!(
                    "width: {w}px; background: {bg}; border: 1px solid {bd}; \
                     border-radius: {r}px; padding: {p}px; color: {fg}; \
                     box-sizing: border-box;",
                    w = DIALOG_WIDTH_PX,
                    bg = COLOR_SURFACE_1,
                    bd = COLOR_BORDER_CHROME,
                    r = RADIUS_MD,
                    p = SPACE_4,
                    fg = COLOR_TEXT_ON_CHROME,
                ),
                div {
                    style: format!(
                        "font-size: {s}px; font-weight: {w}; margin-bottom: {m}px;",
                        s = FONT_SIZE_MD, w = FONT_WEIGHT_SEMIBOLD, m = SPACE_2,
                    ),
                    "{props.labels.title}"
                }
                div {
                    style: format!(
                        "font-size: {s}px; color: {c}; margin-bottom: {m}px;",
                        s = FONT_SIZE_BODY,
                        c = COLOR_TEXT_ON_CHROME_SECONDARY,
                        m = SPACE_3,
                    ),
                    "{props.labels.instructions}"
                }
                // The reference line. A filled bar rather than a border, so its
                // measured extent is exactly its width — a 1px border would put
                // the ends half a pixel out, which is invisible here and would
                // be a real error on a very small reference.
                div {
                    style: format!(
                        "width: {line_px}px; height: 12px; background: {c}; \
                         border-radius: {r}px; margin-bottom: {m}px;",
                        c = COLOR_ACCENT_PRIMARY,
                        r = RADIUS_SM,
                        m = SPACE_3,
                    ),
                }
                div {
                    style: format!(
                        "font-size: {s}px; margin-bottom: {m}px;",
                        s = FONT_SIZE_BODY, m = SPACE_2,
                    ),
                    "{props.labels.field_label}"
                }
                input {
                    r#type: "text",
                    value: "{typed}",
                    // The dialog exists to collect this one value, so the caret
                    // belongs here on open. Without it the reader must click the
                    // field before typing — which the first screen sitting caught,
                    // by typing into a dialog that ignored every keystroke.
                    autofocus: "true",
                    oninput: move |e| {
                        rejected.set(false);
                        typed.set(e.value());
                    },
                    style: format!(
                        "width: 100%; min-height: {t}px; box-sizing: border-box; \
                         padding: 0 {p}px; border-radius: {r}px; \
                         border: 1px solid {bd}; font-size: {s}px;",
                        t = TOUCH_MIN, p = SPACE_2, r = RADIUS_SM,
                        bd = COLOR_BORDER_CHROME, s = FONT_SIZE_BODY,
                    ),
                }
                if rejected() {
                    div {
                        style: format!(
                            "font-size: {s}px; color: {c}; margin-top: {m}px;",
                            s = FONT_SIZE_BODY,
                            c = COLOR_TEXT_ON_CHROME_SECONDARY,
                            m = SPACE_2,
                        ),
                        "{props.labels.rejected}"
                    }
                }
                div {
                    style: format!(
                        "display: flex; gap: {g}px; justify-content: flex-end; \
                         margin-top: {m}px;",
                        g = SPACE_2, m = SPACE_3,
                    ),
                    button {
                        style: dialog_button(false),
                        onclick: move |_| props.on_cancel.call(()),
                        "{props.labels.cancel}"
                    }
                    button {
                        style: dialog_button(true),
                        onclick: submit,
                        "{props.labels.apply}"
                    }
                }
            }
        }
    }
}

/// Shared button style; `primary` fills with the accent.
fn dialog_button(primary: bool) -> String {
    format!(
        "min-height: {t}px; padding: 0 {p}px; border-radius: {r}px; \
         cursor: pointer; font-size: {s}px; border: 1px solid {bd}; \
         background: {bg}; color: {fg};",
        t = TOUCH_MIN,
        p = SPACE_3,
        r = RADIUS_SM,
        s = FONT_SIZE_BODY,
        bd = COLOR_BORDER_CHROME,
        bg = if primary {
            COLOR_ACCENT_PRIMARY
        } else {
            "transparent"
        },
        fg = COLOR_TEXT_ON_CHROME,
    )
}

#[cfg(test)]
#[path = "calibrate_dialog_tests.rs"]
mod tests;
