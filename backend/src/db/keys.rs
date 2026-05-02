// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT
//
// Single-table DynamoDB key construction. See CLAUDE.md §5 for the layout.
//
// Numeric components in sort keys are zero-padded so DDB's lexicographic sort
// matches the numeric order ("000010" sorts after "000002", not before).

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

// ===== Partition keys =====

pub fn user_pk(sub: &str) -> String {
    format!("USER#{sub}")
}

pub fn user_email_gsi1pk(email: &str) -> String {
    format!("USER_EMAIL#{}", email.to_lowercase())
}

pub fn competition_pk(id: Uuid) -> String {
    format!("COMP#{id}")
}

pub fn penalty_status_gsi1pk(competition_id: Uuid) -> String {
    format!("COMP#{competition_id}#PENALTY")
}

pub fn run_status_gsi1pk(competition_id: Uuid) -> String {
    format!("COMP#{competition_id}#RUN")
}

pub fn user_owned_competitions_gsi1pk(owner_sub: &str) -> String {
    format!("USER#{owner_sub}")
}

// ===== Sort keys (constants) =====

pub const SK_PROFILE: &str = "PROFILE";
pub const SK_META: &str = "META";
pub const SK_ACTIVE_COMP: &str = "ACTIVE_COMP";

// ===== Sort keys (dynamic) =====

pub fn user_sk_apikey(key_id: &str) -> String {
    format!("APIKEY#{key_id}")
}

pub fn competition_sk_team(start_number: u32, team_id: Uuid) -> String {
    format!("TEAM#{start_number:06}#{team_id}")
}

pub fn competition_sk_runner(team_id: Uuid, position: u32) -> String {
    format!("RUNNER#{team_id}#{position:04}")
}

pub fn competition_sk_run(run_number: u32, run_id: Uuid) -> String {
    format!("RUN#{run_number:04}#{run_id}")
}

pub fn competition_sk_penalty(penalty_id: Uuid) -> String {
    format!("PENALTY#{penalty_id}")
}

pub fn user_competitions_gsi1sk(date: NaiveDate, competition_id: Uuid) -> String {
    format!("COMP#{date}#{competition_id}")
}

pub fn run_status_gsi1sk(status: &str, run_number: u32) -> String {
    format!("STATUS#{status}#{run_number:04}")
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
    fn team_sk_pads_start_number_for_lexsort() {
        let id = Uuid::nil();
        let small = competition_sk_team(2, id);
        let large = competition_sk_team(10, id);
        assert!(small < large, "{small} should sort before {large}");
        assert_eq!(small, "TEAM#000002#00000000-0000-0000-0000-000000000000");
    }

    #[test]
    fn run_sk_pads_run_number() {
        let id = Uuid::nil();
        let first = competition_sk_run(1, id);
        let tenth = competition_sk_run(10, id);
        assert!(first < tenth, "{first} should sort before {tenth}");
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
