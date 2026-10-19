# kido-core

Shared domain logic for KI-DO Timer: used by the web backend and the
desktop app so live (cloud) and on-prem (desktop as local server) produce
identical results. Pure logic only: no I/O, no async, no storage.

Spec: `docs/concept-gruppenstafette-2027.md` (domain rules) and
`docs/contract-pr1-2027.md` (wire format). The doc comments in `src/`
are the detailed spec.

## Modules

| Module | Content |
|---|---|
| `discipline` | `TimerMode` (`gruppenstaffette`, `loeschangriff`; legacy v1 strings are rejected), lane counts (GS: 1 or 2, default 2; LA: 4), `JudgeMode`, `SyncMode` (`offline` = on-prem, alias `on_prem`) |
| `station` | `StationKind`: `startlinie`, `verteilerlinie`, `strahlrohrlinie`, `knotengestell`, `angriffslinie`, `podest`. Legacy aliases `start`, `verteiler`, `strahlrohr`, `knoten`, `kuebelspritze` are accepted on input; output is always canonical. Stations per mode; only Startlinie records the lineup |
| `penalty` | `PenaltyKind` `sz` / `fv`, `PenaltyStatus` `pending` / `approved` / `rejected`, `SZ_PENALTY_MS` = 15 000 |
| `catalogue` | Fixed error catalogue from the FBL Wertungsbögen: 30 entries (Startlinie 10, Verteilerlinie 4, Strahlrohrlinie 6, Knotengestell 5, Angriffslinie 5; 24 SZ, 6 FV). Stable codes `<station>.<slug>`, never rename one |
| `timing` | `TimeSource` (`lichtschranke` / `handstopp`), `hand_time_mean` and `round_to_centis` (1/100 s, rounded half up) |
| `wg` | `Wertungsgruppe` WG1–WG5 (ticked by the club, never derived), `age_on_dec31` (WG age), `age_on_day` (Tagessieger age, Excel `DATEDIF` semantics), `check_roster` (warnings only) |
| `scoring` | `score_run`, `compute_results` (WG rankings + Tagessieger), `tagessieger_factor`, `competition_ranks` |

## Rules in short

- **Errors:** `pending` and `approved` count, `rejected` does not. Any
  counting `pending` error makes the result *provisional*.
- **Run:** valid = no counting FV and a time is present. Total = measured
  time + 15 s per counting SZ. Invalid runs have no time.
- **WG ranking:** the best valid run per team counts (lowest total at
  1/100 s; tie → lower run number). Places use competition ranking
  1, 1, 3, comparing totals at 1/100 s. Teams with only invalid runs share
  last place (ranked + 1) below the ranked teams. Teams without a
  completed run are "n.a." with no place and are listed last. Equal places
  and groups are ordered by start number.
- **WG ages:** age on 31.12. of the competition year. WG1 8–10, WG2/3 up
  to 14, WG4/5 up to 18. A mismatch only warns ("zu jung" / "zu alt").
- **Tagessieger** (variant a): take the team's best-time run with that
  run's lineup. Ages are completed years on the competition day. If the
  lineup is empty or none of its ids is in the roster, the whole roster
  is used (`lineup_fallback`). Lineup ids that are not in the roster are
  ignored. Factor = 1 − 2·(18 − avg)/100. GW-Zeit = total × factor,
  computed exactly in integers and rounded half up to 1/100 s, then
  ranked 1, 1, 3. Teams without a valid run or without any runner data
  are left out.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
