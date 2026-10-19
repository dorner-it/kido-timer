// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Fixed error catalogue per station, transcribed from the FBL
//! Gruppenstafette Wertungsbögen (2027). SZ vs FV comes from which box
//! (SZ or FV column) is framed on the sheet — confirmed by the user on
//! 2026-10-16.
//!
//! Codes are stable wire identifiers `"<station>.<slug>"`; never rename
//! a code once shipped (old penalties reference it). Order = order on
//! the sheet.

use crate::penalty::PenaltyKind;
use crate::station::StationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogueEntry {
    pub code: &'static str,
    pub station: StationKind,
    pub kind: PenaltyKind,
    /// Text exactly as on the Wertungsbogen (German).
    pub label_de: &'static str,
}

/// Full Gruppenstafette catalogue (30 entries = 10 + 4 + 6 + 5 + 5; an
/// earlier draft of this comment said 29, the enumerated list is
/// authoritative): 24 SZ, 6 FV.
///
/// Startlinie/Zeitnehmer (10):
///  1. SZ  Kontrolle Bekleidung mit Mängel (außer WK4 alles mit Handschuhe)
///  2. SZ  Ein WK erreicht Ziel oder Übergabe nicht mit vollständiger Ausrüstung
///  3. SZ  WK 3 hält B-Schlauch nicht fest
///  4. SZ  Geräte werden vor dem Abschlagen aufgenommen, außer WK 5
///  5. SZ  WK 2 betritt Decke nicht
///  6. SZ  WK 4 legt Ausrüstung nicht neben sich ab und liegt nicht ausgestreckt
///  7. SZ  Ein WK startet vor dem Abschlagen
///  8. SZ  WK 5 durchläuft Ziellinie nicht
///  9. FV  Schläuche zu kurz, B-Schlauch liegt nicht an der Linie
/// 10. FV  Anderer WK als WK 5 läuft durchs Ziel
///
/// Verteilerlinie (4):
///  1. SZ  WK 1 kuppelt nicht mit WK 2 B-Schlauch an Verteiler
///  2. SZ  Verteiler liegt nicht an/auf der Linie
///  3. SZ  B oder C-Kupplung geht wieder auf, wird aber von anderen WK geschlossen
///  4. FV  Kupplung zum Wettkampfende offen
///
/// Strahlrohrlinie (6):
///  1. SZ  WK 2 kuppelt nicht mit WK 3
///  2. SZ  Ein WK greift beim Kuppeln die Kupplung des anderen WK
///  3. SZ  C-Kupplung geht wieder auf, wird aber von anderen WK geschlossen
///  4. SZ  WK 2 läuft nicht zur Hürde
///  5. SZ  WK 3 übergibt Strahlrohr nicht in die Hände von WK 6
///  6. FV  Kupplung zum Wettkampfende offen
///
/// Knotengestell (5):
///  1. SZ  WK 2 durchkriecht die Hürde nicht
///  2. SZ  Eine umgefallene Hürde wird nicht aufgerichtet
///  3. SZ  WK 4 bindet Knoten ohne vollständig angelegter Ausrüstung
///  4. SZ  1 Knoten falsch
///  5. FV  Mehrere Knoten falsch
///
/// Angriffslinie (5):
///  1. SZ  WK 6 umrundet Kübelspritze nicht
///  2. SZ  WK 5 kuppelt nicht an Kübelspritze
///  3. SZ  WK 5 tritt an der Angriffslinie über
///  4. SZ  Strahlrohr vor Eintreffen angekuppelt
///  5. FV  Nicht bekämpfte Ziele
pub const CATALOGUE: &[CatalogueEntry] = &[
    // Startlinie
    CatalogueEntry {
        code: "startlinie.bekleidung_maengel",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Sz,
        label_de: "Kontrolle Bekleidung mit Mängel (außer WK4 alles mit Handschuhe)",
    },
    CatalogueEntry {
        code: "startlinie.ausruestung_unvollstaendig",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Sz,
        label_de: "Ein WK erreicht Ziel oder Übergabe nicht mit vollständiger Ausrüstung",
    },
    CatalogueEntry {
        code: "startlinie.wk3_b_schlauch_nicht_festgehalten",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 3 hält B-Schlauch nicht fest",
    },
    CatalogueEntry {
        code: "startlinie.geraete_vor_abschlagen_aufgenommen",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Sz,
        label_de: "Geräte werden vor dem Abschlagen aufgenommen, außer WK 5",
    },
    CatalogueEntry {
        code: "startlinie.wk2_decke_nicht_betreten",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 2 betritt Decke nicht",
    },
    CatalogueEntry {
        code: "startlinie.wk4_ausruestung_nicht_abgelegt",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 4 legt Ausrüstung nicht neben sich ab und liegt nicht ausgestreckt",
    },
    CatalogueEntry {
        code: "startlinie.fruehstart",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Sz,
        label_de: "Ein WK startet vor dem Abschlagen",
    },
    CatalogueEntry {
        code: "startlinie.wk5_ziellinie_nicht_durchlaufen",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 5 durchläuft Ziellinie nicht",
    },
    CatalogueEntry {
        code: "startlinie.schlaeuche_zu_kurz",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Fv,
        label_de: "Schläuche zu kurz, B-Schlauch liegt nicht an der Linie",
    },
    CatalogueEntry {
        code: "startlinie.falscher_wk_im_ziel",
        station: StationKind::Startlinie,
        kind: PenaltyKind::Fv,
        label_de: "Anderer WK als WK 5 läuft durchs Ziel",
    },
    // Verteilerlinie
    CatalogueEntry {
        code: "verteilerlinie.wk1_kuppelt_nicht_an_verteiler",
        station: StationKind::Verteilerlinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 1 kuppelt nicht mit WK 2 B-Schlauch an Verteiler",
    },
    CatalogueEntry {
        code: "verteilerlinie.verteiler_nicht_an_linie",
        station: StationKind::Verteilerlinie,
        kind: PenaltyKind::Sz,
        label_de: "Verteiler liegt nicht an/auf der Linie",
    },
    CatalogueEntry {
        code: "verteilerlinie.kupplung_geht_auf",
        station: StationKind::Verteilerlinie,
        kind: PenaltyKind::Sz,
        label_de: "B oder C-Kupplung geht wieder auf, wird aber von anderen WK geschlossen",
    },
    CatalogueEntry {
        code: "verteilerlinie.kupplung_offen_bei_ende",
        station: StationKind::Verteilerlinie,
        kind: PenaltyKind::Fv,
        label_de: "Kupplung zum Wettkampfende offen",
    },
    // Strahlrohrlinie
    CatalogueEntry {
        code: "strahlrohrlinie.wk2_kuppelt_nicht_mit_wk3",
        station: StationKind::Strahlrohrlinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 2 kuppelt nicht mit WK 3",
    },
    CatalogueEntry {
        code: "strahlrohrlinie.kupplung_des_anderen_gegriffen",
        station: StationKind::Strahlrohrlinie,
        kind: PenaltyKind::Sz,
        label_de: "Ein WK greift beim Kuppeln die Kupplung des anderen WK",
    },
    CatalogueEntry {
        code: "strahlrohrlinie.c_kupplung_geht_auf",
        station: StationKind::Strahlrohrlinie,
        kind: PenaltyKind::Sz,
        label_de: "C-Kupplung geht wieder auf, wird aber von anderen WK geschlossen",
    },
    CatalogueEntry {
        code: "strahlrohrlinie.wk2_laeuft_nicht_zur_huerde",
        station: StationKind::Strahlrohrlinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 2 läuft nicht zur Hürde",
    },
    CatalogueEntry {
        code: "strahlrohrlinie.strahlrohr_nicht_uebergeben",
        station: StationKind::Strahlrohrlinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 3 übergibt Strahlrohr nicht in die Hände von WK 6",
    },
    CatalogueEntry {
        code: "strahlrohrlinie.kupplung_offen_bei_ende",
        station: StationKind::Strahlrohrlinie,
        kind: PenaltyKind::Fv,
        label_de: "Kupplung zum Wettkampfende offen",
    },
    // Knotengestell
    CatalogueEntry {
        code: "knotengestell.wk2_huerde_nicht_durchkrochen",
        station: StationKind::Knotengestell,
        kind: PenaltyKind::Sz,
        label_de: "WK 2 durchkriecht die Hürde nicht",
    },
    CatalogueEntry {
        code: "knotengestell.huerde_nicht_aufgerichtet",
        station: StationKind::Knotengestell,
        kind: PenaltyKind::Sz,
        label_de: "Eine umgefallene Hürde wird nicht aufgerichtet",
    },
    CatalogueEntry {
        code: "knotengestell.knoten_ohne_ausruestung",
        station: StationKind::Knotengestell,
        kind: PenaltyKind::Sz,
        label_de: "WK 4 bindet Knoten ohne vollständig angelegter Ausrüstung",
    },
    CatalogueEntry {
        code: "knotengestell.ein_knoten_falsch",
        station: StationKind::Knotengestell,
        kind: PenaltyKind::Sz,
        label_de: "1 Knoten falsch",
    },
    CatalogueEntry {
        code: "knotengestell.mehrere_knoten_falsch",
        station: StationKind::Knotengestell,
        kind: PenaltyKind::Fv,
        label_de: "Mehrere Knoten falsch",
    },
    // Angriffslinie
    CatalogueEntry {
        code: "angriffslinie.wk6_kuebelspritze_nicht_umrundet",
        station: StationKind::Angriffslinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 6 umrundet Kübelspritze nicht",
    },
    CatalogueEntry {
        code: "angriffslinie.wk5_kuppelt_nicht_an_kuebelspritze",
        station: StationKind::Angriffslinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 5 kuppelt nicht an Kübelspritze",
    },
    CatalogueEntry {
        code: "angriffslinie.wk5_tritt_ueber",
        station: StationKind::Angriffslinie,
        kind: PenaltyKind::Sz,
        label_de: "WK 5 tritt an der Angriffslinie über",
    },
    CatalogueEntry {
        code: "angriffslinie.strahlrohr_vor_eintreffen_angekuppelt",
        station: StationKind::Angriffslinie,
        kind: PenaltyKind::Sz,
        label_de: "Strahlrohr vor Eintreffen angekuppelt",
    },
    CatalogueEntry {
        code: "angriffslinie.ziele_nicht_bekaempft",
        station: StationKind::Angriffslinie,
        kind: PenaltyKind::Fv,
        label_de: "Nicht bekämpfte Ziele",
    },
];

/// Look up an entry by code.
pub fn lookup(code: &str) -> Option<&'static CatalogueEntry> {
    CATALOGUE.iter().find(|e| e.code == code)
}

/// Look up an entry by code and require it to belong to `station`
/// (station submit validation). `None` if unknown or on another station.
pub fn lookup_for_station(code: &str, station: StationKind) -> Option<&'static CatalogueEntry> {
    lookup(code).filter(|e| e.station == station)
}

/// Entries of one station, in sheet order.
pub fn for_station(station: StationKind) -> impl Iterator<Item = &'static CatalogueEntry> {
    CATALOGUE.iter().filter(move |e| e.station == station)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn exactly_30_entries() {
        // 10 + 4 + 6 + 5 + 5 as enumerated on the Wertungsbögen.
        assert_eq!(CATALOGUE.len(), 30);
    }

    #[test]
    fn per_station_counts() {
        let count = |s| for_station(s).count();
        assert_eq!(count(StationKind::Startlinie), 10);
        assert_eq!(count(StationKind::Verteilerlinie), 4);
        assert_eq!(count(StationKind::Strahlrohrlinie), 6);
        assert_eq!(count(StationKind::Knotengestell), 5);
        assert_eq!(count(StationKind::Angriffslinie), 5);
        assert_eq!(count(StationKind::Podest), 0);
    }

    #[test]
    fn codes_unique_prefixed_and_ascii_snake() {
        let mut seen = HashSet::new();
        for e in CATALOGUE {
            assert!(seen.insert(e.code), "duplicate code {}", e.code);
            let (prefix, slug) = e.code.split_once('.').expect("code has a dot");
            assert_eq!(prefix, e.station.as_str(), "{}", e.code);
            assert!(!slug.is_empty());
            assert!(!slug.contains('.'), "{}", e.code);
            assert!(
                slug.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "{} is not ascii snake",
                e.code
            );
            assert!(!slug.starts_with('_') && !slug.ends_with('_') && !slug.contains("__"));
            assert!(!e.label_de.is_empty());
        }
    }

    #[test]
    fn stations_are_gruppenstafette_stations_and_grouped_in_order() {
        let order: Vec<StationKind> = CATALOGUE.iter().map(|e| e.station).collect();
        let mut sorted = order.clone();
        sorted.sort(); // StationKind derives Ord in canonical order
        assert_eq!(
            order, sorted,
            "entries grouped by station in canonical order"
        );
        assert!(
            CATALOGUE
                .iter()
                .all(|e| e.station.allowed_for(crate::TimerMode::Gruppenstaffette))
        );
    }

    #[test]
    fn fv_set_is_exact() {
        let fv: Vec<(StationKind, &str)> = CATALOGUE
            .iter()
            .filter(|e| e.kind == PenaltyKind::Fv)
            .map(|e| (e.station, e.label_de))
            .collect();
        assert_eq!(
            fv,
            vec![
                (
                    StationKind::Startlinie,
                    "Schläuche zu kurz, B-Schlauch liegt nicht an der Linie"
                ),
                (
                    StationKind::Startlinie,
                    "Anderer WK als WK 5 läuft durchs Ziel"
                ),
                (
                    StationKind::Verteilerlinie,
                    "Kupplung zum Wettkampfende offen"
                ),
                (
                    StationKind::Strahlrohrlinie,
                    "Kupplung zum Wettkampfende offen"
                ),
                (StationKind::Knotengestell, "Mehrere Knoten falsch"),
                (StationKind::Angriffslinie, "Nicht bekämpfte Ziele"),
            ]
        );
        assert_eq!(
            CATALOGUE
                .iter()
                .filter(|e| e.kind == PenaltyKind::Sz)
                .count(),
            24
        );
    }

    #[test]
    fn known_codes() {
        let cases = [
            ("startlinie.bekleidung_maengel", PenaltyKind::Sz),
            ("knotengestell.ein_knoten_falsch", PenaltyKind::Sz),
            ("knotengestell.mehrere_knoten_falsch", PenaltyKind::Fv),
            ("angriffslinie.ziele_nicht_bekaempft", PenaltyKind::Fv),
        ];
        for (code, kind) in cases {
            let e = lookup(code).unwrap_or_else(|| panic!("{code} missing"));
            assert_eq!(e.code, code);
            assert_eq!(e.kind, kind);
        }
    }

    #[test]
    fn lookup_hit_and_miss() {
        let e = lookup("knotengestell.ein_knoten_falsch").unwrap();
        assert_eq!(e.station, StationKind::Knotengestell);
        assert_eq!(e.label_de, "1 Knoten falsch");
        assert!(lookup("knotengestell.unknown").is_none());
        assert!(lookup("").is_none());
        assert!(lookup("ein_knoten_falsch").is_none());
        assert!(lookup("KNOTENGESTELL.EIN_KNOTEN_FALSCH").is_none());
        for e in CATALOGUE {
            assert_eq!(lookup(e.code), Some(e));
        }
    }

    #[test]
    fn lookup_for_station_checks_station() {
        let code = "knotengestell.ein_knoten_falsch";
        assert!(lookup_for_station(code, StationKind::Knotengestell).is_some());
        assert!(lookup_for_station(code, StationKind::Startlinie).is_none());
        assert!(lookup_for_station("nope", StationKind::Knotengestell).is_none());
    }

    #[test]
    fn for_station_keeps_sheet_order() {
        let labels: Vec<&str> = for_station(StationKind::Knotengestell)
            .map(|e| e.label_de)
            .collect();
        assert_eq!(
            labels,
            [
                "WK 2 durchkriecht die Hürde nicht",
                "Eine umgefallene Hürde wird nicht aufgerichtet",
                "WK 4 bindet Knoten ohne vollständig angelegter Ausrüstung",
                "1 Knoten falsch",
                "Mehrere Knoten falsch",
            ]
        );
        let start: Vec<&str> = for_station(StationKind::Startlinie)
            .map(|e| e.label_de)
            .collect();
        assert_eq!(
            start.first(),
            Some(&"Kontrolle Bekleidung mit Mängel (außer WK4 alles mit Handschuhe)")
        );
        assert_eq!(start.last(), Some(&"Anderer WK als WK 5 läuft durchs Ziel"));
        let angriff: Vec<&str> = for_station(StationKind::Angriffslinie)
            .map(|e| e.code)
            .collect();
        assert_eq!(angriff.last(), Some(&"angriffslinie.ziele_nicht_bekaempft"));
    }

    #[test]
    fn fv_entries_are_last_per_station() {
        // On every sheet the FV boxes come after the SZ boxes.
        for &s in StationKind::for_mode(crate::TimerMode::Gruppenstaffette) {
            let kinds: Vec<PenaltyKind> = for_station(s).map(|e| e.kind).collect();
            let first_fv = kinds.iter().position(|k| *k == PenaltyKind::Fv).unwrap();
            assert!(kinds[first_fv..].iter().all(|k| *k == PenaltyKind::Fv));
        }
    }
}
