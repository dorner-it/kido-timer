# PR 1 contract — Gruppenstafette 2027 data model (backend ⇄ frontend ⇄ desktop)

Working agreement for PR 1 (`feat/kido-core-model`). Source of truth for
domain rules: `docs/concept-gruppenstafette-2027.md` and the crate
`crates/kido-core` (doc comments = spec). This file pins the **wire
format**. All JSON is snake_case. All new fields are optional/defaulted on
read so pre-2027 DynamoDB rows still load (read-time migration, no batch
rewrite needed).

Decisions (user, 2026-10-16): Tagessieger variant (a); 1 or 2 lanes per
discipline; judge mode per discipline; WG ticked by the club, no gender;
live vs. on-prem with desktop override; one judge per station, optionally
per station+lane.

## Shared enums (from `kido-core`, backend re-exports them)

| Type | Values |
|---|---|
| `TimerMode` | `gruppenstaffette`, `loeschangriff` |
| `SyncMode` | `live`, `offline` (= on-prem; alias `on_prem` accepted) |
| `JudgeMode` | `per_station` (default), `per_station_and_lane` |
| `StationKind` | `startlinie`, `verteilerlinie`, `strahlrohrlinie`, `knotengestell`, `angriffslinie`, `podest` — legacy aliases accepted on input: `start`, `verteiler`, `strahlrohr`, `knoten`, `kuebelspritze` |
| `PenaltyKind` | `sz`, `fv` |
| `PenaltyStatus` | `pending`, `approved`, `rejected` |
| `TimeSource` | `lichtschranke` (default), `handstopp` |
| `Wertungsgruppe` | `wg1` … `wg5` |

## Discipline

New fields:

```jsonc
{
  "lane_count": 2,            // gruppenstaffette: 1|2 (default 2); loeschangriff: 4
  "judge_mode": "per_station" // default per_station
}
```

- `POST/PUT /api/events/{id}/disciplines[/{discipline_id}]` accept both
  (optional; defaults as above). Invalid `lane_count` for the mode →
  `400 validation`.
- Changing `lane_count` never deletes data; entries with `lane >
  lane_count` keep their value but the response lists them in
  `lane_conflicts: [team_id]` (UI shows a warning).

## Event — parallel runs

```jsonc
{
  "current_run_id": "<uuid|null>",   // kept for old clients = current_run_ids[0]
  "current_run_ids": ["<uuid>", "<uuid>"] // NEW: one per active lane, max lane_count
}
```

- `POST /api/events/{id}/current` accepts `{ "discipline_id", "run_id" }`
  (legacy) **or** `{ "discipline_id", "run_ids": [..] }`.
- Rows without `current_run_ids` read it as `[current_run_id]` (or `[]`).

## Runner

- `gender` is no longer required on POST/PUT and is no longer written by
  the UI. Field stays readable (`null` for new rows).

## TeamEntry (Meldung)

```jsonc
{
  "wg": "wg2",                          // REQUIRED on POST/PUT (input, never derived)
  "wg_history": [                       // NEW, append-only, written by "Umsetzen"
    { "from": "wg2", "to": "wg1", "by": "<user email or sub>", "at": "<rfc3339>", "note": "2 Mädchen" }
  ],
  "wg_warnings": [                      // NEW, computed on every read, not stored
    { "runner_id": "<uuid>", "age": 15, "wg": "wg2", "reason": "zu alt" }
  ]
}
```

- Age check = `kido_core::wg::check_roster` with the **event year**
  (31.12.-age). Warnings never block saving.
- Legacy rows with `wg: null` → response `wg: null`, UI shows "WG fehlt".
- NEW `POST /api/events/{id}/disciplines/{discipline_id}/entries/{team_id}/move-wg`
  body `{ "wg": "wg1", "note": "optional" }` → sets `wg`, appends
  `wg_history`, returns the entry. Organizer only.
- `Wertungsgruppe::derive` and its gender logic are removed.

## Run

```jsonc
{
  "lineup": ["<runner uuid>", ...],     // NEW, default []
  "lineup_confirmed_at": "<rfc3339|null>", // NEW
  "lineup_confirmed_by": "startlinie|organizer|null", // NEW
  "time_source": "lichtschranke",       // NEW, default lichtschranke
  "hand_time_1_ms": null,               // NEW
  "hand_time_2_ms": null                // NEW
}
```

- NEW `PUT /api/events/{id}/disciplines/{discipline_id}/runs/{run_id}/lineup`
  body `{ "runner_ids": [..] }` (organizer). Every id must belong to the
  run's team, no duplicates; empty list allowed (clears).
- NEW `PUT /api/events/{id}/disciplines/{discipline_id}/runs/{run_id}/hand-times`
  body `{ "hand_time_1_ms": 47230, "hand_time_2_ms": 47245 }` → sets
  `time_source = handstopp`, `original_time_ms = hand_time_mean(..)`,
  `correction_ms = 0`. Body `{ "clear": true }` reverts to
  `lichtschranke` (keeps original_time_ms, clears hand times).
- Judge lineup (station `startlinie` only):
  `PUT /api/decider/{owner_sub}/{token}/runs/{run_id}/lineup`
  body `{ "runner_ids": [..] }`; run must be one of `current_run_ids`.

## Penalty

```jsonc
{
  "kind": "sz",                 // NEW. Read-time migration: missing kind →
                                //   is_disqualification == true ? "fv" : "sz"
  "code": "knotengestell.ein_knoten_falsch", // NEW, catalogue code; null on legacy rows
  "station": "knotengestell",   // canonical key on output
  "delta_ms": 15000,            // kept for old clients: sz → 15000, fv → 0
  "is_disqualification": false, // kept on output for old clients (= kind == fv); ignored on input
  "status": "pending",
  ...existing fields (id, run_id, team_id, decider_slot, reason_code, created_at, reviewed_*)
}
```

- Station submit `POST /api/decider/{owner_sub}/{token}/penalties` body:
  `{ "station": "knotengestell", "run_id": "<uuid>", "code": "knotengestell.ein_knoten_falsch", "client_id": "<uuid?>" }`.
  `kind`, `delta_ms` come from the catalogue. Code must exist and belong
  to `station`; station must be allowed for the discipline mode; run must
  be in `current_run_ids`. `run_id` optional only when exactly one run
  is current (backwards compatible).
- Legacy slot route keeps working (writes `kind = sz`, `code = null`,
  `delta_ms` as sent).
- Organizer review unchanged: approve / reject / bulk-approve
  ("Übernehmen": remaining pending → approved).
- NEW public read: `GET /api/catalogue` → `[{ code, station, kind, label_de }]`
  (no auth; static).

## Station state (judge app)

`GET /api/decider/{owner_sub}/{token}/state` adds:

```jsonc
{
  "discipline": { ..., "lane_count": 2, "judge_mode": "per_station" },
  "current_runs": [ { "run": Run, "team": Team, "runners": [Runner...] } ], // NEW, one per current run, ordered by lane
  "current_run": ...  // kept = current_runs[0] for old clients
}
```

## Export / `.kido` — schema_version 4

- `SCHEMA_VERSION = 4`. Payload carries the new Discipline, Event, Run,
  TeamEntry (`wg`, `wg_history`; not `wg_warnings`) and Penalty fields.
- Import accepts v4 only (v3 → `400 validation` with a clear message,
  same pattern as v2→v3).
- Desktop catches up in a later PR (it is still on v2).

## `Run.correction_ms` (legacy, not used for scoring)

`correction_ms` keeps its pre-2027 meaning for old clients: the backend
writes the sum of approved `delta_ms` of the run into it on every
approve/reject/bulk-approve. **Scoring never reads it**: the result is
always `measured + 15 s × counting SZ` (`kido_core::scoring`), where
`measured = original_time_ms` (light barrier) or the hand-time mean. So
SZ cannot be counted twice. The hand-times endpoint resets it to 0; the
next review recomputes it — harmless, because it is display-only.

## Results (kido-core only in PR 1)

`kido_core::scoring::compute_results` is implemented and tested in PR 1;
the HTTP endpoint follows in the results PR (PR 4) as
`GET /api/events/{id}/disciplines/{discipline_id}/results` (nested like
every other organizer route; shape in `docs/api.md` § Results).
