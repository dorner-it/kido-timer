# API Reference

The endpoint surface and conventions live in [`CLAUDE.md` §6 — API Surface](../CLAUDE.md#6-api-surface). This file will be expanded with concrete request/response shapes once each milestone (auth, decider flow, export) is implemented.

## Schema versioning

The desktop-app export contract is versioned via a top-level `"schema_version"` integer. Any breaking change to `/api/export/*` requires bumping this value and coordinating a desktop-app release. Current value: `1`.
