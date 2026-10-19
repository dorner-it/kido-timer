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

/// Full Gruppenstafette catalogue (29 entries):
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
pub const CATALOGUE: &[CatalogueEntry] = &[];

/// Look up an entry by code.
pub fn lookup(code: &str) -> Option<&'static CatalogueEntry> {
    todo!()
}

/// Entries of one station, in sheet order.
pub fn for_station(station: StationKind) -> impl Iterator<Item = &'static CatalogueEntry> {
    CATALOGUE.iter().filter(move |e| e.station == station)
}
