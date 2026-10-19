//! Which run is timed on which desktop lane.
//!
//! Hardware: the timer has 4 channels; the Gruppenstafette uses one channel
//! per lane, so **lane N = channel N**. The discipline's `lane_count`
//! decides how many lanes are in use (Gruppenstafette 1|2, Löschangriff 4).
//!
//! Resolution order for a lane (mirrored in `src/lib/lanes.ts` for the UI):
//! 1. the web's `current_run_ids` — the current run whose `lane` matches;
//!    current runs without a lane fill the remaining lanes in list order.
//!    A current run that is already `completed` is skipped, so a new start
//!    never overwrites a recorded result (on-prem the current runs are
//!    frozen at the last web snapshot; the operator picks the next run),
//! 2. an operator override (lane → run id) from the lane assignment panel,
//! 3. the active run on that lane.

use std::collections::HashMap;

use uuid::Uuid;

use crate::cloud::types::{DisciplinePayload, Run, RunStatus};

/// Current runs (from `event.current_run_ids`) mapped to lanes
/// `1..=lane_count`. Runs with an explicit lane keep it (if in range);
/// runs without a lane take the lowest free lanes in list order. Unknown
/// ids are ignored.
pub fn current_runs_by_lane(snapshot: &DisciplinePayload) -> HashMap<u8, &Run> {
    let lane_count = snapshot.discipline.lane_count.max(1);
    let current: Vec<&Run> = snapshot
        .event
        .current_run_ids
        .iter()
        .filter_map(|id| snapshot.runs.iter().find(|r| r.id == *id))
        .collect();
    let mut by_lane: HashMap<u8, &Run> = HashMap::new();
    for run in &current {
        if let Some(lane) = run.lane {
            if (1..=lane_count).contains(&lane) {
                by_lane.entry(lane).or_insert(run);
            }
        }
    }
    for run in current.iter().filter(|r| r.lane.is_none()) {
        if let Some(free) = (1..=lane_count).find(|l| !by_lane.contains_key(l)) {
            by_lane.insert(free, run);
        }
    }
    by_lane
}

/// The run that receives timing events of desktop lane `lane`.
pub fn resolve_run_for_lane<'a>(
    snapshot: &'a DisciplinePayload,
    lane: u8,
    overrides: &HashMap<u8, Uuid>,
) -> Option<&'a Run> {
    if let Some(run) = current_runs_by_lane(snapshot).get(&lane) {
        if run.status != RunStatus::Completed {
            return Some(run);
        }
    }
    if let Some(run_id) = overrides.get(&lane) {
        if let Some(run) = snapshot.runs.iter().find(|r| r.id == *run_id) {
            return Some(run);
        }
    }
    snapshot
        .runs
        .iter()
        .find(|r| r.status == RunStatus::Active && r.lane == Some(lane))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{id, sample_payload, RUN_A1, RUN_B1, RUN_C1};

    fn no_overrides() -> HashMap<u8, Uuid> {
        HashMap::new()
    }

    #[test]
    fn current_runs_resolve_by_lane() {
        let s = sample_payload();
        assert_eq!(
            resolve_run_for_lane(&s, 1, &no_overrides()).map(|r| r.id),
            Some(id(RUN_A1))
        );
        assert_eq!(
            resolve_run_for_lane(&s, 2, &no_overrides()).map(|r| r.id),
            Some(id(RUN_B1))
        );
    }

    #[test]
    fn order_of_current_run_ids_does_not_matter() {
        let mut s = sample_payload();
        s.event.current_run_ids.reverse();
        assert_eq!(
            resolve_run_for_lane(&s, 1, &no_overrides()).map(|r| r.id),
            Some(id(RUN_A1))
        );
        assert_eq!(
            resolve_run_for_lane(&s, 2, &no_overrides()).map(|r| r.id),
            Some(id(RUN_B1))
        );
    }

    #[test]
    fn current_run_wins_over_override() {
        let s = sample_payload();
        let overrides = HashMap::from([(1u8, id(RUN_C1))]);
        assert_eq!(
            resolve_run_for_lane(&s, 1, &overrides).map(|r| r.id),
            Some(id(RUN_A1))
        );
    }

    #[test]
    fn completed_current_run_is_not_overwritten() {
        let mut s = sample_payload();
        s.runs[0].status = RunStatus::Completed; // A1, lane 1, current
        s.runs[0].original_time_ms = Some(47_240);
        // No override: nothing to record into (instead of overwriting A1).
        assert!(resolve_run_for_lane(&s, 1, &no_overrides()).is_none());
        // The operator picks the next run.
        let overrides = HashMap::from([(1u8, id(RUN_C1))]);
        assert_eq!(
            resolve_run_for_lane(&s, 1, &overrides).map(|r| r.id),
            Some(id(RUN_C1))
        );
        // Cancelled current runs stay the target (restart after false start).
        s.runs[0].status = RunStatus::Cancelled;
        assert_eq!(
            resolve_run_for_lane(&s, 1, &no_overrides()).map(|r| r.id),
            Some(id(RUN_A1))
        );
    }

    #[test]
    fn override_used_when_no_current_run_on_lane() {
        let mut s = sample_payload();
        s.event.current_run_ids = vec![id(RUN_A1)];
        let overrides = HashMap::from([(2u8, id(RUN_C1))]);
        assert_eq!(
            resolve_run_for_lane(&s, 2, &overrides).map(|r| r.id),
            Some(id(RUN_C1))
        );
    }

    #[test]
    fn override_to_unknown_run_is_ignored() {
        let mut s = sample_payload();
        s.event.current_run_ids.clear();
        let overrides = HashMap::from([(1u8, Uuid::new_v4())]);
        assert!(resolve_run_for_lane(&s, 1, &overrides).is_none());
    }

    #[test]
    fn falls_back_to_active_run_on_lane() {
        let mut s = sample_payload();
        s.event.current_run_ids.clear();
        s.runs[1].status = RunStatus::Active; // B1, lane 2
        assert!(resolve_run_for_lane(&s, 1, &no_overrides()).is_none());
        assert_eq!(
            resolve_run_for_lane(&s, 2, &no_overrides()).map(|r| r.id),
            Some(id(RUN_B1))
        );
    }

    #[test]
    fn current_run_without_lane_fills_free_lane() {
        let mut s = sample_payload();
        s.event.current_run_ids = vec![id(RUN_B1), id(RUN_C1)]; // C1 has no lane
        let map = current_runs_by_lane(&s);
        assert_eq!(map.get(&2).map(|r| r.id), Some(id(RUN_B1)));
        assert_eq!(map.get(&1).map(|r| r.id), Some(id(RUN_C1)));
    }

    #[test]
    fn single_lane_discipline() {
        let mut s = sample_payload();
        s.discipline.lane_count = 1;
        s.event.current_run_ids = vec![id(RUN_C1)];
        assert_eq!(
            resolve_run_for_lane(&s, 1, &no_overrides()).map(|r| r.id),
            Some(id(RUN_C1))
        );
        // Lane 2 is not in use in a 1-lane discipline.
        s.event.current_run_ids = vec![id(RUN_B1)];
        assert!(current_runs_by_lane(&s).is_empty());
    }

    #[test]
    fn unknown_current_ids_ignored() {
        let mut s = sample_payload();
        s.event.current_run_ids = vec![Uuid::new_v4()];
        assert!(current_runs_by_lane(&s).is_empty());
    }
}
