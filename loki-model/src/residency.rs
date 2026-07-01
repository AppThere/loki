// SPDX-License-Identifier: Apache-2.0

//! Data-residency type (ADR-C018 / ADR-C019).
//!
//! Residency is recorded per workspace and per document. Managed (Hetzner)
//! deployments are pinned to the EU regions `fsn1`, `nbg1`, `hel1`;
//! self-hosted deployments declare a free-form `self-hosted:<label>` value
//! because the operator's own infrastructure is, by definition, inside their
//! jurisdiction.

use serde::{Deserialize, Serialize};

/// Hetzner regions permitted for managed deployments (DE/FI — EU only).
pub const ALLOWED_HETZNER_REGIONS: [&str; 3] = ["fsn1", "nbg1", "hel1"];

const SELF_HOSTED_PREFIX: &str = "self-hosted:";

/// Where a workspace's / document's data physically resides.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Residency {
    /// A pinned Hetzner EU region (`fsn1`, `nbg1`, or `hel1`).
    HetznerRegion(String),
    /// Customer-operated infrastructure, with an operator-chosen label.
    SelfHosted(String),
}

impl Residency {
    /// Parses and validates a residency value from configuration.
    ///
    /// Managed regions outside the EU allow-list are rejected at config
    /// validation time (ADR-C019, SOV-2).
    pub fn parse(value: &str) -> Result<Self, ResidencyError> {
        if let Some(label) = value.strip_prefix(SELF_HOSTED_PREFIX) {
            if label.is_empty() {
                return Err(ResidencyError::EmptySelfHostedLabel);
            }
            return Ok(Self::SelfHosted(label.to_owned()));
        }
        if ALLOWED_HETZNER_REGIONS.contains(&value) {
            return Ok(Self::HetznerRegion(value.to_owned()));
        }
        Err(ResidencyError::DisallowedRegion(value.to_owned()))
    }

    /// Stable text form stored in the database and config.
    #[must_use]
    pub fn as_config_value(&self) -> String {
        match self {
            Self::HetznerRegion(region) => region.clone(),
            Self::SelfHosted(label) => format!("{SELF_HOSTED_PREFIX}{label}"),
        }
    }
}

impl std::fmt::Display for Residency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.as_config_value())
    }
}

impl TryFrom<String> for Residency {
    type Error = ResidencyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Residency> for String {
    fn from(residency: Residency) -> Self {
        residency.as_config_value()
    }
}

/// Error returned for a residency value that violates the sovereignty pin.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResidencyError {
    /// A managed region outside `fsn1`/`nbg1`/`hel1` (never a US location).
    #[error("residency {0:?} is not an allowed EU region (fsn1, nbg1, hel1) or self-hosted:<label>")]
    DisallowedRegion(String),
    /// `self-hosted:` with no label.
    #[error("self-hosted residency requires a non-empty label (self-hosted:<label>)")]
    EmptySelfHostedLabel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eu_regions_are_accepted() {
        for region in ALLOWED_HETZNER_REGIONS {
            assert_eq!(
                Residency::parse(region),
                Ok(Residency::HetznerRegion(region.to_owned()))
            );
        }
    }

    #[test]
    fn non_eu_regions_are_rejected() {
        assert!(matches!(
            Residency::parse("ash"), // Hetzner Ashburn, VA — US, never allowed
            Err(ResidencyError::DisallowedRegion(_))
        ));
        assert!(matches!(
            Residency::parse("us-east-1"),
            Err(ResidencyError::DisallowedRegion(_))
        ));
    }

    #[test]
    fn self_hosted_round_trips() {
        let residency = Residency::parse("self-hosted:on-prem-frankfurt").unwrap();
        assert_eq!(residency.as_config_value(), "self-hosted:on-prem-frankfurt");
        assert!(matches!(
            Residency::parse("self-hosted:"),
            Err(ResidencyError::EmptySelfHostedLabel)
        ));
    }
}
