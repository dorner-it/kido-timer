# API Reference

The endpoint surface and conventions live in [`CLAUDE.md` §6 — API Surface](../CLAUDE.md#6-api-surface). This file documents concrete request/response shapes for the parts that changed with the Gruppenstafette 2027 data model (PR 1, wire contract: [contract-pr1-2027.md](./contract-pr1-2027.md); domain: [concept-gruppenstafette-2027.md](./concept-gruppenstafette-2027.md)).

For the **desktop-app integration contract** — `.kido` file format, HMAC envelope, `/api/export/...` endpoints, and end-to-end live/offline flows — see [desktop-integration.md](./desktop-integration.md).

Conventions: JSON snake_case, RFC 3339 UTC timestamps, lowercase UUIDs. Errors use `{ "error": { "code", "message" } }`; rule violations below are `400 validation` unless noted. Every new field is optional/defaulted on read, so pre-2027 DynamoDB rows still load (read-time migration, no batch rewrite).

## Shared enums (from `crates/kido-core`)

| Type | Values |
|---|---|
| `TimerMode` | `gruppenstaffette`, `loeschangriff` |
| `SyncMode` | `live`, `offline` (= on-prem; `on_prem` accepted on input) |
| `JudgeMode` | `per_station` (default), `per_station_and_lane` |
| `StationKind` | `startlinie`, `verteilerlinie`, `strahlrohrlinie`, `knotengestell`, `angriffslinie`, `podest`. Legacy input aliases: `start`, `verteiler`, `strahlrohr`, `knoten`, `kuebelspritze`. Output is always canonical. |
| `PenaltyKind` | `sz` (+15 s), `fv` (run invalid) |
| `PenaltyStatus` | `pending`, `approved`, `rejected` |
| `TimeSource` | `lichtschranke` (default), `handstopp` |
| `Wertungsgruppe` | `wg1` … `wg5` |

## Disciplines

`POST /api/events/{id}/disciplines`, `PUT /api/events/{id}/disciplines/{discipline_id}` (organizer)

```jsonc
{
  "name": "Gruppenstafette",
  "mode": "gruppenstaffette",
  "position": 1,
  "lane_count": 2,            // optional; gruppenstaffette 1|2 (default 2), loeschangriff 4
  "judge_mode": "per_station" // optional; default per_station
}
```

- Invalid `lane_count` for the mode → `400 validation`.
- On `PUT`, an omitted `lane_count` keeps the stored value if it is still valid for the (possibly changed) mode, otherwise the mode default; an omitted `judge_mode` keeps the stored value.
- Response = the discipline plus `lane_conflicts: [team_id]` — entries whose `lane > lane_count` (data is never deleted; the UI warns). Always `[]` on `POST`.

## Event — current runs

`POST /api/events/{id}/current` (organizer) accepts

```jsonc
{ "discipline_id": "<uuid>", "run_id": "<uuid>" }            // legacy
{ "discipline_id": "<uuid>", "run_ids": ["<uuid>", "<uuid>"] } // one per active lane
```

- `run_ids` wins when both are sent. Neither → no current run.
- Runs require `discipline_id`; at most `lane_count` runs; no duplicates; every run must belong to the discipline.
- The event carries `current_run_ids` plus the legacy `current_run_id` (= `current_run_ids[0]`). Legacy rows read as `[current_run_id]` (or `[]`).

## Runners

`gender` is optional on `POST`/`PUT` and no longer written by the UI; the field stays readable (`null` for new rows).

## Team entries (Meldungen)

`POST /api/events/{id}/disciplines/{discipline_id}/entries` (organizer, upsert)

```jsonc
{ "team_id": "<uuid>", "start_number": 4, "lane": 1, "wg": "wg2" }
```

- `wg` is **required** (ticked by the club, never derived) → missing = `400 validation`.
- `lane` must be within `1..=lane_count` of the discipline.
- An existing `wg_history` is preserved by the upsert.

Entry responses (upsert, move-wg, and `team_entries` in `GET /api/events/{id}`):

```jsonc
{
  "team_id": "…", "discipline_id": "…", "event_id": "…",
  "start_number": 4, "lane": 1,
  "wg": "wg2",                // null on legacy rows ("WG fehlt")
  "wg_history": [ { "from": "wg2", "to": "wg1", "by": "fbl@example.org", "at": "…", "note": "2 Mädchen" } ],
  "wg_warnings": [ { "runner_id": "…", "age": 15, "wg": "wg2", "reason": "zu alt" } ],
  "created_at": "…", "updated_at": "…"
}
```

`wg_warnings` is computed on every read with `kido_core::wg::check_roster` and the **event year** (31.12.-age); it is never stored or exported and never blocks saving.

`POST /api/events/{id}/disciplines/{discipline_id}/entries/{team_id}/move-wg` (organizer, "Umsetzen")

```jsonc
{ "wg": "wg1", "note": "optional" }
```

Sets `wg` and appends `{ from, to, by, at, note }` to `wg_history` (`by` = organizer e-mail, else Cognito `sub`). Moving to the current WG is a no-op. Returns the entry.

## Runs

New fields on every run:

```jsonc
{
  "lineup": ["<runner uuid>"],          // default []
  "lineup_confirmed_at": "<rfc3339|null>",
  "lineup_confirmed_by": "startlinie|organizer|null",
  "time_source": "lichtschranke",       // default
  "hand_time_1_ms": null,
  "hand_time_2_ms": null
}
```

`PUT /api/events/{id}/disciplines/{discipline_id}/runs/{run_id}/lineup` (organizer)

```jsonc
{ "runner_ids": ["<uuid>", "…"] }
```

Every id must belong to the run's team, no duplicates. Empty list clears the lineup (and the confirmation). Non-empty sets `lineup_confirmed_by = organizer`. Returns the run.

`PUT /api/events/{id}/disciplines/{discipline_id}/runs/{run_id}/hand-times` (organizer)

```jsonc
{ "hand_time_1_ms": 47230, "hand_time_2_ms": 47245 } // → time_source handstopp, original_time_ms = 47240, correction_ms = 0
{ "clear": true }                                      // → time_source lichtschranke, hand times cleared, original_time_ms kept
```

Both times are required (> 0) unless `clear: true`; `clear` together with times is rejected. The mean is rounded to 1/100 s, half up (`kido_core::timing::hand_time_mean`).

## Penalties

```jsonc
{
  "id": "…", "run_id": "…", "team_id": "…", "decider_slot": 0,
  "kind": "sz",                                  // legacy rows: is_disqualification ? "fv" : "sz"
  "code": "knotengestell.ein_knoten_falsch",     // null on legacy rows / slot route
  "station": "knotengestell",                    // canonical key
  "delta_ms": 15000,                             // catalogue: sz → 15000, fv → 0; legacy rows unchanged
  "is_disqualification": false,                  // output only (= kind == fv), ignored on input
  "status": "pending",
  "reason_code": null, "created_at": "…", "reviewed_at": null, "reviewed_by": null
}
```

Organizer review is unchanged: approve / reject / `POST /api/events/{id}/runs/{run_id}/penalties/bulk-approve` ("Übernehmen": remaining pending → approved; response `dq` counts approved FV).

### `GET /api/catalogue` (public, no auth, static)

```json
[ { "code": "knotengestell.ein_knoten_falsch", "station": "knotengestell", "kind": "sz", "label_de": "1 Knoten falsch" } ]
```

Source: `kido_core::catalogue::CATALOGUE`, in sheet order.

## Results (Ergebnisse)

`GET /api/events/{id}/disciplines/{discipline_id}/results` (organizer; `404` for unknown event/discipline, `401` without session)

Computed on every request by `kido_core::scoring::compute_results` (rules: [concept §5](./concept-gruppenstafette-2027.md#5-scoring-backend), `crates/kido-core/src/scoring.rs`). Mapping of stored rows (`backend/src/results.rs::build_scoring_input`):

- `completed` = `run.status == "completed"`; pending/active/cancelled runs are ignored.
- measured time = `original_time_ms` (with `time_source = handstopp` this already holds the hand-time mean). **`correction_ms` is never added** — the total is always measured + 15 s × counting SZ.
- Penalties of all statuses are passed; `pending` and `approved` count (pending → `provisional`), `rejected` does not.
- Teams = entries of the discipline with a `wg`. Entries with `wg: null` (legacy rows) are not ranked and listed in `missing_wg`.
- Tagessieger ages = completed years on the event date, over the counting run's `lineup` (fallback: whole roster → `lineup_fallback: true`).

```jsonc
{
  "event": { "id": "…", "name": "Kreisausscheid", "date": "2027-06-12", "location": "Musterstadt" },
  "discipline": { "id": "…", "name": "Gruppenstafette", "mode": "gruppenstaffette" },
  "generated_at": "2027-06-12T14:03:11.512Z",
  "provisional": true,                 // any ranking row or Tagessieger row is provisional
  "wg_rankings": [                     // WG1 … WG5, only WGs with at least one entry
    {
      "wg": "wg2", "wg_label": "WG2", "wg_label_de": "Jungen bis 14",
      "rows": [                        // ranked (by place, start number), then all_invalid, then not_started
        {
          "team_id": "…", "team_name": "FF Musterdorf", "start_number": 1,
          "place": 1,                  // all_invalid: shared last place (= ranked + 1); not_started: null ("n.a.")
          "status": "ranked",          // ranked | all_invalid | not_started
          "best_run_id": "…", "best_total_ms": 45000,   // null unless ranked
          "provisional": false,
          "runs": [                    // completed runs only, by run_number
            {
              "run_id": "…", "run_number": 1, "lane": 1,
              "time_source": "handstopp", "hand_time_1_ms": 44990, "hand_time_2_ms": 45010,
              "measured_ms": 45000,    // original_time_ms
              "sz_count": 0, "fv_count": 0,
              "total_ms": 45000,       // measured + 15 s × SZ; null when invalid
              "valid": true,
              "invalid_reason": null,  // fehlversuch | no_time
              "provisional": false,
              "is_best": true          // the counting run
            }
          ]
        }
      ]
    }
  ],
  "tagessieger": [                     // by place, then start number; teams without a valid run are left out
    {
      "place": 1, "team_id": "…", "team_name": "FF Musterdorf", "start_number": 1,
      "wg": "wg2", "wg_label": "WG2",
      "run_id": "…", "run_number": 2,
      "total_ms": 45000,               // Bestzeit
      "ages": [12, 13],
      "avg_age": 12.5,                 // rounded to 1 decimal (display)
      "factor": 0.89,                  // 1 − 2·(18 − avg)/100, rounded to 4 decimals (display)
      "gw_ms": 40050,                  // total × factor, exact integer math, rounded to 1/100 s
      "lineup_fallback": false,        // true → "Kader statt Besetzung"
      "provisional": false
    }
  ],
  "missing_wg": [ { "team_id": "…", "team_name": "…", "start_number": 5 } ]
}
```

`gw_ms` and the places are computed from the exact values, not from the rounded `avg_age` / `factor`.

## Judge app (station token)

`GET /api/decider/{owner_sub}/{token}/state` adds

```jsonc
{
  "discipline": { "id": "…", "name": "…", "mode": "gruppenstaffette", "lane_count": 2, "judge_mode": "per_station" },
  "current_runs": [ { "run": Run, "team": Team, "runner": null, "runners": [Runner, …] } ], // ordered by lane
  "current_run": { … }   // = current_runs[0], kept for old clients
}
```

`runners` is the team's full roster (by position) for the lineup tick list.

PR 2: optional query `?station=<key>` (canonical or legacy key). The
response then also carries `"penalties": [Penalty, …]` — the penalties
of the current runs (all statuses, oldest first), restricted to that
station when `station` is given. The judge app uses it to show the
review status of its own entries (ausstehend / bestätigt /
zurückgenommen). Empty when no run is current or the discipline is
offline. Clients must tolerate the field being absent (older backend,
future on-prem server).

`POST /api/decider/{owner_sub}/{token}/penalties`

```jsonc
{ "station": "knotengestell", "run_id": "<uuid>", "code": "knotengestell.ein_knoten_falsch", "client_id": "<uuid?>" }
```

- `kind` and `delta_ms` come from the catalogue; `code` is required, must exist and belong to `station` (`400 validation`).
- `station` must be allowed for the discipline mode (`400 validation`).
- `run_id` must be one of `current_run_ids` (`409 conflict` otherwise). It may be omitted only when exactly one run is current; with several current runs → `400 validation`; with none → `409 conflict`.
- `client_id` keeps the submit idempotent (pending rows are updated, reviewed rows returned unchanged).

`PUT /api/decider/{owner_sub}/{token}/runs/{run_id}/lineup` (Startlinie judge)

```jsonc
{ "runner_ids": ["<uuid>"], "station": "startlinie" } // station optional; if sent it must be startlinie (else 403)
```

The run must be one of `current_run_ids` (`409 conflict`). Same lineup rules as the organizer endpoint; sets `lineup_confirmed_by = startlinie`.

Legacy slot route `POST /api/decider/{owner_sub}/{slot}/{token}/penalties` keeps working: writes `kind = sz`, `code = null`, `delta_ms` as sent. It needs exactly one current run (several → `409 conflict`).

## Schema versioning

The desktop-app export contract is versioned via a top-level `"schema_version"` integer (and the `.kido` envelope `v`). Any breaking change to `/api/export/*` requires bumping this value and coordinating a desktop-app release. Current value: `4` (Gruppenstafette 2027). Imports of older files (v3 and below) are rejected with `400 validation` and a message naming both versions.
