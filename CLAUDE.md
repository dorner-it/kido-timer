# CLAUDE.md

This file is the source of truth for Claude Code when working on this repository. Read it fully before generating, scaffolding, or modifying code.

---

## 1. Project Overview

**Name:** KI-DO Timer (`kido-timer` monorepo: web backend, web frontend, desktop app)
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
| Auth (organizer) | AWS Cognito User Pool + Hosted UI; backend validates Cognito RS256 JWTs against JWKS and stores the session in httpOnly cookies |
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
kido-timer/
├── CLAUDE.md                  # This file
├── desktop/                   # Tauri 2 desktop app (serial timer console), imported from kido-timer-app
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
- `id`: Cognito `sub` (UUID-shaped). The web app does NOT store passwords — Cognito owns identity.
- `email`: lowercased, mirrored from Cognito's `email` claim on first sign-in for query convenience.
- `display_name`: from Cognito's `name` attribute.
- `role`: `Organizer` (only role for now; reserve for future).
- `api_keys`: list of long-lived keys for the desktop app (Argon2id-hashed in DB; only the prefix is shown after creation). Desktop API keys are independent of Cognito and live only in DynamoDB.
- `created_at`, `updated_at`

**Competition**
- `id`: UUID
- `owner_id`: User id (Cognito `sub`)
- `name`: String
- `date`: `NaiveDate`
- `location`: Option<String>
- `mode`: `TimerMode` (see 4.2)
- `is_active`: bool — exactly **zero or one** competition per user is active at a time. The decider page reads the active competition.
- `current_run_id`: Option<UUID> — points at the `Run` currently on the lane (set by organizer)
- `decider_tokens`: array of 4 opaque tokens, one per slot. Used in QR URLs. Rotatable.
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

**Run**
- `id`: UUID — one row per timing result (matches the desktop app's `RunEntry` concept: a confirmed time on a lane).
- `competition_id`: UUID
- `team_id`: UUID
- `runner_id`: Option<UUID> (typically `None` — the team runs as a unit on both v3 modes)
- `run_number`: u32 (1-indexed; e.g. heat 1, heat 2 of the same team)
- `lane`: Option<u8> (1..=`TimerMode.lane_count()`; 1..=2 for Gruppenstaffette, 1..=4 for Löschangriff)
- `status`: enum `Pending | Active | Completed | Cancelled`. Exactly zero or one `Active` run per competition.
- `original_time_ms`: Option<u32> — set by the desktop app on result import or manually by the organizer.
- `correction_ms`: i32 — denormalized sum of approved penalties' `delta_ms`, kept on the run row for fast display.
- `started_at`, `ended_at`: Option<timestamp>
- `created_at`, `updated_at`

**Penalty**
- `id`: UUID
- `competition_id`: UUID (denormalized for query convenience)
- `run_id`: UUID — penalties always attach to a specific Run (a team running heat 1 vs heat 2 has separate penalty buckets).
- `team_id`: UUID (denormalized from the run for query convenience).
- `runner_id`: Option<UUID> (inherited from the run).
- `decider_slot`: u8 (1..=4)
- `delta_ms`: i32 — milliseconds added to the run's correction. Negative values are allowed for organizer corrections. Maps directly onto the desktop app's `RunEntry.correctionMs`.
- `reason_code`: Option<String> (controlled vocabulary, configurable later — start with free-text optional)
- `status`: enum `Pending | Approved | Rejected`
- `created_at`: timestamp
- `reviewed_at`: Option<timestamp>
- `reviewed_by`: Option<String> (Cognito `sub` of the organizer)

### 4.2 TimerMode

v3 (current). Discipline-specific — lane count is intrinsic to the variant.

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerMode {
    Gruppenstaffette,  // 2 lanes, group-vs-group relay format
    Loeschangriff,     // 4 lanes, classic fire-attack discipline
}
```

The v1 generic set (`SingleLane | TwoLaneParallel | Relay | Individual`)
is gone; old strings are rejected with `400 validation`. See
`docs/migration-timer-mode.md` for the in-place DDB rewrite.

Adding a variant requires a coordinated release with the desktop app.

---

## 5. DynamoDB Single-Table Design

**Table name:** `ki-do-timer` (suffixed per environment: `ki-do-timer-dev`, `ki-do-timer-prod`).

**Primary key:** `PK` (String, partition), `SK` (String, sort).

**GSI1:** `GSI1PK`, `GSI1SK` — for owner→competitions lookup and penalty status queries.

### 5.1 Item Shapes

| Entity              | PK                          | SK                                  | GSI1PK                       | GSI1SK                        | Attributes                                |
|---------------------|-----------------------------|-------------------------------------|------------------------------|-------------------------------|-------------------------------------------|
| User                | `USER#<sub>`                | `PROFILE`                           | `USER_EMAIL#<email>`         | `USER#<sub>`                  | sub, email, display_name…                 |
| User API key        | `USER#<sub>`                | `APIKEY#<key_id>`                   | `APIKEY#<hash_prefix>`       | `USER#<sub>`                  | hashed_key, created_at, last_used_at      |
| Competition         | `COMP#<id>`                 | `META`                              | `USER#<owner_sub>`           | `COMP#<date>#<id>`            | name, date, mode, is_active, current_run_id, … |
| Team                | `COMP#<id>`                 | `TEAM#<start_number>#<team_id>`     | —                            | —                             | name, start_number                        |
| Runner              | `COMP#<id>`                 | `RUNNER#<team_id>#<position>`       | —                            | —                             | first_name, last_name, birth_date         |
| Run                 | `COMP#<id>`                 | `RUN#<run_number>#<id>`             | `COMP#<id>#RUN`              | `STATUS#<status>#<run_number>`| team_id, runner_id, lane, original_time_ms, correction_ms, started_at, ended_at |
| Penalty             | `COMP#<id>`                 | `PENALTY#<created_at>#<id>`         | `COMP#<id>#PENALTY`          | `STATUS#<status>#<created_at>`| run_id, team_id, runner_id, decider_slot, delta_ms, reason_code |
| OAuth state         | `OAUTH_STATE#<state>`       | `STATE`                             | —                            | —                             | code_verifier, redirect_after, ttl        |
| Active marker       | `USER#<sub>`                | `ACTIVE_COMP`                       | —                            | —                             | competition_id (single item per user)     |

### 5.2 Access Patterns

- Cognito callback / first sign-in: `GetItem PK = USER#<sub>, SK = PROFILE`; create on first hit.
- Lookup user by email (admin tooling): `Query GSI1 PK = USER_EMAIL#<email>` → 1 item.
- List competitions for organizer: `Query GSI1 PK = USER#<owner_sub>, SK begins_with COMP#`.
- Load full competition (meta + teams + runners + runs + penalties): `Query PK = COMP#<id>` (one query, partition by SK prefix).
- List pending penalties: `Query GSI1 PK = COMP#<id>#PENALTY, SK begins_with STATUS#Pending#`.
- Decider's "what's active": Read `USER#<sub>/ACTIVE_COMP`, then `COMP#<id>/META`, then resolve `current_run_id` to the active run row.
- OAuth state during Cognito callback: `GetItem PK = OAUTH_STATE#<state>` (TTL-expired entries are auto-purged by DDB).
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

### 6.1 Auth (organizer, Cognito)

The backend mediates the Cognito Hosted UI Authorization Code flow with PKCE. Tokens never reach the browser — the backend stores them server-side (or sets opaque httpOnly session cookies signed by the API).

| Method | Path                       | Notes                                                                 |
|--------|----------------------------|-----------------------------------------------------------------------|
| GET    | `/api/auth/login`          | Generate state + PKCE verifier (TTL'd in DDB), 302 to Cognito Hosted UI. |
| GET    | `/api/auth/callback`       | Cognito redirects here with `?code&state`. Backend validates state, exchanges code for tokens with the Cognito client, sets httpOnly cookies, 302 to frontend. |
| POST   | `/api/auth/logout`         | Clear cookies, revoke Cognito refresh token, 302 to Cognito logout endpoint, which redirects to the configured frontend logout URL. |
| GET    | `/api/auth/me`             | Validates the session cookie's access token against Cognito JWKS and returns the current user.                          |

User onboarding is **manual via the Cognito console or AWS CLI** (4 organizers max). Self-service signup is intentionally disabled at the user-pool level.

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

### 7.1 Organizer Identity (Cognito)

- **AWS Cognito User Pool** owns identity. Web app holds zero password material.
- **Hosted UI** in German (`Cognito > User pool > App integration > Hosted UI customisation`) handles sign-in, password reset, and lockout.
- **Authorization Code with PKCE.** The backend Lambda is a confidential client (has a client secret, read at cold start via `cognito-idp:DescribeUserPoolClient`). It exchanges the code for `id_token` + `access_token` + `refresh_token`, validates the `id_token` against the user pool's JWKS, and persists tokens server-side keyed on a session id.
- **Session cookie:** opaque session id, **httpOnly, Secure, SameSite=Lax** (Lax so Cognito's redirect back works), max-age = id-token lifetime.
- **Token lifetimes** (set on the Cognito user pool client): access = id = 60 min, refresh = 30 d.
- **Refresh** is performed transparently by the backend when an inbound request's session has an expired access token but a still-valid refresh token.
- **Onboarding:** create users in the Cognito console / CLI; the user pool is configured with `allow_admin_create_user_only = true` and self-service signup disabled.

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

| Source                     | Purpose                                                                |
|----------------------------|------------------------------------------------------------------------|
| Env var                    | `TABLE_NAME`, `AWS_REGION`, `LOG_LEVEL`, `ENV` (dev/prod)              |
| Env var                    | `COGNITO_USER_POOL_ID`, `COGNITO_CLIENT_ID`, `COGNITO_DOMAIN`, `COGNITO_REDIRECT_URI`, `FRONTEND_BASE_URL` |
| Cognito API at cold start  | `cognito-idp:DescribeUserPoolClient` → fetches the client secret      |
| Cognito JWKS at runtime    | `https://cognito-idp.<region>.amazonaws.com/<pool-id>/.well-known/jwks.json` (cached) |

**No JWT signing keys or refresh peppers** — Cognito issues tokens, we only verify them. Desktop API keys are still stored Argon2id-hashed in DynamoDB.

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

- **Same AWS account and region as `../fms/`.** Terraform reuses the existing state bucket `dorner-it-fms-terraform-state` (key `ki-do-timer/terraform.tfstate`) and lock table `fms-terraform-locks`, and the existing GitHub OIDC provider in the account.
- Terraform provisions: DynamoDB table + GSI1, Cognito User Pool + App Client + Hosted UI domain, Lambda function (ARM64, `provided.al2023`), API Gateway HTTP API, CloudWatch Log Group, IAM least-privilege role, two GitHub Actions IAM roles (`ki-do-timer-github-actions` for Lambda deploy, `ki-do-timer-github-actions-terraform` for plan/apply).
- Lambda binary built with `cargo lambda build --release --arm64 --output-format zip`.
- **CI:** `.github/workflows/terraform.yaml` plans/applies on `infrastructure/**` changes. `.github/workflows/deploy-lambda.yaml` builds + uploads the Lambda zip on `backend/**` changes via `aws lambda update-function-code` (Terraform does not redeploy code, it only manages the function shape). Mirrors `../fms/` patterns.
- **One-time bootstrap:** the two GitHub Actions roles are themselves declared in this Terraform stack, so the very first apply must be run locally with admin AWS creds. After that GitHub Actions takes over.
- Frontend deployed to Vercel; the project ships a `vercel.json` with the API base URL as an env var per environment.
- CORS: API Gateway allows the Vercel domain(s) only. No `*`.

---

## 12. Standalone Desktop App Integration Contract

The desktop app at ki-do.it expects to call `/api/export/competitions/:id` and receive:

```jsonc
{
  "schema_version": 3,
  "competition": {
    "id": "…",
    "name": "…",
    "date": "2026-06-12",
    "mode": "loeschangriff",
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

## 14. Resolved Decisions and Remaining Open Questions

### Resolved

- **Auth:** AWS Cognito User Pool + Hosted UI. The web app holds zero password material. Confirmed 2026-05-02.
- **TimerMode:** v3 set, discipline-specific — `Gruppenstaffette` (2 lanes) and `Loeschangriff` (4 lanes); see §4.2 and `docs/migration-timer-mode.md`. Old v1 strings are rejected with `400 validation`.
- **Penalty granularity:** penalties attach to a `Run` row (a team running multiple heats has separate penalty buckets). Run mirrors the desktop app's `RunEntry` (`originalTimeMs` + `correctionMs`).
- **AWS account & infra:** same account as `../fms/` (eu-central-1, state bucket `dorner-it-fms-terraform-state`, lock table `fms-terraform-locks`).
- **CI:** Terraform plan/apply runs from GitHub Actions via OIDC. Lambda deploy runs from GitHub Actions on `backend/**` changes.
- **GitHub repo:** `dorner-it/kido-timer` (public monorepo since 2026-10-16; merged from `dorner-it/ki-do-timer-web` + `dorner-it/kido-timer-app`, both histories preserved). Desktop releases are tagged `desktop-vX.Y.Z` and published here; the Tauri updater reads `releases/latest/download/latest.json` of this repo.

### Still open

- **Reason-code vocabulary** for penalties (free-text in v1; replace with controlled vocabulary later).
- **Vercel domain(s)** for CORS and Cognito callback/logout URLs (`callback_urls` / `logout_urls`) — needed before first prod apply.
- **Cognito Hosted UI domain prefix** — must be globally unique within the AWS region (e.g. `auth-ki-do-it`). Required input before first apply.
- **Whether the desktop app fetches once and caches, or polls during the event** (affects cache headers and ETag use).
