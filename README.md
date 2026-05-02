# KI-DO Timer Web

Web companion for **TRV Kocab timer** competitions used by fire-department running disciplines (Löschangriff, Staffellauf, …). The web app handles organizer-side preparation, penalty submission via static QR codes from up to four Strafrichter, and exposes an export feed consumed by the standalone ki-do.it desktop application.

The web app does **not** talk to the timer hardware. The desktop app remains the only consumer of the RS232 protocol.

## Quickstart

```sh
make setup              # rustup target + cargo-lambda + npm install

make backend-test       # run integration tests
make backend-build      # build Lambda zip (aarch64-unknown-linux-musl)
make backend-dev        # cargo lambda watch (local dev server, :9000)

make frontend-dev       # astro dev (:4321)
make frontend-build     # astro build → dist/
make frontend-check     # astro check + tsc --noEmit

make tf-fmt             # terraform fmt -recursive -check
make tf-validate        # terraform validate (requires tf-init first)

make dynamodb-up        # DynamoDB Local on :8000
make dynamodb-down
```

## Layout

- `backend/` — Rust + axum, deployed as a single AWS Lambda (ARM64 musl).
- `frontend/` — Astro 4 + React 18 islands + Tailwind, deployed to Vercel.
- `infrastructure/` — Terraform: DynamoDB single table, Lambda, API Gateway HTTP API, Secrets Manager, CloudWatch.
- `docs/` — API reference, single-table data model, decider flow.

The full project specification lives in [`CLAUDE.md`](./CLAUDE.md). It is the source of truth for the stack, domain model, API surface, and conventions.

## License

Proprietary. © Dorner IT (dorner-it.de). See [`LICENSE`](./LICENSE).
