# Decider (Strafrichter) Flow

See [`CLAUDE.md` §6.3 — Decider (unauthenticated, slot token in URL)](../CLAUDE.md#63-decider-unauthenticated-slot-token-in-url) for the route surface and validation rules.

## QR URL shape

```
https://app.ki-do.it/decider/<owner_user_id>/<slot>/<slot_token>
```

The frontend reads the path params, then talks to `/api/decider/:owner_id/:slot/:token/...` on the backend. Slot must be `1..=4` and `slot_token` must match `decider_tokens[slot - 1]` of the user's currently active competition.

## Constraints

- No personal data is collected from deciders. The slot is the identity.
- Deciders may withdraw only their own pending submissions.
- Per-token rate limiting (e.g. 60 req/min) is implemented in-Lambda as a token bucket keyed on `(token, source IP)`.
