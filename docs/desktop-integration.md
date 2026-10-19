# Desktop Integration Spec

Contract between the standalone **KI-DO Timer desktop app** and the **KI-DO Timer Web** backend.

This is the source of truth for: the `.kido` file format, the API surface, and the live/offline round-trip flows. Treat as **stable**: any breaking change bumps `schema_version` (file payload) and/or introduces a new endpoint path; no silent format drift.

---

## 1. Identities and credentials

| Credential | Holder | Source | Used for |
|---|---|---|---|
| **Cognito session cookie** | Browser (organizer) | `/api/auth/sign-in` | Web UI; not used by desktop |
| **Desktop API key** | Desktop app | Issued via web `/settings`; shown once at creation; argon2id-hashed in DDB | `X-API-Key` header on `/api/export/...` |
| **Per-user HMAC key** | Desktop app (cached) and server | `GET /api/export/me/hmac-key` (one-time, API-key authed) | Signing/verifying `.kido` envelopes |

**Onboarding sequence for a fresh desktop install:**

1. Operator opens the web `/settings` page, clicks *+ Neuer Schlüssel*, copies the plaintext (shown exactly once).
2. Operator pastes the plaintext into the desktop app; desktop stores it in the OS keychain.
3. Desktop calls `GET /api/export/me` to confirm the key authenticates and learn the user's `sub` / `email`.
4. Desktop calls `GET /api/export/me/hmac-key`, caches the returned 32-byte base64url string locally (keychain).

Rotating the API key on web → desktop must be re-paired (steps 1–4). Rotating the HMAC key (TBD UI) → desktop must re-fetch via step 4 and any in-flight `.kido` files become unverifiable.

---

## 2. `.kido` file format

A `.kido` file is a UTF-8 JSON document containing a tamper-evident envelope:

```jsonc
{
  "v": 4,                                  // envelope version (matches schema_version)
  "alg": "HS256",                          // HMAC-SHA256, fixed
  "owner_sub": "<cognito-sub>",            // tenant identifier
  "issued_at": "2026-05-06T10:13:00Z",     // RFC 3339 UTC
  "payload": "<base64-of-utf8-json>",      // standard base64 (with padding)
  "sig": "<base64url-no-pad>"              // HMAC-SHA256 over payload bytes
}
```

Schema history:
- **v1**: original flat `Competition` model (gone — `/api/competitions/...` returns 410).
- **v2**: introduced the `Event → Discipline` hierarchy.
- **v3**: replaced the generic `TimerMode` with discipline-specific
  `gruppenstaffette` (2 lanes) and `loeschangriff` (4 lanes). Old values
  are rejected with `400 validation`.
- **v4** (2026-10, Gruppenstafette 2027): Discipline `lane_count` /
  `judge_mode`, Event `current_run_ids`, Run `lineup*` / `time_source` /
  `hand_time_*`, TeamEntry `wg` (input) + `wg_history`, Penalty `kind` /
  `code` and canonical station keys — see §2.5. The web imports v4
  only; v3 files are rejected with `400 validation` ("veraltetes Schema
  v3 … erwartet v4"). The desktop app reads and writes v4 only since
  0.6.0 (older envelopes, payloads, live snapshots and local backups are
  rejected with "Bitte Web-App/Desktop aktualisieren").

### 2.1 Signing rule

```
sig = base64url_no_pad(
        HMAC_SHA256(
          key   = base64url_decode(user.hmac_key),       // 32 bytes
          input = utf8_bytes(envelope.payload)            // base64 string, NOT decoded
        )
      )
```

The MAC covers the **base64 string** of the payload, not the raw JSON, so verification doesn't depend on canonical JSON formatting.

### 2.2 Inner payload

> The example below shows the historical v3 field set; the v4 additions
> are listed in §2.5. The current payload is a `DisciplinePayload`
> (`event`, `discipline`, `teams`, `runners`, `team_entries`, `runs`,
> `approved_penalties`).

After base64-decoding `envelope.payload` you get this JSON:

```jsonc
{
  "schema_version": 4,
  "exported_at": "2026-05-06T10:13:00Z",
  "owner_sub": "<cognito-sub>",                 // must equal envelope.owner_sub
  "competition": {
    "id": "<uuid>",
    "owner_sub": "<cognito-sub>",
    "name": "Bezirksausscheid 2026",
    "date": "2026-06-12",                       // YYYY-MM-DD
    "location": "Sportplatz Musterhausen",      // nullable
    "mode": "loeschangriff",                    // see TimerMode below
    "sync_mode": "live",                        // see SyncMode below
    "is_active": true,                          // organizer toggle
    "current_run_id": "<uuid|null>",            // run currently on the lane
    "created_at": "...",
    "updated_at": "..."
    // NOTE: decider_tokens are intentionally NOT in the file
  },
  "teams": [
    { "id": "<uuid>", "competition_id": "<uuid>", "name": "FF Beispiel", "start_number": 1 }
  ],
  "runners": [
    {
      "id": "<uuid>", "team_id": "<uuid>", "competition_id": "<uuid>",
      "first_name": "Erika", "last_name": "Mustermann",
      "birth_date": "2002-04-12", "position": 1
    }
  ],
  "runs": [
    {
      "id": "<uuid>", "competition_id": "<uuid>", "team_id": "<uuid>", "runner_id": null,
      "run_number": 1, "lane": 1, "status": "completed",
      "original_time_ms": 47230, "correction_ms": 0,
      "started_at": "...", "ended_at": "...",
      "created_at": "...", "updated_at": "..."
    }
  ],
  "approved_penalties": [
    {
      "id": "<uuid>", "competition_id": "<uuid>", "run_id": "<uuid>",
      "team_id": "<uuid>", "runner_id": null, "decider_slot": 1,
      "delta_ms": 5000, "reason_code": null,
      "status": "approved",
      "created_at": "...", "reviewed_at": "...", "reviewed_by": "offline-decision"
    }
  ]
}
```

**Enum values:**

```
TimerMode    = "gruppenstaffette" | "loeschangriff"
SyncMode     = "live" | "offline"
RunStatus    = "pending" | "active" | "completed" | "cancelled"
PenaltyStatus = "pending" | "approved" | "rejected"
```

`TimerMode` reflects the actual discipline run on TRV Kocab hardware
rather than a generic lane layout. Lane counts are intrinsic to the
variant:

| Variant            | Lanes | Discipline                                 |
|--------------------|-------|--------------------------------------------|
| `gruppenstaffette` | 2     | Gruppenstaffette (group-vs-group relay)    |
| `loeschangriff`    | 4     | Löschangriff (classic fire-attack)         |

Unknown enum values — including the v1/v2 strings `single_lane`,
`two_lane_parallel`, `relay`, `individual` — must cause the consumer to
reject the file with a clear error (don't silently fall through).

### 2.3 Direction conventions

The same envelope shape is used in both directions:

| Direction | Producer | What's populated |
|---|---|---|
| Web → Desktop (preparation) | Web download | `competition`, `teams`, `runners`. `runs`/`approved_penalties` are usually empty (or carry pre-existing live data if any). |
| Desktop → Web (offline result) | Desktop export | All of the above **plus** the desktop's `runs[]` (with `original_time_ms`, `started_at`, `ended_at`, `status`) and any `approved_penalties[]` decided locally. |

When the desktop produces the result file, it MUST:
- Preserve `competition.id`, all `team.id` and `runner.id` values from the source file (don't regenerate UUIDs).
- Set `payload.exported_at` to its own clock at write time.
- Set `payload.owner_sub` and `envelope.owner_sub` to the cached `me().sub`.
- Sign with the cached HMAC key.

### 2.4 File extension and MIME

- Extension: `.kido`
- HTTP `Content-Type` on the web's download response: `application/json; charset=utf-8`
- HTTP `Content-Disposition`: `attachment; filename="kido-<slug>-<date>.kido"`

The desktop is free to write the file with any name the operator picks; the web import endpoint accepts any filename.

### 2.5 Schema v4 additions (Gruppenstafette 2027)

All additions are present on export; on import every addition is optional
and defaulted (same read-time migration as the DynamoDB rows), but
`schema_version` / envelope `v` must be `4`.

```jsonc
{
  "schema_version": 4,
  "event": {
    "current_run_id": "<uuid|null>",          // legacy mirror = current_run_ids[0]
    "current_run_ids": ["<uuid>", "<uuid>"]   // one per active lane
  },
  "discipline": {
    "mode": "gruppenstaffette",
    "lane_count": 2,                          // gruppenstaffette 1|2, loeschangriff 4
    "judge_mode": "per_station",              // | "per_station_and_lane"
    "sync_mode": "offline"                    // "on_prem" accepted as alias on input
  },
  "team_entries": [
    {
      "wg": "wg2",                            // ticked by the club; null on legacy rows
      "wg_history": [ { "from": "wg2", "to": "wg1", "by": "…", "at": "…", "note": null } ]
      // wg_warnings is NOT exported (computed on read in the web API)
    }
  ],
  "runs": [
    {
      "lineup": ["<runner uuid>"],
      "lineup_confirmed_at": "<rfc3339|null>",
      "lineup_confirmed_by": "startlinie",    // | "organizer" | null
      "time_source": "handstopp",             // | "lichtschranke" (default)
      "hand_time_1_ms": 47230,
      "hand_time_2_ms": 47245                 // original_time_ms = mean rounded to 1/100 s
    }
  ],
  "approved_penalties": [
    {
      "kind": "fv",                           // "sz" (+15 s) | "fv" (run invalid)
      "code": "knotengestell.mehrere_knoten_falsch", // catalogue code, null on legacy rows
      "station": "knotengestell",             // canonical key
      "delta_ms": 0,                          // sz → 15000, fv → 0 (legacy rows unchanged)
      "is_disqualification": true             // = kind == fv, kept for old readers
    }
  ]
}
```

New enum values:

```
JudgeMode      = "per_station" | "per_station_and_lane"
StationKind    = "startlinie" | "verteilerlinie" | "strahlrohrlinie" | "knotengestell" | "angriffslinie" | "podest"
                 (legacy input aliases: start, verteiler, strahlrohr, knoten, kuebelspritze)
PenaltyKind    = "sz" | "fv"
TimeSource     = "lichtschranke" | "handstopp"
Wertungsgruppe = "wg1" … "wg5"
```

Missing fields read as: `current_run_ids` → `[current_run_id]`;
`lane_count` → mode default; `judge_mode` → `per_station`; `wg_history`,
`lineup` → `[]`; `time_source` → `lichtschranke`; `kind` →
`is_disqualification ? fv : sz`; `code` → `null`.

The live endpoints `GET /api/export/events` and `GET /api/export/disciplines`
additionally return `current_run_ids` and `lane_count` / `judge_mode`.

### 2.6 Desktop local result store (0.6.0)

The desktop keeps its own copy of the loaded discipline so no result is
lost — neither on-prem nor when a live POST fails or the app crashes.

- **Model**: `base` = last snapshot from the web (picker / live poll) or
  from a `.kido` file; `run_edits` = run timing changes recorded on the
  desktop that the web has not acknowledged. The UI, the results and the
  `.kido` export use base + edits.
- **Live**: every change is posted (`POST …/runs/:run_id/result`); the
  edit is dropped when the POST succeeds, kept (and queued as failed
  post) when it fails. Polls never hide a local edit.
- **On-prem** (`sync_mode = offline`, web default or desktop override):
  nothing is posted; edits are the master copy and end up in the `.kido`
  export (status, `original_time_ms`, `started_at`, `ended_at`).
- **Back to live**: if local edits differ from the web (status/time),
  posting stops and the operator chooses "Lokale Ergebnisse hochladen"
  (one result POST per run, in run-number order) or "Lokale Änderungen
  verwerfen". Penalties / lineups are not uploaded yet (on-prem PR).
  Note: the web rejects result POSTs with `409` while the discipline is
  set to `offline` on the web — switch it to live there first.
- **Persistence**: JSON under the Tauri app data dir,
  `<app_data>/local-store/disciplines/<discipline_id>.json`
  (`{ format: 1, saved_at, effective_mode, store: { base, run_edits },
  failed_posts }`) plus `local-store/current.json` (discipline restored on
  start). Written atomically (tmp + rename) after every change; files are
  kept per discipline, unreadable files are moved aside as
  `<id>.broken-<unix>.json`, never overwritten.
- **Sync-mode override**: stored per discipline in the desktop's
  localStorage (`kido-timer:sync-overrides:v1`); choosing the web default
  removes the override.

---

## 3. API surface

Base URL is the API Gateway endpoint (currently `https://idr9teznt4.execute-api.eu-central-1.amazonaws.com`, see `terraform output api_endpoint`). All requests/responses are JSON unless noted. All timestamps are RFC 3339 UTC. All IDs are lowercase UUID v4.

### 3.1 Error envelope

Every non-2xx response from this API has shape:

```json
{ "error": { "code": "snake_case_code", "message": "human-readable" } }
```

Mappings used by the live/offline endpoints:

| HTTP | `code` | When |
|---|---|---|
| 400 | `validation` | Body shape / enum / id mismatch |
| 401 | `unauthorized` | Missing/invalid `X-API-Key` (or session cookie) |
| 403 | `forbidden` | Authenticated but not the resource owner |
| 404 | `not_found` | Competition / run / penalty doesn't exist |
| 409 | `conflict` | Mode mismatch (live ingest while offline, etc.) |
| 429 | `rate_limited` | Reserved; not currently emitted by these routes |
| 500 | `internal` | Server bug; log the request id, retry later |

### 3.2 Endpoints (desktop, `X-API-Key`)

#### `GET /api/export/me`

Identity check. Returns the user the API key resolves to.

**Response 200:**
```json
{
  "sub": "8a4b...",
  "email": "drewis@dorner-it.de",
  "display_name": "Drewis Dorner",
  "key_id": "<uuid>"
}
```

#### `GET /api/export/me/hmac-key`

One-shot HMAC key fetch. Cache locally; do **not** call on every operation.

**Response 200:**
```json
{ "hmac_key": "<base64url-no-pad of 32 bytes>" }
```

#### `GET /api/export/competitions`

Lists competitions the API key's owner has.

**Response 200:**
```json
[
  {
    "id": "<uuid>",
    "name": "...",
    "date": "2026-06-12",
    "mode": "loeschangriff",
    "sync_mode": "live",
    "is_active": true,
    "current_run_id": "<uuid|null>"
  }
]
```

#### `GET /api/export/competitions/:id`

Full competition snapshot. Same JSON shape as the inner payload of a `.kido` file (§2.2), **unsigned** (no envelope). Polled in live mode; called once at the start of an offline session.

**Response 200:** the `CompetitionPayload` from §2.2.

**Errors:** `404` if not owned; `403` if owned by someone else (defence in depth — shouldn't happen).

#### `POST /api/export/competitions/:id/runs/:run_id/result`

Idempotent live timing update. Call whenever a run's timing state changes (start, finish, cancel). Server preserves `correction_ms`, `team_id`, `runner_id`, `lane`.

**Request:**
```json
{
  "run_number": 1,
  "status": "completed",
  "original_time_ms": 47230,
  "started_at": "2026-06-12T10:00:12Z",
  "ended_at":   "2026-06-12T10:00:59Z"
}
```

`run_number` must equal the stored value for `run_id` (sanity check — DDB sort key is composite). `started_at` / `ended_at` may be `null` for non-completed states. `original_time_ms` may be `null` if the timer hasn't produced a final time yet.

**Response 200:** the updated `Run` row.

**Errors:**
- `409 conflict` if `competition.sync_mode != "live"`
- `400 validation` if `run_number` doesn't match the stored row
- `404 not_found` if run id doesn't exist on this competition

### 3.3 Endpoints (browser, session cookie)

These are listed for completeness — they're not called by the desktop, but the desktop developer may want to know what the operator does in the UI between desktop sessions.

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/api/competitions/:id/sync-mode` | `{ "sync_mode": "live"\|"offline" }` |
| `GET` | `/api/competitions/:id/export-file` | Download signed `.kido` (browser-only) |
| `POST` | `/api/competitions/:id/import-results` | Upload signed envelope. Body is the raw envelope JSON. Only when `sync_mode = "offline"`. Returns `{ runs_imported, penalties_imported, runs_skipped, penalties_skipped }`. |

The operator is expected to download the prepared file from web → hand to desktop, and the produced result file from desktop → upload via web. The desktop never POSTs back to `/api/competitions/:id/import-results` itself.

---

## 4. End-to-end flows

### 4.1 Live mode

```
Web (organizer)                     Desktop                          Web API
─────────────────                   ────────                         ───────────
Create comp (UI)
Set sync_mode=live (default)
Activate comp
Set current_run via UI ────────────────────────────► (read on next poll)

                                    GET /me                         ← whoami
                                    GET /me/hmac-key                ← cache key
                                    GET /competitions               ← list
                                    GET /competitions/:id (poll 1-5s)
                                                                    ← full snapshot

                                    Timer fires; run completes
                                    POST /competitions/:id/runs/:run_id/result
                                      { run_number, status: completed,
                                        original_time_ms, started_at, ended_at }
                                                                    → 200 Run

Decider page polls          ──────► visible in CurrentRunBanner + RunsList
Competition detail polls    ──────► visible in RunsList
```

**Recommended desktop polling cadence:** 1–2 s while a run is *active*, 5 s otherwise. Lambda + API Gateway handles this volume without trouble.

### 4.2 Offline mode

```
Web (organizer)                              Desktop                        File system
─────────────────                            ────────                       ──────────
1. Create / prepare comp on web
2. Switch sync_mode → offline
   (deciders auto-disable, web shows offline stub)
3. Click "Datei exportieren"
   GET /api/competitions/:id/export-file
   ─────────────────────────────────────────────────────────────────►  kido-<slug>.kido
                                                                       (signed envelope)

4. Operator hands file to desktop machine
                                             "Open .kido" / drag-drop
                                             Verify envelope.sig with cached HMAC key
                                             Decode payload, hydrate local state

5. Run competition on desktop
                                             For each run, capture original_time_ms
                                             Operator decides penalties locally
                                             (decider_slot can be set arbitrarily; web
                                              treats them as approved on import)

6. "Export results" in desktop
                                             Build CompetitionPayload (existing IDs
                                             preserved + new runs + approved_penalties)
                                             Sign with HMAC key → write .kido
                                                                   ◄────  kido-result.kido

7. Operator hands file back to web
8. Click "Datei importieren" on web
   POST /api/competitions/:id/import-results
   body = the envelope JSON
   Web verifies sig, applies runs + penalties
   Returns counts.
```

### 4.3 Switching between modes

| From | To | What gets blocked / unblocked |
|---|---|---|
| live → offline | | Decider POSTs return 409. Live ingest returns 409. Result-file import unblocked. |
| offline → live | | Decider POSTs allowed again. Live ingest unblocked. Result-file import blocked. Existing imported runs are kept. |

The web does **not** wipe data on mode switch — switching is metadata-only.

---

## 5. Implementation hints for the desktop side

- **Verify the envelope before you display anything.** A bad sig means the file is from a different account or has been tampered with — surface it to the operator, don't silently proceed.
- **Cache the HMAC key in the OS keychain**, not in plaintext on disk. It's effectively as sensitive as the API key (in fact, both together let you forge offline files for this user).
- **Don't trust your local clock for `correction_ms` math.** The web is the authority for penalties (they're either decided online by deciders or stamped `approved` on import).
- **Idempotency:** the live ingest endpoint is keyed on `run_id` and is idempotent. If a POST times out, retry — the server will simply overwrite with the same values.
- **Schema version check first.** When loading any payload (file, `GET /api/export/disciplines/:id`, every poll), bail if `schema_version != 4` — probe the version before the full parse so old payloads yield the version message, not a parse error. New fields land additively under `schema_version: 4` for non-breaking changes; a v5 will mean a coordinated release.

---

## 6. Open / future

- **HMAC key rotation UI** is not yet exposed. When added, rotation invalidates outstanding `.kido` files; operators will need to re-export.
- **Reason-code vocabulary** for penalties is currently free-text. Will be tightened later; desktop should accept any string for now.
- **Active-run hand-off:** today the organizer sets `current_run_id` in the web UI, and the desktop polls. If we later want desktop-driven "next run" advancement, we'll add `POST /api/export/competitions/:id/active-run`.
