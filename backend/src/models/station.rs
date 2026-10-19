// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Judging posts (Wertungsrichter-Stationen). The type lives in the
//! shared `kido-core` crate; this module re-exports it so existing call
//! sites keep compiling.
//!
//! Canonical wire keys follow the FBL Wertungsbögen: `startlinie`,
//! `verteilerlinie`, `strahlrohrlinie`, `knotengestell`,
//! `angriffslinie`, `podest`. The pre-2027 keys `start`, `verteiler`,
//! `strahlrohr`, `knoten`, `kuebelspritze` are still accepted on input
//! (and therefore on legacy DynamoDB rows); output is always canonical.

pub use kido_core::station::StationKind;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::timer_mode::TimerMode;

    #[test]
    fn serializes_canonical_keys() {
        let expected = [
            (StationKind::Startlinie, "startlinie"),
            (StationKind::Verteilerlinie, "verteilerlinie"),
            (StationKind::Strahlrohrlinie, "strahlrohrlinie"),
            (StationKind::Knotengestell, "knotengestell"),
            (StationKind::Angriffslinie, "angriffslinie"),
            (StationKind::Podest, "podest"),
        ];
        for (s, key) in expected {
            assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{key}\""));
        }
    }

    #[test]
    fn legacy_keys_deserialize_to_canonical() {
        let cases = [
            ("start", StationKind::Startlinie),
            ("verteiler", StationKind::Verteilerlinie),
            ("strahlrohr", StationKind::Strahlrohrlinie),
            ("knoten", StationKind::Knotengestell),
            ("kuebelspritze", StationKind::Angriffslinie),
            ("strahlrohrlinie", StationKind::Strahlrohrlinie),
            ("podest", StationKind::Podest),
        ];
        for (legacy, canonical) in cases {
            let parsed: StationKind = serde_json::from_str(&format!("\"{legacy}\"")).unwrap();
            assert_eq!(parsed, canonical, "{legacy}");
            // Re-serialising emits the canonical key, never the alias.
            let out = serde_json::to_value(parsed).unwrap();
            if !matches!(legacy, "strahlrohrlinie" | "podest") {
                assert_ne!(
                    out,
                    serde_json::json!(legacy),
                    "{legacy} must not round-trip"
                );
            }
        }
    }

    #[test]
    fn unknown_key_rejected() {
        assert!(serde_json::from_str::<StationKind>("\"zielrichter\"").is_err());
    }

    #[test]
    fn round_trip_all_variants() {
        for s in StationKind::ALL {
            let json = serde_json::to_string(&s).unwrap();
            let back: StationKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, s);
        }
    }

    // --- behaviour owned by kido-core (panics with `todo!()` until the
    // kido-core bodies are merged) ---

    #[test]
    fn stations_per_mode() {
        assert_eq!(
            StationKind::for_mode(TimerMode::Gruppenstaffette),
            &[
                StationKind::Startlinie,
                StationKind::Verteilerlinie,
                StationKind::Strahlrohrlinie,
                StationKind::Knotengestell,
                StationKind::Angriffslinie,
            ]
        );
        assert_eq!(
            StationKind::for_mode(TimerMode::Loeschangriff),
            &[StationKind::Podest, StationKind::Angriffslinie]
        );
        assert!(!StationKind::Podest.allowed_for(TimerMode::Gruppenstaffette));
        assert!(!StationKind::Knotengestell.allowed_for(TimerMode::Loeschangriff));
        assert!(StationKind::Angriffslinie.allowed_for(TimerMode::Loeschangriff));
    }

    #[test]
    fn only_startlinie_records_lineup() {
        for s in StationKind::ALL {
            assert_eq!(s.records_lineup(), s == StationKind::Startlinie, "{s:?}");
        }
    }

    #[test]
    fn parse_accepts_legacy_and_canonical() {
        assert_eq!(
            StationKind::parse("knoten"),
            Some(StationKind::Knotengestell)
        );
        assert_eq!(
            StationKind::parse("knotengestell"),
            Some(StationKind::Knotengestell)
        );
        assert_eq!(StationKind::parse("nope"), None);
        assert_eq!(StationKind::Knotengestell.as_str(), "knotengestell");
    }
}
