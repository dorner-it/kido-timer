// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Judging posts (Wertungsrichter-Stationen). Keys follow the FBL
//! Wertungsbögen. Pre-2027 keys stay readable via serde aliases.

use serde::{Deserialize, Serialize};

use crate::discipline::TimerMode;

/// Canonical wire values (snake_case):
/// `startlinie`, `verteilerlinie`, `strahlrohrlinie`, `knotengestell`,
/// `angriffslinie`, `podest`.
///
/// Legacy aliases that must still deserialize:
/// `start` → Startlinie, `verteiler` → Verteilerlinie,
/// `strahlrohr` → Strahlrohrlinie, `knoten` → Knotengestell,
/// `kuebelspritze` → Angriffslinie. (`strahlrohrlinie` and `podest` keep
/// their names.) Serialization always emits the canonical value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationKind {
    /// WR 1 — Startlinie/Zeitnehmer. Also records the lineup per run.
    #[serde(alias = "start")]
    Startlinie,
    #[serde(alias = "verteiler")]
    Verteilerlinie,
    #[serde(alias = "strahlrohr")]
    Strahlrohrlinie,
    #[serde(alias = "knoten")]
    Knotengestell,
    #[serde(alias = "kuebelspritze")]
    Angriffslinie,
    /// Löschangriff only: Podest/Wasserbehälter.
    Podest,
}

impl StationKind {
    pub const ALL: [StationKind; 6] = [
        StationKind::Startlinie,
        StationKind::Verteilerlinie,
        StationKind::Strahlrohrlinie,
        StationKind::Knotengestell,
        StationKind::Angriffslinie,
        StationKind::Podest,
    ];

    /// Canonical wire key, e.g. `"knotengestell"`.
    pub fn as_str(self) -> &'static str {
        match self {
            StationKind::Startlinie => "startlinie",
            StationKind::Verteilerlinie => "verteilerlinie",
            StationKind::Strahlrohrlinie => "strahlrohrlinie",
            StationKind::Knotengestell => "knotengestell",
            StationKind::Angriffslinie => "angriffslinie",
            StationKind::Podest => "podest",
        }
    }

    /// German display label as on the Wertungsbögen, e.g.
    /// `"Startlinie/Zeitnehmer"`, `"Knotengestell"`, `"Podest/Wasserbehälter"`.
    pub fn label_de(self) -> &'static str {
        match self {
            StationKind::Startlinie => "Startlinie/Zeitnehmer",
            StationKind::Verteilerlinie => "Verteilerlinie",
            StationKind::Strahlrohrlinie => "Strahlrohrlinie",
            StationKind::Knotengestell => "Knotengestell",
            StationKind::Angriffslinie => "Angriffslinie",
            StationKind::Podest => "Podest/Wasserbehälter",
        }
    }

    /// Parse canonical keys **and** legacy aliases. `None` if unknown.
    pub fn parse(key: &str) -> Option<StationKind> {
        match key {
            "startlinie" | "start" => Some(StationKind::Startlinie),
            "verteilerlinie" | "verteiler" => Some(StationKind::Verteilerlinie),
            "strahlrohrlinie" | "strahlrohr" => Some(StationKind::Strahlrohrlinie),
            "knotengestell" | "knoten" => Some(StationKind::Knotengestell),
            "angriffslinie" | "kuebelspritze" => Some(StationKind::Angriffslinie),
            "podest" => Some(StationKind::Podest),
            _ => None,
        }
    }

    /// Stations staffed in the given discipline, in canonical order.
    /// Gruppenstaffette: Startlinie, Verteilerlinie, Strahlrohrlinie,
    /// Knotengestell, Angriffslinie. Loeschangriff: Podest, Angriffslinie.
    pub fn for_mode(mode: TimerMode) -> &'static [StationKind] {
        match mode {
            TimerMode::Gruppenstaffette => &[
                StationKind::Startlinie,
                StationKind::Verteilerlinie,
                StationKind::Strahlrohrlinie,
                StationKind::Knotengestell,
                StationKind::Angriffslinie,
            ],
            TimerMode::Loeschangriff => &[StationKind::Podest, StationKind::Angriffslinie],
        }
    }

    /// Whether this station is staffed in `mode` (see [`Self::for_mode`]).
    pub fn allowed_for(self, mode: TimerMode) -> bool {
        Self::for_mode(mode).contains(&self)
    }

    /// Only the Startlinie judge confirms the lineup of a run.
    pub fn records_lineup(self) -> bool {
        matches!(self, StationKind::Startlinie)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_wire_values_roundtrip() {
        for s in StationKind::ALL {
            let json = serde_json::to_string(&s).unwrap();
            assert_eq!(json, format!("\"{}\"", s.as_str()));
            let back: StationKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, s);
        }
        let keys: Vec<&str> = StationKind::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            keys,
            [
                "startlinie",
                "verteilerlinie",
                "strahlrohrlinie",
                "knotengestell",
                "angriffslinie",
                "podest"
            ]
        );
    }

    #[test]
    fn legacy_aliases_deserialize_and_serialize_canonically() {
        let cases = [
            ("start", StationKind::Startlinie, "startlinie"),
            ("verteiler", StationKind::Verteilerlinie, "verteilerlinie"),
            (
                "strahlrohr",
                StationKind::Strahlrohrlinie,
                "strahlrohrlinie",
            ),
            ("knoten", StationKind::Knotengestell, "knotengestell"),
            ("kuebelspritze", StationKind::Angriffslinie, "angriffslinie"),
        ];
        for (legacy, kind, canonical) in cases {
            let parsed: StationKind = serde_json::from_str(&format!("\"{legacy}\"")).unwrap();
            assert_eq!(parsed, kind, "{legacy}");
            assert_eq!(
                serde_json::to_string(&parsed).unwrap(),
                format!("\"{canonical}\"")
            );
            assert_eq!(StationKind::parse(legacy), Some(kind));
        }
    }

    #[test]
    fn unknown_station_rejected() {
        assert!(serde_json::from_str::<StationKind>("\"wasserbehaelter\"").is_err());
        assert!(serde_json::from_str::<StationKind>("\"Startlinie\"").is_err());
        assert_eq!(StationKind::parse("wasserbehaelter"), None);
        assert_eq!(StationKind::parse(""), None);
        assert_eq!(StationKind::parse("Knotengestell"), None);
    }

    #[test]
    fn parse_canonical_keys() {
        for s in StationKind::ALL {
            assert_eq!(StationKind::parse(s.as_str()), Some(s));
        }
    }

    #[test]
    fn labels() {
        assert_eq!(StationKind::Startlinie.label_de(), "Startlinie/Zeitnehmer");
        assert_eq!(StationKind::Verteilerlinie.label_de(), "Verteilerlinie");
        assert_eq!(StationKind::Strahlrohrlinie.label_de(), "Strahlrohrlinie");
        assert_eq!(StationKind::Knotengestell.label_de(), "Knotengestell");
        assert_eq!(StationKind::Angriffslinie.label_de(), "Angriffslinie");
        assert_eq!(StationKind::Podest.label_de(), "Podest/Wasserbehälter");
    }

    #[test]
    fn stations_for_mode() {
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
    }

    #[test]
    fn allowed_for() {
        let gs = TimerMode::Gruppenstaffette;
        let la = TimerMode::Loeschangriff;
        assert!(StationKind::Startlinie.allowed_for(gs));
        assert!(StationKind::Verteilerlinie.allowed_for(gs));
        assert!(StationKind::Strahlrohrlinie.allowed_for(gs));
        assert!(StationKind::Knotengestell.allowed_for(gs));
        assert!(StationKind::Angriffslinie.allowed_for(gs));
        assert!(!StationKind::Podest.allowed_for(gs));

        assert!(StationKind::Podest.allowed_for(la));
        assert!(StationKind::Angriffslinie.allowed_for(la));
        assert!(!StationKind::Startlinie.allowed_for(la));
        assert!(!StationKind::Verteilerlinie.allowed_for(la));
        assert!(!StationKind::Strahlrohrlinie.allowed_for(la));
        assert!(!StationKind::Knotengestell.allowed_for(la));
    }

    #[test]
    fn only_startlinie_records_lineup() {
        for s in StationKind::ALL {
            assert_eq!(s.records_lineup(), s == StationKind::Startlinie, "{s:?}");
        }
    }
}
