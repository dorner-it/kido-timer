// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT
//
// Single-table DynamoDB key construction.
//
// **v2 layout (Event-scoped).** All event-bound rows share PK = `EVENT#<id>`
// so a single `Query PK = EVENT#<id>` returns the full day's data — meta,
// disciplines, teams, entries, runners, runs, penalties — in one round trip.
//
// Numeric components in sort keys are zero-padded so DDB's lexicographic
// sort matches the numeric order ("000010" sorts after "000002", not before).
//
// Sort-key prefixes:
//   META                                   — event metadata
//   DISC#<position:04>#<id>                — discipline
//   TEAM#<position:06>#<id>                — team
//   ENTRY#<disc_id>#<start:06>#<team_id>   — team_entry (meldung)
//   RUNNER#<team_id>#<position:04>#<id>    — runner (unique per runner;
//                                            legacy rows without `#<id>`
//                                            are still read, see
//                                            `event_sk_runner_legacy`)
//   RUN#<disc_id>#<run_number:04>#<id>     — run
//   PENALTY#<disc_id>#<id>                 — penalty

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

// ===== Partition keys =====

pub fn user_pk(sub: &str) -> String {
    format!("USER#{sub}")
}

pub fn user_email_gsi1pk(email: &str) -> String {
    format!("USER_EMAIL#{}", email.to_lowercase())
}

pub fn event_pk(id: Uuid) -> String {
    format!("EVENT#{id}")
}

pub fn penalty_status_gsi1pk(event_id: Uuid) -> String {
    format!("EVENT#{event_id}#PENALTY")
}

pub fn run_status_gsi1pk(event_id: Uuid) -> String {
    format!("EVENT#{event_id}#RUN")
}

pub fn user_owned_events_gsi1pk(owner_sub: &str) -> String {
    format!("USER#{owner_sub}")
}

// ===== Sort keys (constants) =====

pub const SK_PROFILE: &str = "PROFILE";
pub const SK_META: &str = "META";
pub const SK_ACTIVE_EVENT: &str = "ACTIVE_EVENT";

// ===== Sort keys (dynamic) =====

pub fn user_sk_apikey(key_id: &str) -> String {
    format!("APIKEY#{key_id}")
}

pub fn event_sk_discipline(position: u32, discipline_id: Uuid) -> String {
    format!("DISC#{position:04}#{discipline_id}")
}

pub fn event_sk_team(position: u32, team_id: Uuid) -> String {
    format!("TEAM#{position:06}#{team_id}")
}

pub fn event_sk_team_entry(discipline_id: Uuid, start_number: u32, team_id: Uuid) -> String {
    format!("ENTRY#{discipline_id}#{start_number:06}#{team_id}")
}

/// Runner sort key. The runner id is part of the key so two runners of
/// the same team can never share (and overwrite) an item — the position
/// alone is *not* unique (a move or a create at an occupied position
/// would otherwise clobber the other runner). Zero-padded position first
/// so `begins_with(SK, "RUNNER#<team_id>#")` still returns the roster in
/// position order.
pub fn event_sk_runner(team_id: Uuid, position: u32, runner_id: Uuid) -> String {
    format!("RUNNER#{team_id}#{position:04}#{runner_id}")
}

/// Pre-fix runner sort key (`RUNNER#<team_id>#<position:04>`, no id).
/// Only used to find and remove rows written before the key change —
/// never for new writes. Deletes against it must be conditional on the
/// runner id, since a legacy key identifies a *position*, not a runner.
pub fn event_sk_runner_legacy(team_id: Uuid, position: u32) -> String {
    format!("RUNNER#{team_id}#{position:04}")
}

/// Query prefix for one team's runners — matches new and legacy keys.
pub fn event_sk_runner_prefix_for_team(team_id: Uuid) -> String {
    format!("RUNNER#{team_id}#")
}

/// Query prefix for all runners of an event — matches new and legacy keys.
pub const SK_RUNNER_PREFIX: &str = "RUNNER#";

pub fn event_sk_run(discipline_id: Uuid, run_number: u32, run_id: Uuid) -> String {
    format!("RUN#{discipline_id}#{run_number:04}#{run_id}")
}

pub fn event_sk_run_prefix_for_discipline(discipline_id: Uuid) -> String {
    format!("RUN#{discipline_id}#")
}

pub fn event_sk_penalty(discipline_id: Uuid, penalty_id: Uuid) -> String {
    format!("PENALTY#{discipline_id}#{penalty_id}")
}

pub fn event_sk_penalty_prefix_for_discipline(discipline_id: Uuid) -> String {
    format!("PENALTY#{discipline_id}#")
}

pub fn user_events_gsi1sk(date: NaiveDate, event_id: Uuid) -> String {
    format!("EVENT#{date}#{event_id}")
}

pub fn run_status_gsi1sk(status: &str, discipline_id: Uuid, run_number: u32) -> String {
    format!("STATUS#{status}#{discipline_id}#{run_number:04}")
}

pub fn penalty_status_gsi1sk(status: &str, created_at: DateTime<Utc>) -> String {
    format!("STATUS#{status}#{}", created_at.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn user_pk_format() {
        assert_eq!(user_pk("abc-123"), "USER#abc-123");
    }

    #[test]
    fn email_gsi_lowercases() {
        assert_eq!(
            user_email_gsi1pk("Org@Example.DE"),
            "USER_EMAIL#org@example.de"
        );
    }

    #[test]
    fn team_sk_pads_position_for_lexsort() {
        let id = Uuid::nil();
        let small = event_sk_team(2, id);
        let large = event_sk_team(10, id);
        assert!(small < large, "{small} should sort before {large}");
        assert_eq!(small, "TEAM#000002#00000000-0000-0000-0000-000000000000");
    }

    fn uuid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn runner_sk_contains_team_position_and_id() {
        let team = uuid(1);
        let runner = uuid(0xabc);
        assert_eq!(
            event_sk_runner(team, 2, runner),
            format!("RUNNER#{team}#0002#{runner}")
        );
    }

    #[test]
    fn runner_sk_sorts_by_position_regardless_of_id() {
        let team = uuid(1);
        // A "large" id at a small position must still sort first.
        let p2 = event_sk_runner(team, 2, uuid(u128::MAX));
        let p10 = event_sk_runner(team, 10, uuid(0));
        let p1 = event_sk_runner(team, 1, uuid(u128::MAX));
        let mut keys = vec![p10.clone(), p2.clone(), p1.clone()];
        keys.sort();
        assert_eq!(keys, vec![p1, p2, p10]);
    }

    #[test]
    fn runner_sk_is_unique_for_same_position() {
        let team = uuid(1);
        let a = event_sk_runner(team, 2, uuid(10));
        let b = event_sk_runner(team, 2, uuid(11));
        assert_ne!(
            a, b,
            "two runners at the same position must not share a key"
        );
    }

    #[test]
    fn runner_sks_old_and_new_match_team_prefix_and_keep_position_order() {
        let team = uuid(1);
        let other_team = uuid(2);
        let prefix = event_sk_runner_prefix_for_team(team);
        let legacy1 = event_sk_runner_legacy(team, 1);
        let new1 = event_sk_runner(team, 1, uuid(7));
        let legacy2 = event_sk_runner_legacy(team, 2);
        let new2 = event_sk_runner(team, 2, uuid(3));
        for k in [&legacy1, &new1, &legacy2, &new2] {
            assert!(k.starts_with(&prefix), "{k} must match {prefix}");
            assert!(k.starts_with(SK_RUNNER_PREFIX));
        }
        assert!(!event_sk_runner(other_team, 1, uuid(7)).starts_with(&prefix));
        // Mixed old/new keys still come back grouped by position.
        let mut keys = vec![new2.clone(), legacy2.clone(), new1.clone(), legacy1.clone()];
        keys.sort();
        assert_eq!(keys, vec![legacy1, new1, legacy2, new2]);
    }

    #[test]
    fn legacy_runner_sk_differs_from_new() {
        let team = uuid(1);
        assert_eq!(
            event_sk_runner_legacy(team, 3),
            format!("RUNNER#{team}#0003")
        );
        assert_ne!(
            event_sk_runner_legacy(team, 3),
            event_sk_runner(team, 3, uuid(5))
        );
    }

    #[test]
    fn run_sk_groups_by_discipline_then_run_number() {
        let disc = Uuid::nil();
        let id = Uuid::nil();
        let first = event_sk_run(disc, 1, id);
        let tenth = event_sk_run(disc, 10, id);
        assert!(first < tenth, "{first} should sort before {tenth}");
        assert!(first.starts_with(&format!("RUN#{disc}#")));
    }

    #[test]
    fn penalty_status_gsi1sk_orders_by_created_at() {
        let earlier = Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0).unwrap();
        let later = Utc.with_ymd_and_hms(2026, 5, 1, 13, 0, 0).unwrap();
        let a = penalty_status_gsi1sk("pending", earlier);
        let b = penalty_status_gsi1sk("pending", later);
        assert!(a < b);
    }
}
