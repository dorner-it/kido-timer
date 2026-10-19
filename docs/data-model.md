# Data Model — DynamoDB Single Table

See [`CLAUDE.md` §5 — DynamoDB Single-Table Design](../CLAUDE.md#5-dynamodb-single-table-design) for the canonical item shapes, key construction, and access patterns.

## Index summary

| Index   | Partition key | Sort key |
|---------|---------------|----------|
| Primary | `PK`          | `SK`     |
| GSI1    | `GSI1PK`      | `GSI1SK` |

All access in production code paths must be `GetItem` or `Query`. **No scans.**

## Gruppenstafette 2027 attributes (PR 1)

No key changed. New attributes are optional on read (`#[serde(default)]` in the `db/*` item structs); old items are migrated **at read time** and rewritten in the new shape on their next save. No batch migration is needed.

| Item | New attribute | Type | Missing on old items reads as |
|---|---|---|---|
| Discipline | `lane_count` | N | `mode.default_lane_count()` (Gruppenstaffette 2, Löschangriff 4) |
| Discipline | `judge_mode` | S `per_station` \| `per_station_and_lane` | `per_station` |
| Event (META) | `current_run_ids` | L of S | `[current_run_id]` or `[]`; `current_run_id` is still written (= first element) |
| TeamEntry | `wg` | S `wg1`…`wg5` | `null` (now required input on every write, never derived) |
| TeamEntry | `wg_history` | L of M `{from, to, by, at, note}` | `[]` |
| Run | `lineup` | L of S (runner ids) | `[]` |
| Run | `lineup_confirmed_at` / `lineup_confirmed_by` | S / S `startlinie` \| `organizer` | `null` |
| Run | `time_source` | S `lichtschranke` \| `handstopp` | `lichtschranke` |
| Run | `hand_time_1_ms` / `hand_time_2_ms` | N | `null` |
| Penalty | `kind` | S `sz` \| `fv` | `is_disqualification ? fv : sz` |
| Penalty | `code` | S (catalogue code) | `null` |
| Penalty | `station` | S | legacy keys (`start`, `verteiler`, `strahlrohr`, `knoten`, `kuebelspritze`) read as the canonical key and are written canonically on the next save |

Kept for old readers: Penalty `is_disqualification` (always written as `kind == fv`) and `delta_ms`; Event `current_run_id`; Runner `gender` (optional, no longer written by the UI).

TeamEntry `wg_warnings` is **not** stored — it is computed on every read.

## On-prem round trip (PR 6)

No new attributes and no key change. `POST /api/export/disciplines/{id}/onprem-upload`
and the `.kido` import write ordinary Run / Penalty items (full `PutItem`,
so `GSI1SK` follows the new run / penalty status) and, for
`current_run_ids`, the Event META item. Penalties uploaded from the
desktop keep their client id as `id` (SK `…PENALTY#<discipline>#<id>`), so
re-uploads overwrite the same item. `reviewed_by` may carry the
sentinels `"onprem-upload"` (reviewed on the desktop without a reviewer
name) or `"offline-import"` (legacy `approved_penalties` import).
