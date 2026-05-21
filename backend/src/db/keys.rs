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
//   RUNNER#<team_id>#<position:04>         — runner
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

pub fn event_sk_runner(team_id: Uuid, position: u32) -> String {
    format!("RUNNER#{team_id}#{position:04}")
}

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
