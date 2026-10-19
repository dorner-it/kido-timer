# PR 6 contract — on-prem mode (desktop as local server)

Decided 2026-10-16 (see `docs/concept-gruppenstafette-2027.md` §6): the
web sets `sync_mode` per discipline as a default, the desktop can override
it. **On-prem** = the desktop app starts a local HTTP server in the venue
network; judges' phones load the **same judge UI** from the desktop; the
organizer reviews in the desktop; afterwards everything is uploaded to the
web (button) or exported as `.kido` (fallback).

## A. Backend additions (web, API-key auth like `/api/export/*`)

### A1. All penalties in snapshot and `.kido` (schema v4, additive)

`GET /api/export/disciplines/{id}` and the `.kido` payload gain
`penalties: Penalty[]` — **all** penalties of the discipline (pending,
approved, rejected). `approved_penalties` stays unchanged for old readers.
Readers must default `penalties` to `[]` when missing. Import of a `.kido`
(`import-results`) reads `penalties` if present (upsert by id, keeps the
most advanced status: rejected/approved win over pending), else falls back
to `approved_penalties` as today.

### A2. Desktop switches the web's sync mode

`POST /api/export/disciplines/{id}/sync-mode` body `{ "sync_mode": "live" | "offline" }`
→ `200` with the updated `Discipline`. Same semantics as the organizer
route `POST /api/events/{id}/disciplines/{did}/sync-mode`.

### A3. Upload after on-prem

`POST /api/export/disciplines/{id}/onprem-upload` body:

```jsonc
{
  "runs": [ {                      // only runs the desktop changed
    "id": "<uuid>", "run_number": 1, "status": "completed",
    "original_time_ms": 47230, "started_at": "...", "ended_at": "...",
    "lineup": ["<runner uuid>"], "lineup_confirmed_at": "...", "lineup_confirmed_by": "startlinie|organizer",
    "time_source": "lichtschranke|handstopp", "hand_time_1_ms": null, "hand_time_2_ms": null,
    "penalties_accepted_at": null  // additive: local "Übernehmen"; later value wins, null never clears
  } ],
  "penalties": [ Penalty ],        // full rows incl. status/reviewed_*; id = client id from the judge phone
  "current_run_ids": []            // optional; cleared by default after an on-prem day
}
```

- Idempotent: runs updated by id (only fields above); penalties upserted
  by id; uploading twice gives the same state.
- Validation: runs/penalties must belong to the discipline; penalty
  `code` must exist in the catalogue and match `station`; `kind` and
  `delta_ms` are recomputed from the catalogue (client values ignored).
- After a successful upload `correction_ms` of affected runs is
  recomputed (legacy display value, see PR 1 contract).
- Review undo ("Rückgängig", additive): the web never turns a decision
  it already stores back to `pending` through an upload ("most advanced
  status wins" stays). The desktop therefore only offers undo for
  decisions made locally (its penalty overlay); decisions from the web
  snapshot (base approved / rejected) are not undoable on the desktop.
- Response `200 { "runs_updated": n, "penalties_upserted": m }`.
- Allowed regardless of the web's `sync_mode`.

## B. Desktop local server (on-prem)

- Starts only when the effective mode is **On-prem** and a discipline is
  loaded; stops when leaving on-prem or on app exit.
- Binds `0.0.0.0:8787` (fallback 8788…8797 if busy). Shows all non-
  loopback IPv4 addresses and a QR code with
  `http://<ip>:<port>/judge/?t=<token>`.
- `token` = random 128-bit, generated per on-prem session, persisted with
  the local store (survives restart so phones keep working), "Neuer
  Zugangscode" button rotates it (old phones then get 401).
- Serves the **embedded judge UI** (static build of the frontend judge app,
  see C) under `/judge/` and the API below. Same JSON shapes as the cloud
  (PR 1/PR 2 contracts), so the judge UI is identical:
  - `GET  /api/catalogue`
  - `GET  /api/decider/{owner_sub}/{token}/state?station=` (incl.
    `current_runs`, `discipline.lane_count/judge_mode`, `penalties`)
  - `POST /api/decider/{owner_sub}/{token}/penalties`
  - `DELETE /api/decider/{owner_sub}/{token}/penalties/{penalty_id}`
  - `PUT  /api/decider/{owner_sub}/{token}/runs/{run_id}/lineup`
  - `owner_sub` = the snapshot's owner. Wrong token → 401. Same
    validation rules as the backend (use `kido-core` catalogue/station
    helpers; share pure validation code where possible).
- No CORS needed (same origin). Request body limit (e.g. 64 KiB), simple
  rate limit per IP. Plain HTTP (LAN); the judge UI already avoids
  secure-context APIs.
- All writes go into the persisted local store (penalties incl. status,
  lineup, hand times, `current_run_ids`) with the same fsync/rename
  persistence as PR 5.

## C. Static judge build

- `frontend`: `npm run build:judge` builds the judge React app as a plain
  SPA (Vite, no Astro SSR) into `frontend/dist-judge/` with base `/judge/`.
  Reads `t` (token) and `o` (owner_sub, optional — the desktop can inject
  it) from the URL; API base = same origin (`window.__KIDO_API_BASE__ = ''`
  injected by the desktop or default to `location.origin`). No external
  fonts/CDNs.
- `desktop/src-tauri` embeds `frontend/dist-judge` at compile time
  (`rust-embed` or `include_dir`). If the folder is missing, embed a
  placeholder page saying the judge UI was not built (so `cargo build`
  works without Node) — CI/release builds it first.
- `.github/workflows/ci.yml` and `desktop-release.yml`: build
  `frontend` judge bundle before the desktop build.

## D. Organizer in the desktop (on-prem)

- Current runs per lane (up to `lane_count`) — sets local
  `current_run_ids`.
- Review per current run: errors by station, Bestätigen / Zurücknehmen,
  rejected crossed out, FV with confirm dialog, "Übernehmen" (pending →
  approved, sets local `Run.penalties_accepted_at`), "Rückgängig"
  (decided → pending; only for decisions made on this desktop — not for
  ones from the web snapshot — and only after the last "Übernehmen"),
  summary (same wording as web PR 3).
- Hand times (two inputs, mean via `kido_core::timing::hand_time_mean`),
  lineup correction.
- Results/print (PR 5) use the local data.

## E. Going back to live / upload

- "Ins Web hochladen" (needs internet + API key): A2 (set web to
  `offline` during upload is not needed — A3 works in any mode), then A3,
  then optionally A2 `live`. On success the local store is marked synced.
- Divergence dialog from PR 5 uses A3 instead of per-run result POSTs.
- Desktop override "Live" calls A2 `live` first, so result POSTs are no
  longer rejected with 409.
- `.kido` export includes `penalties` (A1) and all local run fields
  (incl. `penalties_accepted_at`).

## F. Windows firewall

NSIS installer hook adds an inbound firewall rule for the app exe
(private + public profiles, TCP 8787–8797) on install and removes it on
uninstall (`netsh advfirewall firewall add/delete rule`). Documented in
the desktop README together with the manual fallback.
