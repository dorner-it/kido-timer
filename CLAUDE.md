# CLAUDE.md

This file is the source of truth for Claude Code when working on this repository. Read it fully before generating, scaffolding, or modifying code.

---

## 1. Project Overview

**Name:** KI-DO Timer Web (`ki-do-timer-web`)
**Domain:** ki-do.it
**Purpose:** Web application to prepare and operate **TRV Kocab timer** competitions for fire-department running disciplines (Löschangriff, Staffellauf, etc.).

This web app is the **organizer-side companion** to the existing standalone desktop application (the "ki-do.it timer app"), which:

- Connects to the TRV Kocab timer over a reverse-engineered RS232 protocol.
- Receives live times during a run.
- Supports several timer modes (single lane, two-lane parallel, relay, individual).

The web app does **not** talk to the timer hardware directly. It is responsible for:

1. Letting an organizer (the "Configurator") prepare competitions: teams, runners, modes.
2. Letting up to 4 unauthenticated **Penalty Deciders** (Strafrichter) record penalties from a tablet/phone via a static QR code, on the currently active run.
3. Letting the organizer review, approve, or reject submitted penalties.
4. Exposing the prepared competition data (and approved penalties) to the standalone desktop app via an API-key-authenticated endpoint.

---

## 2. Tech Stack (locked)

| Layer            | Choice                                                                |
|------------------|-----------------------------------------------------------------------|
| Backend language | Rust (stable, edition 2024)                                           |
| HTTP framework   | `axum` 0.8                                                            |
| Lambda adapter   | `lambda_http` (via `cargo-lambda`)                                    |
| Build target     | `aarch64-unknown-linux-musl` (ARM64 / Graviton)                       |
| Async runtime    | `tokio` (multi-thread flavor)                                         |
| Frontend         | Astro 4+ with React 18 islands                                        |
| Styling          | Tailwind CSS                                                          |
| Forms            | `react-hook-form` + `zod`                                             |
| Database         | DynamoDB, single-table design                                         |
| Secrets          | AWS Secrets Manager (no environment-stored secrets in production)     |
| Auth (organizer) | JWT (HS256), refresh tokens, Argon2id-hashed passwords                |
| Auth (deciders)  | Per-slot static bearer token in QR URL (rotatable)                    |
| Auth (desktop)   | Long-lived API key per installation, stored in DynamoDB               |
| Hosting (API)    | API Gateway HTTP API → single Lambda                                  |
| Hosting (web)    | Vercel (static + SSR off, talks to API via fetch)                     |
| IaC              | Terraform                                                             |
| Logging          | `tracing` + `tracing-subscriber` (JSON to CloudWatch)                 |

**Do not introduce alternative frameworks (e.g., Actix, Rocket, Next.js) without an explicit instruction.**

---

## 3. Repository Layout

Scaffold the project as a monorepo:

```
ki-do-timer-web/
├── CLAUDE.md                  # This file
├── README.md                  # Quickstart for humans
├── .gitignore
├── .editorconfig
├── Makefile                   # Top-level convenience targets
│
├── backend/
│   ├── Cargo.toml             # Single crate, not a workspace (initially)
│   ├── Cargo.lock
│   ├── rust-toolchain.toml    # Pin stable channel
│   ├── src/
│   │   ├── main.rs            # Lambda entrypoint
│   │   ├── lib.rs             # Re-exports for tests
│   │   ├── app.rs             # axum Router construction
│   │   ├── config.rs          # Env + Secrets Manager loading
│   │   ├── error.rs           # AppError + IntoResponse
│   │   ├── auth/
│   │   │   ├── mod.rs
│   │   │   ├── jwt.rs         # Sign/verify, refresh logic
│   │   │   ├── password.rs    # Argon2id hash/verify
│   │   │   ├── extractor.rs   # axum extractor for AuthedUser
│   │   │   └── decider.rs     # Decider token verification
│   │   ├── db/
│   │   │   ├── mod.rs         # DynamoDB client wrapper
│   │   │   ├── keys.rs        # PK/SK builders, GSI builders
│   │   │   ├── competitions.rs
│   │   │   ├── teams.rs
│   │   │   ├── runners.rs
│   │   │   ├── penalties.rs
│   │   │   └── users.rs
│   │   ├── models/
│   │   │   ├── mod.rs
│   │   │   ├── competition.rs
│   │   │   ├── team.rs
│   │   │   ├── runner.rs
│   │   │   ├── penalty.rs
│   │   │   ├── user.rs
│   │   │   └── timer_mode.rs  # Mirrors modes in standalone app
│   │   └── routes/
│   │       ├── mod.rs
│   │       ├── auth.rs        # /api/auth/*
│   │       ├── competitions.rs
│   │       ├── teams.rs
│   │       ├── runners.rs
│   │       ├── penalties.rs   # Organizer review endpoints
│   │       ├── decider.rs     # Unauthenticated /penalty/...
│   │       └── export.rs      # /api/export/* for desktop app
│   └── tests/
│       ├── integration.rs
│       └── helpers/
│
├── frontend/
│   ├── package.json
│   ├── astro.config.mjs
│   ├── tailwind.config.cjs
│   ├── tsconfig.json
│   ├── public/
│   ├── src/
│   │   ├── pages/
│   │   │   ├── index.astro              # Login redirect
│   │   │   ├── login.astro
│   │   │   ├── competitions/
│   │   │   │   ├── index.astro          # List
│   │   │   │   ├── [id].astro           # Detail / edit
│   │   │   │   └── new.astro
│   │   │   ├── penalties/[id].astro     # Organizer review queue
│   │   │   └── decider/[slot].astro     # PUBLIC penalty entry page
│   │   ├── components/
│   │   │   ├── react/                   # React islands
│   │   │   │   ├── TeamEditor.tsx
│   │   │   │   ├── RunnerForm.tsx
│   │   │   │   ├── PenaltyReviewList.tsx
│   │   │   │   ├── DeciderPad.tsx
│   │   │   │   └── ActiveRunIndicator.tsx
│   │   │   └── astro/                   # Static Astro components
│   │   ├── layouts/
│   │   ├── lib/
│   │   │   ├── api.ts                   # Typed fetch client
│   │   │   ├── auth.ts                  # Token storage (httpOnly cookie helpers)
│   │   │   └── types.ts                 # Shared TS types (mirror backend)
│   │   └── styles/
│   └── tests/
│
├── infrastructure/
│   ├── main.tf
│   ├── variables.tf
│   ├── outputs.tf
│   ├── providers.tf
│   ├── backend.tf            # Remote state
│   └── modules/
│       ├── api/              # API Gateway + Lambda + IAM
│       ├── dynamodb/         # Single table + GSIs
│       └── secrets/          # JWT signing key, etc.
│
└── docs/
    ├── api.md                # Endpoint reference
    ├── data-model.md         # DynamoDB single-table layout
    └── decider-flow.md       # QR / penalty flow
```

---

## 4. Domain Model

### 4.1 Entities

**User (Configurator / Organizer)**
- `id`: UUID v4
- `email`: lowercased, unique
- `password_hash`: Argon2id
- `display_name`: String
- `role`: `Organizer` (only role for now; reserve for future)
- `api_keys`: list of long-lived keys for the desktop app (hashed in DB; only the prefix is shown after creation)
- `created_at`, `updated_at`

**Competition**
- `id`: UUID
- `owner_id`: UUID (User)
- `name`: String
- `date`: `NaiveDate`
- `location`: Option<String>
- `mode`: `TimerMode` (see 4.2)
- `is_active`: bool — exactly **zero or one** competition per user is active at a time. The decider page reads the active competition.
- `current_run`: Option<RunRef> — which team/runner is currently on the lane (set by organizer)
- `decider_tokens`: array of 4 opaque tokens, one per slot. Used in QR URLs. Rotatable.
- `desktop_api_keys`: list of keys with which the standalone app can pull this competition's data. (May also be inherited from the User's keys — discuss in PR.)
- `created_at`, `updated_at`

**Team**
- `id`: UUID
- `competition_id`: UUID
- `name`: String
- `start_number`: u32 (running order)

**Runner**
- `id`: UUID
- `team_id`: UUID
- `competition_id`: UUID (denormalized for query convenience)
- `first_name`: String
- `last_name`: String
- `birth_date`: `NaiveDate`
- `position`: u32 (running order within team)

**Penalty**
- `id`: UUID
- `competition_id`: UUID
- `team_id`: UUID
- `runner_id`: Option<UUID> (penalties are typically per-team but optionally per-runner)
- `decider_slot`: u8 (1..=4)
- `delta`: i8 (typically +1; allow negative for corrections)
- `reason_code`: Option<String> (controlled vocabulary, configurable later — start with free-text optional)
- `status`: enum `Pending | Approved | Rejected`
- `created_at`: timestamp
- `reviewed_at`: Option<timestamp>
- `reviewed_by`: Option<UUID>

### 4.2 TimerMode

Mirror the modes already implemented in the standalone ki-do.it timer desktop app. The web app does not need to *interpret* them — it stores the mode on the competition so the desktop app picks the right behavior on import.

```rust
pub enum TimerMode {
    SingleLane,
    TwoLaneParallel,
    Relay,
    Individual,
    // Extend to match the desktop app exactly. Treat the desktop app as authoritative.
}
```

When unsure which variants exist, **ask the user** — do not invent variants.

---

## 5. DynamoDB Single-Table Design

**Table name:** `ki-do-timer` (suffixed per environment: `ki-do-timer-dev`, `ki-do-timer-prod`).

**Primary key:** `PK` (String, partition), `SK` (String, sort).

**GSI1:** `GSI1PK`, `GSI1SK` — for owner→competitions lookup and penalty status queries.

### 5.1 Item Shapes

| Entity              | PK                          | SK                                 | GSI1PK                       | GSI1SK                        | Attributes                                |
|---------------------|-----------------------------|------------------------------------|------------------------------|-------------------------------|-------------------------------------------|
| User                | `USER#<email>`              | `PROFILE`                          | `USER#<id>`                  | `PROFILE`                     | id, email, password_hash, display_name…   |
| User API key        | `USER#<id>`                 | `APIKEY#<key_id>`                  | `APIKEY#<hash_prefix>`       | `USER#<id>`                   | hashed_key, created_at, last_used_at      |
| Competition         | `COMP#<id>`                 | `META`                             | `USER#<owner_id>`            | `COMP#<date>#<id>`            | name, date, mode, is_active, …            |
| Team                | `COMP#<id>`                 | `TEAM#<start_number>#<team_id>`    | —                            | —                             | name, start_number                        |
| Runner              | `COMP#<id>`                 | `RUNNER#<team_id>#<position>`      | —                            | —                             | first_name, last_name, birth_date         |
| Penalty             | `COMP#<id>`                 | `PENALTY#<created_at>#<id>`        | `COMP#<id>#PENALTY`          | `STATUS#<status>#<created_at>`| team_id, runner_id, decider_slot, delta…  |
| Active marker       | `USER#<id>`                 | `ACTIVE_COMP`                      | —                            | —                             | competition_id (single item per user)     |

### 5.2 Access Patterns

- Login: `Query PK = USER#<email>` → 1 item.
- List competitions for organizer: `Query GSI1 PK = USER#<owner_id>, SK begins_with COMP#`.
- Load full competition (meta + teams + runners + penalties): `Query PK = COMP#<id>` (one query, sort by SK prefix).
- List pending penalties: `Query GSI1 PK = COMP#<id>#PENALTY, SK begins_with STATUS#Pending#`.
- Decider's "what's active": Read `USER#<id>/ACTIVE_COMP`, then read `COMP#<id>/META`. The decider URL contains the user-id and slot token, so we know which user.
- Desktop export: `Query PK = COMP#<id>` and filter for approved penalties only.

**No scans in production code paths.** All access patterns must be Query or GetItem.

---

## 6. API Surface

All routes are mounted under the API Gateway HTTP API. Single Lambda, axum routes the rest.

Conventions:
- JSON request/response bodies, `Content-Type: application/json`.
- Errors: `{ "error": { "code": "snake_case", "message": "human readable" } }` with appropriate HTTP status.
- Timestamps: RFC 3339 in UTC.
- IDs: UUID v4 as canonical lowercase string.
- Auth header for organizer: `Authorization: Bearer <jwt>`.
- Auth header for desktop: `X-API-Key: <key>`.
- Decider token: in URL path (see 6.3).

### 6.1 Auth (organizer)

| Method | Path                  | Body / Notes                                          |
|--------|-----------------------|-------------------------------------------------------|
| POST   | `/api/auth/login`     | `{ email, password }` → `{ access_token, refresh_token, user }` |
| POST   | `/api/auth/refresh`   | `{ refresh_token }` → new pair                        |
| POST   | `/api/auth/logout`    | Invalidates refresh token (deny-list in DDB).         |
| GET    | `/api/auth/me`        | Returns current user.                                 |

User registration is **not** a public endpoint in v1. Bootstrap an organizer via a Terraform-invoked one-shot script or a CLI subcommand `cargo run -- create-user`.

### 6.2 Configurator (organizer, JWT)

| Method | Path                                                        | Purpose                              |
|--------|-------------------------------------------------------------|--------------------------------------|
| GET    | `/api/competitions`                                         | List own competitions                |
| POST   | `/api/competitions`                                         | Create                               |
| GET    | `/api/competitions/:id`                                     | Full competition (teams, runners)    |
| PUT    | `/api/competitions/:id`                                     | Update name/date/mode/location       |
| DELETE | `/api/competitions/:id`                                     | Delete (cascades teams/runners)      |
| POST   | `/api/competitions/:id/activate`                            | Mark active (deactivates others)     |
| POST   | `/api/competitions/:id/current-run`                         | Set currently-running team           |
| POST   | `/api/competitions/:id/decider-tokens/rotate`               | Regenerate all 4 slot tokens         |
| POST   | `/api/competitions/:id/teams`                               | Add team                             |
| PUT    | `/api/competitions/:id/teams/:team_id`                      | Update team                          |
| DELETE | `/api/competitions/:id/teams/:team_id`                      | Delete team                          |
| POST   | `/api/competitions/:id/teams/:team_id/runners`              | Add runner                           |
| PUT    | `/api/competitions/:id/teams/:team_id/runners/:runner_id`   | Update runner                        |
| DELETE | `/api/competitions/:id/teams/:team_id/runners/:runner_id`   | Delete runner                        |
| GET    | `/api/competitions/:id/penalties?status=pending`            | List penalties                       |
| POST   | `/api/competitions/:id/penalties/:penalty_id/approve`       | Approve                              |
| POST   | `/api/competitions/:id/penalties/:penalty_id/reject`        | Reject (with optional reason)        |

### 6.3 Decider (unauthenticated, slot token in URL)

The QR code points at the **frontend** page, e.g.:

```
https://app.ki-do.it/decider/<owner_user_id>/<slot>/<slot_token>
```

The frontend reads owner/slot/token, then talks to:

| Method | Path                                                                        | Purpose                          |
|--------|-----------------------------------------------------------------------------|----------------------------------|
| GET    | `/api/decider/:owner_id/:slot/:token/state`                                 | Active competition + current run |
| POST   | `/api/decider/:owner_id/:slot/:token/penalties`                             | Submit a penalty                 |
| DELETE | `/api/decider/:owner_id/:slot/:token/penalties/:penalty_id`                 | Withdraw own pending penalty     |

**Server-side validation:**
- `slot` must be in `1..=4`.
- `token` must equal `decider_tokens[slot - 1]` of the *currently active* competition for that user.
- Decider can only modify penalties they themselves submitted (matched by slot) and only while `status = Pending`.
- Aggressive rate limiting (e.g., 60 req/min per token) — implement as in-Lambda token bucket keyed on (token, source IP).

### 6.4 Desktop App Export (API key)

| Method | Path                                          | Purpose                                |
|--------|-----------------------------------------------|----------------------------------------|
| GET    | `/api/export/competitions`                    | List the API-key owner's competitions  |
| GET    | `/api/export/competitions/:id`                | Full competition snapshot (JSON)       |
| GET    | `/api/export/competitions/:id/penalties`      | Approved penalties only                |

Export JSON shape is **stable** and versioned via top-level `"schema_version": 1`. Document any change in `docs/api.md` and bump the version.

---

## 7. Authentication Details

### 7.1 Organizer JWT

- Algorithm: **HS256**. Key from AWS Secrets Manager (`ki-do-timer/jwt-signing-key`), rotated manually.
- Access token lifetime: **15 minutes**.
- Refresh token lifetime: **30 days**, single-use, stored hashed in DynamoDB on refresh issue; deny-listed on logout.
- Claims: `sub` (user id), `email`, `role`, `iat`, `exp`, `jti`.
- Frontend stores tokens in **httpOnly, Secure, SameSite=Strict** cookies set by the API.

### 7.2 Decider Tokens

- 32-byte URL-safe base64 random string per slot.
- Generated at competition creation; rotatable.
- Stored in plaintext on the competition item (they're already URL-distributable).
- **No personal data** is collected from deciders. The slot is the identity.

### 7.3 Desktop API Keys

- 40-byte URL-safe base64 random string. Shown **once** at creation.
- Stored in DDB as `argon2id(key)` plus a 6-char prefix for UI display.
- Bound to a User; can be revoked.

---

## 8. Configuration & Secrets

Backend Lambda reads at cold start:

| Source              | Purpose                                                       |
|---------------------|---------------------------------------------------------------|
| Env var             | `TABLE_NAME`, `AWS_REGION`, `LOG_LEVEL`, `ENV` (dev/prod)     |
| Secrets Manager     | `ki-do-timer/<env>/jwt-signing-key`                           |
| Secrets Manager     | `ki-do-timer/<env>/jwt-refresh-pepper` (HMAC pepper)          |

**Never** put signing keys, peppers, or API keys in env vars. Pattern matches the existing AWS Lambda Rust services in this user's stack.

---

## 9. Coding Conventions

### 9.1 Rust

- **Edition 2024**. Toolchain pinned via `rust-toolchain.toml`.
- `#![deny(warnings)]` not used; instead, CI runs `cargo clippy -- -D warnings`.
- Error handling: `thiserror` for typed errors in lib code, `anyhow` only in `main.rs` and tests.
- Error response: a single `AppError` enum implementing `IntoResponse` produces the JSON error body. Never let an `Err(_)` bubble into a 500 without a code.
- Logging: `tracing` exclusively. No `println!` / `eprintln!` outside `main.rs` startup.
- Use `aws-sdk-dynamodb` and `aws-sdk-secretsmanager` (official AWS Rust SDKs). Build clients **once** at cold start; reuse across invocations.
- DynamoDB items: convert via `serde_dynamo` to keep handler code clean.
- All public types implementing `Serialize`/`Deserialize` use `#[serde(rename_all = "snake_case")]`.
- No `unwrap()` / `expect()` in handler or DB code. They are acceptable in tests and in `main.rs` startup *only* with a clear panic message.
- Tests: prefer integration tests against a local DynamoDB (DynamoDB Local in Docker) over unit-mocking the AWS SDK.

### 9.2 TypeScript / Astro

- TypeScript strict mode (`"strict": true`, `"noUncheckedIndexedAccess": true`).
- React components: function components only, no class components.
- State: hooks only; consider `zustand` if shared state grows (but start without).
- Server data: a single typed `apiClient` in `src/lib/api.ts`; no ad-hoc `fetch` calls in components.
- Validation: `zod` schemas for every form and every API response parse.
- UI text: **German**. Code, identifiers, comments, commit messages: **English**.
- Date formatting: use the user's locale; default `de-DE`.

### 9.3 Commits & Branches

- Conventional Commits (`feat:`, `fix:`, `chore:`, `refactor:`, `docs:`, `test:`).
- Branch names: `feat/<short-slug>`, `fix/<short-slug>`.
- One logical change per PR; keep PRs reviewable.

### 9.4 Licensing

- Repository licence: **proprietary** (dorner-it.de). Add `LICENSE` file with proprietary notice. Add SPDX header `// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT` to all source files.

---

## 10. Build, Run, Test

Provide a top-level `Makefile` so a fresh contributor can:

```
make setup           # install toolchains, cargo-lambda, npm deps
make backend-dev     # cargo lambda watch
make backend-test    # cargo test (uses DynamoDB Local via docker compose)
make backend-build   # cargo lambda build --release --arm64
make frontend-dev    # astro dev
make frontend-build  # astro build
make frontend-test   # vitest
make lint            # cargo clippy + eslint + tsc --noEmit
make fmt             # cargo fmt + prettier
make tf-plan         # terraform plan
make tf-apply        # terraform apply
```

Provide a `compose.yaml` at repo root that runs DynamoDB Local on port 8000 for tests.

---

## 11. Deployment

- Terraform provisions: DynamoDB table + GSI1, Lambda function (ARM64, runtime `provided.al2023`), API Gateway HTTP API, Secrets Manager secrets, CloudWatch Log Group, IAM least-privilege role.
- Lambda binary built with `cargo lambda build --release --arm64 --output-format zip`.
- Frontend deployed to Vercel; the project ships a `vercel.json` with the API base URL as an env var per environment.
- CORS: API Gateway allows the Vercel domain(s) only. No `*`.

---

## 12. Standalone Desktop App Integration Contract

The desktop app at ki-do.it expects to call `/api/export/competitions/:id` and receive:

```jsonc
{
  "schema_version": 1,
  "competition": {
    "id": "…",
    "name": "…",
    "date": "2026-06-12",
    "mode": "TwoLaneParallel",
    "location": "…",
    "teams": [
      {
        "id": "…",
        "name": "…",
        "start_number": 1,
        "runners": [
          { "id": "…", "first_name": "…", "last_name": "…", "birth_date": "1998-04-22", "position": 1 }
        ]
      }
    ],
    "approved_penalties": [
      { "id": "…", "team_id": "…", "runner_id": null, "delta": 1, "reason_code": null, "approved_at": "…" }
    ]
  }
}
```

Treat this contract as **stable**. Any breaking change requires bumping `schema_version` and coordinating a desktop app release.

---

## 13. What Claude Code Should Do First

When invoked in an empty directory with this file present:

1. Confirm the layout in section 3 with the user before scaffolding (one-line summary + ask).
2. Generate the directory tree, `.gitignore`, `Makefile`, `compose.yaml`, `rust-toolchain.toml`, `LICENSE`, `README.md`.
3. Scaffold the backend crate with one healthcheck route (`GET /api/health` → `{ "ok": true }`) wired through `lambda_http`, building locally and as a Lambda zip.
4. Scaffold the Astro frontend with the login page calling a stubbed `/api/auth/login`.
5. Scaffold Terraform with DynamoDB + Lambda + API Gateway, but **do not apply** — produce `terraform plan` output for review.
6. Stop and ask before implementing auth, the decider flow, or the export endpoint — each is a discrete milestone.

---

## 14. Open Questions to Resolve With User Before Coding

- Exact list of `TimerMode` variants (mirror the desktop app — request a copy of its enum).
- Whether penalties are per-team or per-runner in v1 (current spec: per-team, with optional runner reference).
- Whether the desktop app fetches once and caches, or polls during the event (affects cache headers and ETag use).
- Whether organizer onboarding needs a self-service signup, or remains CLI-only.
- Reason-code vocabulary for penalties, or stay free-text in v1.

When in doubt: **ask, don't guess.**
