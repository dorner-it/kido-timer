// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use serde::{Deserialize, Serialize};

/// Timer modes for fire-department running disciplines.
///
/// Both the web app and the standalone desktop app mirror this list verbatim.
/// Adding a variant requires a coordinated release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerMode {
    SingleLane,
    TwoLaneParallel,
    Relay,
    Individual,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&TimerMode::TwoLaneParallel).unwrap(),
            "\"two_lane_parallel\""
        );
    }

    #[test]
    fn round_trip() {
        for mode in [
            TimerMode::SingleLane,
            TimerMode::TwoLaneParallel,
            TimerMode::Relay,
            TimerMode::Individual,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let parsed: TimerMode = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, mode);
        }
    }
}
