//! Organizer actions in the desktop (on-prem) — pure store operations
//! with German error messages for the UI. Same rules as the web's
//! organizer routes (PR 1 / PR 3): review only pending errors,
//! "Übernehmen" = remaining pending → approved and records
//! `Run.penalties_accepted_at`, "Rückgängig" = decided → pending until
//! that timestamp freezes it (`kido_core::penalty::can_undo_review`),
//! lineup = roster of the
//! run's team without duplicates, hand times = mean of two stopwatches
//! (`kido_core::timing::hand_time_mean`).

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::judge::{lineup_edit, validate_lineup};
use super::store::{HandTimesEdit, LocalStore};
use crate::cloud::types::{DisciplinePayload, LineupConfirmedBy, Penalty, PenaltyStatus, Run};

/// Validate the organizer's current runs (one per lane, max
/// `lane_count`, known runs, no duplicates, no two runs on the same
/// explicit lane).
pub fn validate_current_runs(view: &DisciplinePayload, ids: &[Uuid]) -> Result<(), String> {
    let lane_count = usize::from(view.discipline.lane_count.max(1));
    if ids.len() > lane_count {
        return Err(format!(
            "Höchstens {lane_count} aktuelle Läufe (eine pro Bahn)"
        ));
    }
    let mut seen = HashSet::new();
    let mut lanes = HashSet::new();
    for run_id in ids {
        if !seen.insert(*run_id) {
            return Err("Ein Lauf ist mehrfach ausgewählt".into());
        }
        let run =
            view.runs.iter().find(|r| r.id == *run_id).ok_or_else(|| {
                format!("Lauf {run_id} ist im geladenen Wettkampf nicht vorhanden")
            })?;
        if let Some(lane) = run.lane {
            if usize::from(lane) > lane_count || lane == 0 {
                return Err(format!(
                    "Lauf {} ist Bahn {lane} zugeordnet — die Disziplin hat nur {lane_count} Bahn(en)",
                    run.run_number
                ));
            }
            if !lanes.insert(lane) {
                return Err(format!("Bahn {lane} ist doppelt belegt"));
            }
        }
    }
    Ok(())
}

pub fn set_current_runs(store: &mut LocalStore, ids: Vec<Uuid>) -> Result<(), String> {
    validate_current_runs(&store.view(), &ids)?;
    store.set_current_runs(ids);
    Ok(())
}

/// Bestätigen (`approve`) / Zurücknehmen of one pending error.
pub fn review_penalty(
    store: &mut LocalStore,
    penalty_id: Uuid,
    approve: bool,
    by: &str,
    now: &str,
) -> Result<Penalty, String> {
    let mut p = store
        .penalty(penalty_id)
        .ok_or_else(|| "Fehler nicht gefunden".to_string())?;
    if p.status != PenaltyStatus::Pending {
        return Err("Nur offene Fehler können bestätigt oder zurückgenommen werden".into());
    }
    p.status = if approve {
        PenaltyStatus::Approved
    } else {
        PenaltyStatus::Rejected
    };
    p.reviewed_at = Some(now.to_string());
    p.reviewed_by = Some(by.to_string());
    store.upsert_penalty(p.clone());
    Ok(p)
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// "Rückgängig" allowed? [`kido_core::penalty::can_undo_review`] on the
/// desktop's string timestamps. An unparseable `reviewed_at` counts as
/// missing; an unparseable `penalties_accepted_at` freezes (cannot prove
/// the decision is newer).
pub fn can_undo_review(p: &Penalty, run: &Run) -> bool {
    let accepted = match run.penalties_accepted_at.as_deref() {
        None => None,
        Some(s) => match parse_ts(s) {
            Some(t) => Some(t),
            None => return false,
        },
    };
    let reviewed = p.reviewed_at.as_deref().and_then(parse_ts);
    kido_core::penalty::can_undo_review(p.status, reviewed, accepted)
}

/// "Rückgängig": an error decided **on this desktop** → pending again
/// (review fields cleared), until the run's "Übernehmen" froze the
/// decision.
///
/// Decisions that come from the web snapshot (base already approved /
/// rejected) are refused: the upload never turns a decision stored on the
/// web back to pending ("most advanced status wins"), so offering undo
/// there would be misleading.
pub fn undo_review(store: &mut LocalStore, penalty_id: Uuid) -> Result<Penalty, String> {
    let mut p = store
        .penalty(penalty_id)
        .ok_or_else(|| "Fehler nicht gefunden".to_string())?;
    if p.status == PenaltyStatus::Pending {
        return Err("Fehler ist noch offen – nichts rückgängig zu machen".into());
    }
    if store.is_web_decided(penalty_id) {
        return Err(
            "Entscheidung stammt aus dem Web – Rückgängig ist nur für Entscheidungen auf diesem Desktop möglich"
                .into(),
        );
    }
    let view = store.view();
    let run = view.runs.iter().find(|r| r.id == p.run_id).ok_or_else(|| {
        format!(
            "Lauf {} ist im geladenen Wettkampf nicht vorhanden",
            p.run_id
        )
    })?;
    if !can_undo_review(&p, run) {
        return Err(
            "Lauf wurde bereits übernommen – die Entscheidung kann nicht mehr rückgängig gemacht werden"
                .into(),
        );
    }
    p.status = PenaltyStatus::Pending;
    p.reviewed_at = None;
    p.reviewed_by = None;
    store.upsert_penalty(p.clone());
    Ok(p)
}

/// "Übernehmen": every pending error of the run → approved (with
/// `reviewed_at = now`) and `Run.penalties_accepted_at = now`, so every
/// decision made so far is frozen. Returns how many were confirmed.
pub fn take_over(
    store: &mut LocalStore,
    run_id: Uuid,
    by: &str,
    now: &str,
) -> Result<usize, String> {
    let view = store.view();
    if !view.runs.iter().any(|r| r.id == run_id) {
        return Err(format!(
            "Lauf {run_id} ist im geladenen Wettkampf nicht vorhanden"
        ));
    }
    store.set_penalties_accepted_at(run_id, now)?;
    let pending: Vec<Penalty> = view
        .penalties
        .into_iter()
        .filter(|p| p.run_id == run_id && p.status == PenaltyStatus::Pending)
        .collect();
    let n = pending.len();
    for mut p in pending {
        p.status = PenaltyStatus::Approved;
        p.reviewed_at = Some(now.to_string());
        p.reviewed_by = Some(by.to_string());
        store.upsert_penalty(p);
    }
    Ok(n)
}

/// Lineup correction by the organizer (any run).
pub fn set_lineup(
    store: &mut LocalStore,
    run_id: Uuid,
    runner_ids: Vec<Uuid>,
    now: &str,
) -> Result<(), String> {
    let view = store.view();
    let run = view
        .runs
        .iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| format!("Lauf {run_id} ist im geladenen Wettkampf nicht vorhanden"))?;
    validate_lineup(&runner_ids, &view.runners, run.team_id)?;
    store.set_lineup(
        run_id,
        lineup_edit(runner_ids, LineupConfirmedBy::Organizer, now),
    )
}

/// Hand times (both > 0) → Handzeit = mean, rounded to 1/100 s.
pub fn set_hand_times(
    store: &mut LocalStore,
    run_id: Uuid,
    t1: u32,
    t2: u32,
) -> Result<u32, String> {
    if t1 == 0 || t2 == 0 {
        return Err("Handzeiten müssen > 0 sein".into());
    }
    store.set_hand_times(
        run_id,
        HandTimesEdit::Set {
            hand_time_1_ms: t1,
            hand_time_2_ms: t2,
        },
    )?;
    Ok(kido_core::timing::hand_time_mean(t1, t2))
}

/// "Handzeit entfernen": back to the light barrier; the time shown so far
/// stays until the timekeeper records a new one (web semantics).
pub fn clear_hand_times(store: &mut LocalStore, run_id: Uuid) -> Result<(), String> {
    let keep_time_ms = store
        .view()
        .runs
        .iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| format!("Lauf {run_id} ist im geladenen Wettkampf nicht vorhanden"))?
        .original_time_ms;
    store.set_hand_times(run_id, HandTimesEdit::Clear { keep_time_ms })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{
        id, sample_payload, PENALTY_1, RUNNER_A1, RUNNER_B1, RUN_A1, RUN_B1, RUN_C1,
    };
    use crate::cloud::types::StationKind;
    use crate::cloud::types::{PenaltyKind, TimeSource};
    use crate::local::judge::{submit_penalty, SubmitStationPenaltyInput};

    const NOW: &str = "2027-06-12T10:05:00.000Z";

    fn with_pending(code: &str, run: &str) -> (LocalStore, Penalty) {
        let mut st = LocalStore::new(sample_payload());
        let (_, p) = submit_penalty(
            &mut st,
            SubmitStationPenaltyInput {
                station: StationKind::Knotengestell,
                run_id: Some(id(run)),
                code: Some(code.into()),
                client_id: None,
            },
            Uuid::new_v4(),
            "2027-06-12T10:01:00.000Z",
        )
        .unwrap();
        (st, p)
    }

    #[test]
    fn current_runs_validation() {
        let p = sample_payload();
        assert!(validate_current_runs(&p, &[]).is_ok());
        assert!(validate_current_runs(&p, &[id(RUN_A1), id(RUN_B1)]).is_ok());
        assert!(
            validate_current_runs(&p, &[id(RUN_A1), id(RUN_C1)]).is_ok(),
            "C1 has no lane"
        );
        let e = validate_current_runs(&p, &[id(RUN_A1), id(RUN_B1), id(RUN_C1)]).unwrap_err();
        assert!(e.contains("Höchstens 2"));
        let e = validate_current_runs(&p, &[id(RUN_A1), id(RUN_A1)]).unwrap_err();
        assert!(e.contains("mehrfach"));
        let e = validate_current_runs(&p, &[Uuid::new_v4()]).unwrap_err();
        assert!(e.contains("nicht vorhanden"));
        let mut same_lane = sample_payload();
        same_lane.runs[1].lane = Some(1);
        let e = validate_current_runs(&same_lane, &[id(RUN_A1), id(RUN_B1)]).unwrap_err();
        assert!(e.contains("doppelt belegt"));
        let mut one_lane = sample_payload();
        one_lane.discipline.lane_count = 1;
        let e = validate_current_runs(&one_lane, &[id(RUN_B1)]).unwrap_err();
        assert!(e.contains("nur 1 Bahn"));
    }

    #[test]
    fn set_current_runs_applies_to_view() {
        let mut st = LocalStore::new(sample_payload());
        set_current_runs(&mut st, vec![id(RUN_C1)]).unwrap();
        assert_eq!(st.view().event.current_run_ids, vec![id(RUN_C1)]);
        assert!(set_current_runs(&mut st, vec![id(RUN_A1), id(RUN_A1)]).is_err());
        assert_eq!(
            st.view().event.current_run_ids,
            vec![id(RUN_C1)],
            "unchanged on error"
        );
    }

    #[test]
    fn approve_and_withdraw_pending() {
        let (mut st, p) = with_pending("knotengestell.ein_knoten_falsch", RUN_A1);
        let approved = review_penalty(&mut st, p.id, true, "desktop", NOW).unwrap();
        assert_eq!(approved.status, PenaltyStatus::Approved);
        assert_eq!(approved.reviewed_by.as_deref(), Some("desktop"));
        assert_eq!(approved.reviewed_at.as_deref(), Some(NOW));
        // Reviewed → no second review.
        assert!(review_penalty(&mut st, p.id, false, "desktop", NOW).is_err());

        let (mut st, p) = with_pending("knotengestell.ein_knoten_falsch", RUN_A1);
        let rejected = review_penalty(&mut st, p.id, false, "desktop", NOW).unwrap();
        assert_eq!(rejected.status, PenaltyStatus::Rejected);
        // Rejected stays visible (crossed out in the UI).
        assert!(st.view().penalties.iter().any(|x| x.id == p.id));
    }

    #[test]
    fn review_unknown_penalty_fails() {
        let mut st = LocalStore::new(sample_payload());
        assert!(review_penalty(&mut st, Uuid::new_v4(), true, "d", NOW).is_err());
        assert!(
            review_penalty(&mut st, id(PENALTY_1), true, "d", NOW).is_err(),
            "already approved"
        );
    }

    #[test]
    fn take_over_approves_only_pending_of_that_run() {
        let (mut st, a) = with_pending("knotengestell.ein_knoten_falsch", RUN_A1);
        let (_, b) = submit_penalty(
            &mut st,
            SubmitStationPenaltyInput {
                station: StationKind::Knotengestell,
                run_id: Some(id(RUN_A1)),
                code: Some("knotengestell.mehrere_knoten_falsch".into()),
                client_id: None,
            },
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        let (_, other) = submit_penalty(
            &mut st,
            SubmitStationPenaltyInput {
                station: StationKind::Knotengestell,
                run_id: Some(id(RUN_B1)),
                code: Some("knotengestell.ein_knoten_falsch".into()),
                client_id: None,
            },
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        review_penalty(&mut st, b.id, false, "d", NOW).unwrap();
        let n = take_over(&mut st, id(RUN_A1), "d", NOW).unwrap();
        assert_eq!(n, 1);
        assert_eq!(st.penalty(a.id).unwrap().status, PenaltyStatus::Approved);
        assert_eq!(st.penalty(b.id).unwrap().status, PenaltyStatus::Rejected);
        assert_eq!(st.penalty(other.id).unwrap().status, PenaltyStatus::Pending);
        assert_eq!(take_over(&mut st, id(RUN_A1), "d", NOW).unwrap(), 0);
        assert!(take_over(&mut st, Uuid::new_v4(), "d", NOW).is_err());
        assert_eq!(st.penalty(b.id).unwrap().kind, PenaltyKind::Fv);
    }

    #[test]
    fn organizer_lineup_any_run() {
        let mut st = LocalStore::new(sample_payload());
        set_lineup(&mut st, id(RUN_C1), vec![], NOW).unwrap();
        set_lineup(&mut st, id(RUN_A1), vec![id(RUNNER_A1)], NOW).unwrap();
        let run = st.view().runs[0].clone();
        assert_eq!(run.lineup, vec![id(RUNNER_A1)]);
        assert_eq!(run.lineup_confirmed_by, Some(LineupConfirmedBy::Organizer));
        assert!(set_lineup(&mut st, id(RUN_A1), vec![id(RUNNER_B1)], NOW).is_err());
        assert!(set_lineup(&mut st, Uuid::new_v4(), vec![], NOW).is_err());
    }

    #[test]
    fn hand_times_set_and_clear() {
        let mut st = LocalStore::new(sample_payload());
        assert_eq!(
            set_hand_times(&mut st, id(RUN_A1), 47_230, 47_245).unwrap(),
            47_240
        );
        let run = st.view().runs[0].clone();
        assert_eq!(run.time_source, TimeSource::Handstopp);
        assert_eq!(run.original_time_ms, Some(47_240));
        assert!(set_hand_times(&mut st, id(RUN_A1), 0, 47_245).is_err());
        assert!(set_hand_times(&mut st, Uuid::new_v4(), 1, 1).is_err());
        clear_hand_times(&mut st, id(RUN_A1)).unwrap();
        let run = st.view().runs[0].clone();
        assert_eq!(run.time_source, TimeSource::Lichtschranke);
        assert_eq!(run.hand_time_1_ms, None);
        assert_eq!(run.original_time_ms, Some(47_240), "time kept like the web");
        assert!(clear_hand_times(&mut st, Uuid::new_v4()).is_err());
    }

    // ── Rückgängig / Übernehmen timestamp ───────────────────────────

    const LATER: &str = "2027-06-12T10:10:00.000Z";
    const LATEST: &str = "2027-06-12T10:20:00.000Z";

    fn run_a1(st: &LocalStore) -> Run {
        st.view()
            .runs
            .into_iter()
            .find(|r| r.id == id(RUN_A1))
            .unwrap()
    }

    #[test]
    fn undo_approved_and_rejected_back_to_pending() {
        for approve in [true, false] {
            let (mut st, p) = with_pending("knotengestell.ein_knoten_falsch", RUN_A1);
            review_penalty(&mut st, p.id, approve, "desktop", NOW).unwrap();
            let undone = undo_review(&mut st, p.id).unwrap();
            assert_eq!(undone.status, PenaltyStatus::Pending);
            assert_eq!(undone.reviewed_at, None);
            assert_eq!(undone.reviewed_by, None);
            assert_eq!(st.penalty(p.id).unwrap().status, PenaltyStatus::Pending);
            // ... and it can be decided again.
            review_penalty(&mut st, p.id, !approve, "desktop", LATER).unwrap();
        }
    }

    #[test]
    fn undo_recomputes_legacy_correction() {
        let (mut st, p) = with_pending("knotengestell.ein_knoten_falsch", RUN_A1);
        // PENALTY_1 (approved, base) + the new one.
        review_penalty(&mut st, p.id, true, "desktop", NOW).unwrap();
        assert_eq!(run_a1(&st).correction_ms, 30_000);
        undo_review(&mut st, p.id).unwrap();
        assert_eq!(run_a1(&st).correction_ms, 15_000);
    }

    #[test]
    fn undo_pending_or_unknown_fails() {
        let (mut st, p) = with_pending("knotengestell.ein_knoten_falsch", RUN_A1);
        let e = undo_review(&mut st, p.id).unwrap_err();
        assert!(e.contains("noch offen"));
        assert!(undo_review(&mut st, Uuid::new_v4()).is_err());
    }

    #[test]
    fn take_over_sets_penalties_accepted_at_every_time() {
        let mut st = LocalStore::new(sample_payload());
        assert_eq!(run_a1(&st).penalties_accepted_at, None);
        take_over(&mut st, id(RUN_A1), "d", NOW).unwrap();
        assert_eq!(run_a1(&st).penalties_accepted_at.as_deref(), Some(NOW));
        take_over(&mut st, id(RUN_A1), "d", LATER).unwrap();
        assert_eq!(run_a1(&st).penalties_accepted_at.as_deref(), Some(LATER));
        assert!(st.has_edits(), "timestamp is a local change to upload");
        // Unknown run: nothing recorded.
        assert!(take_over(&mut st, Uuid::new_v4(), "d", NOW).is_err());
    }

    #[test]
    fn take_over_freezes_earlier_and_bulk_approved_decisions() {
        let (mut st, decided) = with_pending("knotengestell.ein_knoten_falsch", RUN_A1);
        let (_, open) = submit_penalty(
            &mut st,
            SubmitStationPenaltyInput {
                station: StationKind::Knotengestell,
                run_id: Some(id(RUN_A1)),
                code: Some("knotengestell.ein_knoten_falsch".into()),
                client_id: None,
            },
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        review_penalty(&mut st, decided.id, false, "d", NOW).unwrap();
        take_over(&mut st, id(RUN_A1), "d", LATER).unwrap();

        let bulk = st.penalty(open.id).unwrap();
        assert_eq!(bulk.status, PenaltyStatus::Approved);
        assert_eq!(bulk.reviewed_at.as_deref(), Some(LATER), "same instant");
        assert!(!can_undo_review(&bulk, &run_a1(&st)));
        let e = undo_review(&mut st, open.id).unwrap_err();
        assert!(e.contains("übernommen"));
        let e = undo_review(&mut st, decided.id).unwrap_err();
        assert!(e.contains("übernommen"));
        // The web's decision (PENALTY_1, base) is never undoable here.
        assert!(undo_review(&mut st, id(PENALTY_1)).is_err());
        assert_eq!(
            st.penalty(decided.id).unwrap().status,
            PenaltyStatus::Rejected
        );
    }

    #[test]
    fn decision_after_take_over_stays_undoable_until_next_take_over() {
        let mut st = LocalStore::new(sample_payload());
        take_over(&mut st, id(RUN_A1), "d", NOW).unwrap();
        // Late report, decided after Übernehmen.
        let (_, late) = submit_penalty(
            &mut st,
            SubmitStationPenaltyInput {
                station: StationKind::Knotengestell,
                run_id: Some(id(RUN_A1)),
                code: Some("knotengestell.ein_knoten_falsch".into()),
                client_id: None,
            },
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        review_penalty(&mut st, late.id, true, "d", LATER).unwrap();
        assert!(can_undo_review(&st.penalty(late.id).unwrap(), &run_a1(&st)));
        undo_review(&mut st, late.id).unwrap();
        review_penalty(&mut st, late.id, true, "d", LATER).unwrap();
        take_over(&mut st, id(RUN_A1), "d", LATEST).unwrap();
        assert!(undo_review(&mut st, late.id).is_err());
    }

    #[test]
    fn web_decision_not_undoable_locally() {
        // PENALTY_1 was approved on the web (base snapshot): no undo on the
        // desktop, even before any Übernehmen — the upload would keep the
        // web's decision anyway.
        let mut st = LocalStore::new(sample_payload());
        assert!(st.is_web_decided(id(PENALTY_1)));
        assert_eq!(st.web_decided_penalty_ids(), vec![id(PENALTY_1)]);
        let e = undo_review(&mut st, id(PENALTY_1)).unwrap_err();
        assert!(e.contains("aus dem Web"), "{e}");
        assert_eq!(
            st.penalty(id(PENALTY_1)).unwrap().status,
            PenaltyStatus::Approved
        );
        assert!(!st.has_edits(), "nothing recorded");
    }

    #[test]
    fn local_decision_on_web_pending_penalty_is_undoable() {
        // The web has the error as pending; the desktop decides → local
        // decision → undoable.
        let mut base = sample_payload();
        base.penalties[0].status = PenaltyStatus::Pending;
        base.penalties[0].reviewed_at = None;
        base.penalties[0].reviewed_by = None;
        base.sync_approved_penalties();
        let mut st = LocalStore::new(base);
        assert!(!st.is_web_decided(id(PENALTY_1)));
        review_penalty(&mut st, id(PENALTY_1), false, "d", NOW).unwrap();
        assert!(!st.is_web_decided(id(PENALTY_1)), "overlay is not the base");
        undo_review(&mut st, id(PENALTY_1)).unwrap();
        assert_eq!(
            st.penalty(id(PENALTY_1)).unwrap().status,
            PenaltyStatus::Pending
        );
        assert!(!st.has_edits(), "back to the base state → overlay dropped");
    }

    #[test]
    fn uploaded_local_decision_becomes_web_decided() {
        let (mut st, p) = with_pending("knotengestell.ein_knoten_falsch", RUN_A1);
        review_penalty(&mut st, p.id, true, "d", NOW).unwrap();
        let up = st.upload_payload();
        st.mark_synced(&up, LATER.into());
        assert!(st.is_web_decided(p.id));
        assert!(undo_review(&mut st, p.id)
            .unwrap_err()
            .contains("aus dem Web"));
    }

    #[test]
    fn can_undo_review_parses_mixed_precision_and_bad_timestamps() {
        let st = LocalStore::new(sample_payload());
        let mut run = run_a1(&st);
        let mut p = st.penalty(id(PENALTY_1)).unwrap();
        // Base reviewed_at 09:45:00Z (web, second precision).
        run.penalties_accepted_at = Some("2027-06-12T09:45:00.000Z".into());
        assert!(!can_undo_review(&p, &run), "equal instant across formats");
        run.penalties_accepted_at = Some("2027-06-12T09:44:59.999999999Z".into());
        assert!(can_undo_review(&p, &run));
        run.penalties_accepted_at = Some("kaputt".into());
        assert!(
            !can_undo_review(&p, &run),
            "unparseable accepted_at freezes"
        );
        run.penalties_accepted_at = None;
        p.reviewed_at = Some("kaputt".into());
        assert!(can_undo_review(&p, &run), "no take-over → undoable");
        run.penalties_accepted_at = Some("2027-06-12T09:00:00Z".into());
        assert!(
            !can_undo_review(&p, &run),
            "unparseable reviewed_at = missing"
        );
        p.status = PenaltyStatus::Pending;
        run.penalties_accepted_at = None;
        assert!(!can_undo_review(&p, &run));
    }

    #[test]
    fn take_over_timestamp_is_in_upload_and_kido_export() {
        let mut st = LocalStore::new(sample_payload());
        take_over(&mut st, id(RUN_B1), "d", NOW).unwrap();
        let up = st.upload_payload();
        assert_eq!(
            up.runs.len(),
            1,
            "the run is uploaded although no penalty changed"
        );
        assert_eq!(up.runs[0].id, id(RUN_B1));
        assert_eq!(up.runs[0].penalties_accepted_at.as_deref(), Some(NOW));
        let json = serde_json::to_value(&up).unwrap();
        assert_eq!(json["runs"][0]["penalties_accepted_at"], NOW);
        // The `.kido` export is the view.
        let view = serde_json::to_value(st.view()).unwrap();
        let run = view["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == RUN_B1)
            .unwrap()
            .clone();
        assert_eq!(run["penalties_accepted_at"], NOW);

        // After the web accepted the upload, the overlay is folded into the base.
        st.mark_synced(&up, LATER.into());
        assert!(st.run_extras.is_empty());
        assert_eq!(st.base.runs[1].penalties_accepted_at.as_deref(), Some(NOW));
    }
}
