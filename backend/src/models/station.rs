// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use serde::{Deserialize, Serialize};

use crate::models::timer_mode::TimerMode;

/// A judging post (Strafrichter station) along the track. A station is
/// the operator's seat for the day — one decider stays at "Knoten",
/// another at "Strahlrohr", and so on. The station they pick gets
/// recorded on every penalty so the organiser can see *where* a fault
/// was observed, not just *who* observed it.
///
/// Variants are kept loosely aligned with the German Feuerwehr-
/// Wettkampfordnung. Edit the list once the parallel branch finalises
/// the discipline / mode mapping; allow_for() is intentionally defensive
/// so the set continues to typecheck if `TimerMode` is rewritten later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationKind {
    Start,
    Verteiler,
    Strahlrohr,
    Knoten,
    Kuebelspritze,
    // Löschangriff-only:
    Podest,
    Strahlrohrlinie,
}

impl StationKind {
    pub const ALL: [StationKind; 7] = [
        StationKind::Start,
        StationKind::Verteiler,
        StationKind::Strahlrohr,
        StationKind::Knoten,
        StationKind::Kuebelspritze,
        StationKind::Podest,
        StationKind::Strahlrohrlinie,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            StationKind::Start => "start",
            StationKind::Verteiler => "verteiler",
            StationKind::Strahlrohr => "strahlrohr",
            StationKind::Knoten => "knoten",
            StationKind::Kuebelspritze => "kuebelspritze",
            StationKind::Podest => "podest",
            StationKind::Strahlrohrlinie => "strahlrohrlinie",
        }
    }

    /// Whether this station can be staffed under the given timer mode.
    ///
    /// Stations `Podest` and `Strahlrohrlinie` are Löschangriff-only and
    /// therefore restricted to `TwoLaneParallel`. Everything else is
    /// available in every mode.
    ///
    /// TODO: the parallel feature branch is rewriting `TimerMode`. Once
    /// it lands, revisit this mapping and add discipline-aware branches.
    /// Until then we keep a defensive fallback so the helper continues
    /// to compile even if new variants are added: any unknown mode gets
    /// the common (non-Löschangriff) set.
    pub fn allowed_for(self, mode: TimerMode) -> bool {
        match self {
            StationKind::Podest | StationKind::Strahlrohrlinie => {
                matches!(mode, TimerMode::TwoLaneParallel)
            }
            // Common stations: usable in every mode that currently exists.
            // The catch-all keeps this defensive against TimerMode growing.
            StationKind::Start
            | StationKind::Verteiler
            | StationKind::Strahlrohr
            | StationKind::Knoten
            | StationKind::Kuebelspritze => match mode {
                TimerMode::SingleLane
                | TimerMode::TwoLaneParallel
                | TimerMode::Relay
                | TimerMode::Individual => true,
            },
        }
    }

    /// Convenience list of every station valid for the given mode, in
    /// the canonical order used on the picker.
    pub fn for_mode(mode: TimerMode) -> Vec<StationKind> {
        StationKind::ALL
            .iter()
            .copied()
            .filter(|s| s.allowed_for(mode))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&StationKind::Kuebelspritze).unwrap(),
            "\"kuebelspritze\""
        );
        assert_eq!(
            serde_json::to_string(&StationKind::Strahlrohrlinie).unwrap(),
            "\"strahlrohrlinie\""
        );
    }

    #[test]
    fn round_trip_all_variants() {
        for s in StationKind::ALL {
            let json = serde_json::to_string(&s).unwrap();
            let back: StationKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, s);
        }
    }

    #[test]
    fn loeschangriff_only_stations_are_two_lane() {
        assert!(StationKind::Podest.allowed_for(TimerMode::TwoLaneParallel));
        assert!(!StationKind::Podest.allowed_for(TimerMode::SingleLane));
        assert!(!StationKind::Podest.allowed_for(TimerMode::Relay));
        assert!(!StationKind::Podest.allowed_for(TimerMode::Individual));

        assert!(StationKind::Strahlrohrlinie.allowed_for(TimerMode::TwoLaneParallel));
        assert!(!StationKind::Strahlrohrlinie.allowed_for(TimerMode::SingleLane));
    }

    #[test]
    fn common_stations_are_universal() {
        for mode in [
            TimerMode::SingleLane,
            TimerMode::TwoLaneParallel,
            TimerMode::Relay,
            TimerMode::Individual,
        ] {
            for s in [
                StationKind::Start,
                StationKind::Verteiler,
                StationKind::Strahlrohr,
                StationKind::Knoten,
                StationKind::Kuebelspritze,
            ] {
                assert!(s.allowed_for(mode), "{s:?} should be allowed for {mode:?}");
            }
        }
    }

    #[test]
    fn for_mode_returns_seven_for_loeschangriff_five_otherwise() {
        assert_eq!(StationKind::for_mode(TimerMode::TwoLaneParallel).len(), 7);
        assert_eq!(StationKind::for_mode(TimerMode::SingleLane).len(), 5);
        assert_eq!(StationKind::for_mode(TimerMode::Relay).len(), 5);
        assert_eq!(StationKind::for_mode(TimerMode::Individual).len(), 5);
    }
}
