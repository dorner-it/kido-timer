# Concept — Gruppenstafette 2027

Status: **agreed (2026-10-10), decisions finalized 2026-10-16** — implementation in progress
(monorepo `dorner-it/kido-timer`, one PR per step).
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
| Startlinie | Bekleidung mit Mängeln; WK erreicht Ziel/Übergabe nicht mit vollständiger Ausrüstung; WK3 hält B-Schlauch nicht fest; Geräte vor dem Abschlagen aufgenommen (außer WK5); WK2 betritt Decke nicht; WK4 legt Ausrüstung nicht ab / liegt nicht ausgestreckt; WK startet vor dem Abschlagen; WK5 durchläuft Ziellinie nicht; **FV:** Schläuche zu kurz / B-Schlauch nicht an der Linie; **FV:** anderer WK als WK5 läuft durchs Ziel |
| Verteilerlinie | WK1 kuppelt nicht mit WK2 B-Schlauch an Verteiler; Verteiler nicht an/auf der Linie; B-/C-Kupplung geht auf, von anderem WK geschlossen; Kupplung zum Wettkampfende offen |
| Strahlrohrlinie | WK2 kuppelt nicht mit WK3; WK greift Kupplung des anderen WK; C-Kupplung geht auf, von anderem WK geschlossen; WK2 läuft nicht zur Hürde; WK3 übergibt Strahlrohr nicht in die Hände von WK6; Kupplung zum Wettkampfende offen |
| Knotengestell | WK2 durchkriecht Hürde nicht; umgefallene Hürde nicht aufgerichtet; WK4 bindet Knoten ohne vollständige Ausrüstung; 1 Knoten falsch; mehrere Knoten falsch |
| Angriffslinie | WK6 umrundet Kübelspritze nicht; WK5 kuppelt nicht an Kübelspritze; WK5 tritt an der Angriffslinie über; Strahlrohr vor Eintreffen angekuppelt; nicht bekämpfte Ziele |

- SZ vs FV per error: the xlsx files carry no colours; on every sheet
  exactly one of the two boxes (SZ or FV column) is **framed** per error
  row. Result (confirmed by the user 2026-10-16): FV = Startlinie
  "Schläuche zu kurz…" and "Anderer WK als WK 5 läuft durchs Ziel";
  Verteilerlinie and Strahlrohrlinie "Kupplung zum Wettkampfende offen";
  Knotengestell "Mehrere Knoten falsch"; Angriffslinie "Nicht bekämpfte
  Ziele" (matches Mirko, Q11). All others SZ. 29 errors in total; the
  catalogue lives in `crates/kido-core/src/catalogue.rs`.
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
- Lanes: 1 or 2, set per discipline (`Discipline.lane_count`, default 2;
  Löschangriff fixed 4). Mirko didn't answer Q17 — the system works with
  either (user decision 2026-10-16). With 2 lanes two runs are current at
  the same time (`Event.current_run_ids`).
- Judges with 2 lanes: configurable per discipline (`judge_mode`): one
  judge per station who picks the lane for each error, or one judge per
  station and lane.

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
- **Decided 2026-10-16: variant (a)** — the run with the best time, with
  that run's lineup (same run as in the WG ranking).

New endpoint `GET /api/disciplines/:id/results` → WG rankings + Tagessieger.
Frontend: result view + print view. The scoring code lives in the shared
crate `crates/kido-core` so backend and desktop compute identical results.

## 6. Desktop app

- Schema bump v3 → v4 (`Run` and `Penalty` change in export / `.kido`).
- Protocol view shows "ungültig" and "Handzeit".
- Results (WG ranking, Tagessieger) are also computed and printable in the
  desktop via `kido-core` (needed for on-prem).
- **Live vs. on-prem** (decided 2026-10-16): the web sets a default per
  discipline (`sync_mode`), the desktop can override it on the day.
  - *Live*: desktop talks to the cloud API, judges use the web app.
  - *On-prem*: the desktop starts a local HTTP server in the venue
    network. Judges' phones scan a QR code with the laptop address and get
    the **same judge UI** (frontend judge pages built statically and
    embedded in the desktop, API base switched). The organizer reviews and
    confirms errors, lineups and hand times in the desktop.
  - Before: load the discipline from the web (snapshot / `.kido`). After:
    upload button (API key) as soon as internet is back; `.kido` export as
    fallback.

## 7. Implementation order (one PR each)

0. Monorepo `dorner-it/kido-timer` (web + desktop, histories preserved),
   CI, desktop release from the monorepo (bridge release 0.5.4 in the old
   repo moves the updater).
1. `kido-core` crate (catalogue, WG check, timing, scoring) + backend model
   + migration: WG as input + warning check, `Penalty.kind`, station keys,
   `Run.lineup`, time source / hand times, lane count, judge mode, parallel
   runs; frontend adapted to the model.
2. Judge app: error catalogue per station, lane selection, lineup check at
   Start.
3. Organizer web UI: review (withdraw/confirm), hand-time entry, Umsetzen.
4. Results: endpoint, WG ranking, Tagessieger, print view.
5. Desktop: schema v4, `kido-core`, show invalid runs and hand times,
   results + print, live/on-prem override.
6. Desktop on-prem server: local HTTP API for judges, embedded judge UI,
   organizer review in the desktop, upload back to the web.

Out of scope: Löschangriff 2028 (7 WGs, 2 channels per lane, valid/invalid
only, two judges: Podest/Wasserbehälter + Angriffslinie), power outage during
Löschangriff, certificates.
