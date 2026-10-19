//! Organizer actions in the desktop (on-prem) — pure store operations
//! with German error messages for the UI. Same rules as the web's
//! organizer routes (PR 1 / PR 3): review only pending errors,
//! "Übernehmen" = remaining pending → approved, lineup = roster of the
//! run's team without duplicates, hand times = mean of two stopwatches
//! (`kido_core::timing::hand_time_mean`).

use std::collections::HashSet;

use uuid::Uuid;

use super::judge::{lineup_edit, validate_lineup};
use super::store::{HandTimesEdit, LocalStore};
use crate::cloud::types::{DisciplinePayload, LineupConfirmedBy, Penalty, PenaltyStatus};

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

/// "Übernehmen": every pending error of the run → approved. Returns how
/// many were confirmed.
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
}
