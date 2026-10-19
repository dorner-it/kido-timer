# Data Model — DynamoDB Single Table

See [`CLAUDE.md` §5 — DynamoDB Single-Table Design](../CLAUDE.md#5-dynamodb-single-table-design) for the canonical item shapes, key construction, and access patterns.

## Index summary

| Index   | Partition key | Sort key |
|---------|---------------|----------|
| Primary | `PK`          | `SK`     |
| GSI1    | `GSI1PK`      | `GSI1SK` |

All access in production code paths must be `GetItem` or `Query`. **No scans.**

## Runner sort key

`SK = RUNNER#<team_id>#<position:04>#<runner_id>` (see `backend/src/db/keys.rs`).

The runner id is part of the key because the position alone is not unique:
with the old key `RUNNER#<team_id>#<position:04>` moving a runner onto an
occupied position (or creating one there) silently overwrote the other
runner's item. Position stays first and zero-padded, so
`begins_with(SK, "RUNNER#<team_id>#")` still returns a team's roster in
position order.

- **Legacy rows** (old key, no `#<runner_id>`) stay readable — both shapes
  match the query prefixes `RUNNER#` / `RUNNER#<team_id>#`. A legacy row is
  moved to the new key on its next write (update deletes it with condition
  `id = :id` and puts the new key). Delete removes both the new and the
  legacy key, each conditional on `id = :id`, so a legacy (position-only)
  key can never take another runner with it. No migration runs in the
  request path; `cargo run --bin migrate_runner_keys -- --table <t> [--dry-run]`
  is an optional, idempotent operator cleanup.
- **Position change = swap.** Moving runner A onto a position held by B
  gives B A's old position. All deletes/puts for that change go through
  one `TransactWriteItems`; a stale roster (failed `id = :id`) cancels it
  → `409 conflict`, nothing written.
- **Duplicate positions** are allowed (e.g. create at an occupied position
  keeps both runners). Lists are ordered by position, then name.
- **Team delete cascades** to the team's runner rows (every stored row,
  new or legacy key, conditional on `id`) and its Meldungen (`ENTRY#…`,
  conditional on `team_id`); the team item goes last. `TransactWriteItems`
  in chunks of 100 (team in the last chunk, so a failed chunk leaves the
  team in place for a retry). Runs and penalties are kept.
- Nothing outside `db/runners.rs`, the team cascade (which deletes the SKs
  exactly as read) and the migration tool looks at the runner SK; exports and `.kido` files carry `position` as an attribute.

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
| Run | `penalties_accepted_at` | S (RFC 3339; last "Übernehmen", set by bulk-approve via `UpdateItem`) | `null` (nothing frozen — every decision can be undone) |
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
