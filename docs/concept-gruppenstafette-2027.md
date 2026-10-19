# Concept — Gruppenstafette 2027

Status: **draft, agreed in principle (2026-10-10)** — implementation not started.
Sources: Mirko's (FBL) answers of 2026-10-10 to our 17 questions, the five
Gruppenstafette Wertungsbögen, "Tagessieger mit Formel Kenny" (Excel), and
the code state of `kido-timer-web` / `kido-timer-app` on 2026-10-10.

Context: only one competition per year, alternating — **2027 Gruppenstafette**,
2028 Löschangriff. Löschangriff is out of scope here.

## 0. Starting point

Already in place (web): Event → Discipline → TeamEntry, judge stations with a
single event token, bulk-approve, `TimerMode::Gruppenstaffette`.

Contradicts Mirko's answers and must be reshaped:

- `Wertungsgruppe::derive()` derives the WG from age + gender — Mirko: WG is
  **ticked by the club**, no gender field.
- `Runner.gender` is required on new runners.
- `Penalty.is_disqualification` models an invalid run as a 999 s time
  penalty — an invalid run is a status, not a time.

No ranking / Tagessieger logic exists in any repo yet.

## 1. Registration & Wertungsgruppe

- `TeamEntry.wg` becomes an **input** (taken from the registration form),
  required; WG1–WG5 for the Gruppenstafette.
- `derive()` is replaced by a **check that only warns**:
  - Age per runner = competition year − birth year (= age on 31.12., which
    is already the current logic).
  - WG1 = 8–10, WG2/3 = up to 14, WG4/5 = up to 18.
  - Runners that don't fit → red marker on the entry ("rotes Kreuz"),
    saving is not blocked.
- **"Umsetzen"** (moving a team to another WG) is a manual FBL action that
  changes the WG and logs who/when. Rule of thumb from Mirko: one runner of
  the other gender is tolerated after asking the other teams; from two on,
  the team moves to the age-appropriate mixed WG.
- `Runner.gender` becomes optional and is removed from the UI (field kept so
  existing rows still load).
- The whole roster incl. substitutes is entered (roster spreadsheet already
  supports that).

## 2. Lineup per run (WR 1 at the Startlinie)

- New: `Run.lineup: Vec<runner_id>` — the runners who actually run.
- Judge app, station **Start**: before each run a "Besetzung" view with the
  team's roster as a tick list + "Besetzung bestätigen" (Mirko's "X" idea).
- The organizer can correct the lineup in the web UI.
- Replaces the "new team when runners change" workaround.

## 3. Errors: SZ and FV

`Penalty.kind: Sz | Fv` replaces `is_disqualification`:

- **SZ** = fixed +15 s, any number, run stays valid.
- **FV** = run invalid, no time.
- Migration: `is_disqualification = true` → `Fv`.

Fixed error catalogue per station, transcribed from the Wertungsbögen:

| Station | Errors (shortened) |
|---|---|
| Startlinie | Bekleidung mit Mängeln; WK erreicht Ziel/Übergabe nicht mit vollständiger Ausrüstung; WK3 hält B-Schlauch nicht fest; Geräte vor dem Abschlagen aufgenommen (außer WK5); WK2 betritt Decke nicht; WK4 legt Ausrüstung nicht ab / liegt nicht ausgestreckt |
| Verteilerlinie | WK1 kuppelt nicht mit WK2 B-Schlauch an Verteiler; Verteiler nicht an/auf der Linie; B-/C-Kupplung geht auf, von anderem WK geschlossen; Kupplung zum Wettkampfende offen |
| Strahlrohrlinie | WK2 kuppelt nicht mit WK3; WK greift Kupplung des anderen WK; C-Kupplung geht auf, von anderem WK geschlossen; WK2 läuft nicht zur Hürde; WK3 übergibt Strahlrohr nicht in die Hände von WK6; Kupplung zum Wettkampfende offen |
| Knotengestell | WK2 durchkriecht Hürde nicht; umgefallene Hürde nicht aufgerichtet; WK4 bindet Knoten ohne vollständige Ausrüstung; 1 Knoten falsch; mehrere Knoten falsch |
| Angriffslinie | WK6 umrundet Kübelspritze nicht; WK5 kuppelt nicht an Kübelspritze; WK5 tritt an der Angriffslinie über; Strahlrohr vor Eintreffen angekuppelt; nicht bekämpfte Ziele |

- SZ vs FV per error comes from the **red boxes** on the sheets (cell
  formatting only) — to be read out during implementation and checked by
  the user. "Nicht bekämpfte Ziele" is always FV (Mirko, Q11).
- The judge taps the error; the station is recorded automatically. Free
  text no longer needed.

Station keys follow Mirko's sheets: `kuebelspritze` → `angriffslinie`;
Löschangriff (2028) only `podest` + `angriffslinie`. Old keys stay readable
via serde aliases. One judge per station.

Workflow (Mirko, Q10):

1. Judge raises the red flag and records the error → `Pending`.
2. FBL checks with the team leader → confirms (`Approved`) or withdraws
   (`Rejected`).
3. Withdrawn errors stay visible, crossed out.
4. **"Übernehmen"** (existing bulk-approve) finalizes the run; errors still
   pending at that point count as confirmed.

## 4. Timing

- New: `Run.time_source: Lichtschranke | Handstopp`, plus `hand_time_1_ms`
  and `hand_time_2_ms`.
- Hand times are only entered when the light barrier fails (e.g. power
  outage). Then `original_time_ms` = mean of the two, rounded to 1/100 s;
  the result list marks it "Handzeit".
- Lanes: 1 or 2, set per discipline (`lane_count()` stays the maximum,
  `TeamEntry.lane` stays optional). Mirko didn't answer Q17.

## 5. Scoring (backend)

Run result:

- valid = no confirmed FV
- time = measured time + 15 s × confirmed SZ

Ranking per WG:

- Best valid run per team counts.
- Ties: standard competition ranking **1, 1, 3**, compared at 1/100 s
  (matches `RANK.EQ` in Kenny's Excel).
- Teams with only invalid runs share **last place**, listed below the rest.
- Teams that didn't start: "n.a.", no place.

Tagessieger (across all WGs, one per competition):

- Age per runner = **completed years on the competition day**
  (`DATEDIF(…;TODAY();"Y")` in Kenny's Excel) — not the 31.12. age.
- Average age = mean over the run's lineup (actual runners only, Q6).
- Factor = 1 − 2·(18 − average age)/100.
- GW-Zeit = time × factor, ranked 1, 1, 3.
- Teams without a valid run are left out.
- **Open decision:** if a team's two runs have different lineups, which run
  counts?
  - (a) the run with the best time, with that run's lineup — matches
    Mirko's "Bestzeit × Faktor" and the WG ranking (**recommended**);
  - (b) the run with the best GW-Zeit — may differ from the WG ranking.

New endpoint `GET /api/disciplines/:id/results` → WG rankings + Tagessieger.
Frontend: result view + print view.

## 6. Desktop app

- Schema bump v3 → v4 (`Run` and `Penalty` change in export / `.kido`).
- Protocol view shows "ungültig" and "Handzeit".
- Ranking stays web-only.

## 7. Implementation order (one PR each)

1. Backend model + migration: WG as input + warning check, `Penalty.kind`,
   station keys, `Run.lineup`, time source / hand times.
2. Judge app: error catalogue per station, lineup check at Start.
3. Organizer web UI: review (withdraw/confirm), hand-time entry, Umsetzen.
4. Results: endpoint, WG ranking, Tagessieger, print view.
5. Desktop: schema v4, show invalid runs and hand times.

Out of scope: Löschangriff 2028 (7 WGs, 2 channels per lane, valid/invalid
only, two judges: Podest/Wasserbehälter + Angriffslinie), power outage during
Löschangriff, certificates.
