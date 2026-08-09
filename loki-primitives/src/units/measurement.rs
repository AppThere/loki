// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The user's **measurement unit** — which unit lengths are shown and typed in
//! (Spec 08 T6.4, decision D-03).
//!
//! # Display and entry only
//!
//! This changes no stored value. The model keeps its typed [`Length`] units and
//! the formats keep theirs; this is the presentation layer's answer to "what
//! unit does this number wear". A round-trip through
//! [`MeasurementUnit::format`] and [`MeasurementUnit::parse`] returns the same
//! length, so switching unit cannot edit a document.
//!
//! # Resolution order
//!
//! D-03: **explicit user setting → OS measurement setting → locale region →
//! metric.** The precedence is structural rather than documented:
//! [`effective_measurement_unit`] takes the explicit setting as its argument and
//! the ambient resolution is private, so there is no way to ask for the
//! environment's answer while forgetting the user's.
//!
//! # Paper size is a different question
//!
//! `loki_doc_model`'s `default_page_size_for_locale` reads the same locale to
//! choose A4 vs US Letter, and its region list is deliberately **not** this
//! one. Mexico, Canada and the Philippines use US Letter paper and are metric;
//! unifying the two lists would get one of the two answers wrong for every one
//! of them. `mexico_uses_letter_paper_but_metric_units` pins that apart.
//!
//! [`Length`]: super::Length

use super::well_known::Points;

/// A unit the user can see and type lengths in.
///
/// Font sizes are **not** subject to this: type size is quoted in points by
/// universal typographic convention, the way LibreOffice and Word both keep it
/// regardless of the measurement setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MeasurementUnit {
    /// Millimetres — the metric default.
    #[default]
    Millimeter,
    /// Centimetres.
    Centimeter,
    /// Inches.
    Inch,
    /// Typographic points (1/72 in).
    Point,
    /// Picas (12 pt).
    Pica,
}

impl MeasurementUnit {
    /// Every unit, in the order a settings menu lists them.
    pub const ALL: &'static [MeasurementUnit] = &[
        MeasurementUnit::Millimeter,
        MeasurementUnit::Centimeter,
        MeasurementUnit::Inch,
        MeasurementUnit::Point,
        MeasurementUnit::Pica,
    ];

    /// The suffix shown after a formatted value, and accepted when typed.
    #[must_use]
    pub const fn abbreviation(self) -> &'static str {
        match self {
            MeasurementUnit::Millimeter => "mm",
            MeasurementUnit::Centimeter => "cm",
            MeasurementUnit::Inch => "in",
            MeasurementUnit::Point => "pt",
            MeasurementUnit::Pica => "pc",
        }
    }

    /// How many of this unit make one point — the single conversion fact each
    /// variant contributes, from which both directions are derived.
    #[must_use]
    const fn per_point(self) -> f64 {
        match self {
            MeasurementUnit::Millimeter => 25.4 / 72.0,
            MeasurementUnit::Centimeter => 2.54 / 72.0,
            MeasurementUnit::Inch => 1.0 / 72.0,
            MeasurementUnit::Point => 1.0,
            MeasurementUnit::Pica => 1.0 / 12.0,
        }
    }

    /// Decimal places used when formatting. Chosen so one step of the last
    /// digit is under a third of a millimetre in every unit — coarser rounding
    /// would make a value that cannot be typed back in.
    #[must_use]
    const fn decimals(self) -> usize {
        match self {
            MeasurementUnit::Millimeter | MeasurementUnit::Point => 1,
            MeasurementUnit::Centimeter | MeasurementUnit::Inch | MeasurementUnit::Pica => 2,
        }
    }

    /// `length` expressed as a bare number in this unit.
    #[must_use]
    pub fn value_of(self, length: Points) -> f64 {
        length.value() * self.per_point()
    }

    /// `amount` of this unit as a length in points.
    #[must_use]
    pub fn to_points(self, amount: f64) -> Points {
        Points::new(amount / self.per_point())
    }

    /// `length` formatted for display, with the unit suffix — e.g. `"25.4 mm"`.
    #[must_use]
    pub fn format(self, length: Points) -> String {
        format!(
            "{:.*} {}",
            self.decimals(),
            self.value_of(length),
            self.abbreviation()
        )
    }

    /// `length` formatted without the suffix, for a text field that labels its
    /// own unit.
    #[must_use]
    pub fn format_bare(self, length: Points) -> String {
        format!("{:.*}", self.decimals(), self.value_of(length))
    }

    /// Parses typed `text` into a length.
    ///
    /// A bare number is read in **this** unit; an explicit suffix (`"2in"`,
    /// `"12 pt"`, case-insensitive) overrides it, so a user who knows what they
    /// want is not forced through the current setting. Returns `None` for an
    /// unparseable number, an unrecognised suffix, or a non-finite value.
    #[must_use]
    pub fn parse(self, text: &str) -> Option<Points> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }
        // Split the trailing alphabetic suffix, if any, from the number.
        let split = t
            .char_indices()
            .rev()
            .take_while(|(_, c)| c.is_alphabetic() || *c == '"' || *c == '\'')
            .map(|(i, _)| i)
            .last()
            .unwrap_or(t.len());
        let (num, suffix) = t.split_at(split);
        let suffix = suffix.trim().to_ascii_lowercase();
        let unit = if suffix.is_empty() {
            self
        } else {
            MeasurementUnit::ALL
                .iter()
                .copied()
                .find(|u| u.abbreviation() == suffix)?
        };
        let amount: f64 = num.trim().parse().ok()?;
        if !amount.is_finite() {
            return None;
        }
        Some(unit.to_points(amount))
    }
}

/// The unit implied by a POSIX `LC_MEASUREMENT` value: `1` (or a locale naming a
/// metric region) is metric, `2` is US customary.
fn unit_from_lc_measurement(value: &str) -> Option<MeasurementUnit> {
    match value.trim() {
        "1" => Some(MeasurementUnit::Millimeter),
        "2" => Some(MeasurementUnit::Inch),
        _ => None,
    }
}

/// Regions that measure in US customary units.
///
/// Deliberately short: the United States, plus Liberia and Myanmar, which are
/// the only other countries that have not adopted the metric system. It is
/// **not** the US-Letter-paper list — see the module docs.
const IMPERIAL_REGIONS: &[&str] = &["_US", "_LR", "_MM"];

/// The unit implied by a locale string such as `"en_US.UTF-8"`.
fn unit_from_locale(locale: &str) -> Option<MeasurementUnit> {
    let upper = locale.to_uppercase();
    if upper.is_empty() {
        return None;
    }
    // A bare language with no region ("C", "POSIX", "en") says nothing about
    // measurement; falling through to metric is the D-03 default, not a guess
    // that the speaker is American.
    if IMPERIAL_REGIONS.iter().any(|r| upper.contains(r)) {
        Some(MeasurementUnit::Inch)
    } else if upper.contains('_') {
        Some(MeasurementUnit::Millimeter)
    } else {
        None
    }
}

/// The D-03 chain over already-read environment values — pure, so the
/// precedence is testable without mutating the process environment.
///
/// `explicit` is the user's setting, `lc_measurement` the OS measurement
/// setting, `locale` the region fallback; metric ends the chain.
#[must_use]
pub fn resolve_measurement_unit(
    explicit: Option<MeasurementUnit>,
    lc_measurement: Option<&str>,
    locale: Option<&str>,
) -> MeasurementUnit {
    explicit
        .or_else(|| lc_measurement.and_then(unit_from_lc_measurement))
        .or_else(|| locale.and_then(unit_from_locale))
        .unwrap_or(MeasurementUnit::Millimeter)
}

/// The environment's answer, read once — the environment does not change within
/// a process, and this is consulted on every render of every length field.
///
/// Private on purpose: exposing it would let a caller ask the environment while
/// forgetting the user's setting, which is the one mistake the D-03 order
/// exists to prevent. Go through [`effective_measurement_unit`].
fn ambient_measurement_unit() -> MeasurementUnit {
    use std::sync::OnceLock;
    static AMBIENT: OnceLock<MeasurementUnit> = OnceLock::new();
    *AMBIENT.get_or_init(|| {
        let lc = std::env::var("LC_MEASUREMENT").ok();
        let locale = ["LC_ALL", "LC_MEASUREMENT", "LANG", "LANGUAGE"]
            .iter()
            .find_map(|v| std::env::var(v).ok())
            .filter(|s| !s.is_empty());
        resolve_measurement_unit(None, lc.as_deref(), locale.as_deref())
    })
}

/// The unit to display and parse in: `explicit` when the user has chosen one,
/// else the environment's answer.
///
/// `explicit` is `None` until a settings store exists to hold it — persisting
/// the choice is T6.3's job, and this signature is already the shape that will
/// take it.
#[must_use]
pub fn effective_measurement_unit(explicit: Option<MeasurementUnit>) -> MeasurementUnit {
    explicit.unwrap_or_else(ambient_measurement_unit)
}

#[cfg(test)]
#[path = "measurement_tests.rs"]
mod tests;
