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
        todo!()
    }

    /// German display label as on the Wertungsbögen, e.g.
    /// `"Startlinie/Zeitnehmer"`, `"Knotengestell"`, `"Podest/Wasserbehälter"`.
    pub fn label_de(self) -> &'static str {
        todo!()
    }

    /// Parse canonical keys **and** legacy aliases. `None` if unknown.
    pub fn parse(key: &str) -> Option<StationKind> {
        todo!()
    }

    /// Stations staffed in the given discipline, in canonical order.
    /// Gruppenstaffette: Startlinie, Verteilerlinie, Strahlrohrlinie,
    /// Knotengestell, Angriffslinie. Loeschangriff: Podest, Angriffslinie.
    pub fn for_mode(mode: TimerMode) -> &'static [StationKind] {
        todo!()
    }

    pub fn allowed_for(self, mode: TimerMode) -> bool {
        todo!()
    }

    /// Only the Startlinie judge confirms the lineup of a run.
    pub fn records_lineup(self) -> bool {
        todo!()
    }
}
