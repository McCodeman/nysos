// SPDX-FileCopyrightText: Copyright 2026 Marshall Cody McCain (mccodeman@proton.me)
// SPDX-License-Identifier: Apache-2.0

//! Explicit runtime gates with centrally defined defaults and user overrides.
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    /// Attached, scroll-synchronized logical line-number gutters (enabled by default)
    LineNumbers,
}

/// Default gates when a demo omits its `features` setting.
pub fn defaults() -> Vec<Feature> {
    vec![Feature::LineNumbers]
}

/// Startup overrides are merged into the editable demo; disabling wins.
pub fn apply_overrides(features: &mut Vec<Feature>, enable: &[Feature], disable: &[Feature]) {
    for feature in enable {
        if !features.contains(feature) {
            features.push(*feature);
        }
    }
    features.retain(|feature| !disable.contains(feature));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disable_wins_and_repeated_enables_are_idempotent() {
        let mut features = defaults();
        apply_overrides(&mut features, &[Feature::LineNumbers; 2], &[]);
        assert_eq!(features, [Feature::LineNumbers]);
        apply_overrides(
            &mut features,
            &[Feature::LineNumbers],
            &[Feature::LineNumbers],
        );
        assert!(features.is_empty());
    }
}
