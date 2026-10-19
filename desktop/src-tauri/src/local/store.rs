//! In-memory local result store for one discipline.
//!
//! Model: `base` is the last snapshot received from the web (live poll,
//! picker selection) or read from a `.kido` file. On top of it the store
//! keeps **overlays** — changes recorded on this desktop that the web has
//! not acknowledged yet:
//!
//! - `run_edits`: run timing (status, time, timestamps) — also used for
//!   live result POSTs,
//! - `run_extras`: lineup and hand times (on-prem organizer / Startlinie
//!   judge),
//! - `penalty_edits`: penalty rows created by judges' phones or reviewed
//!   by the organizer (full rows, keyed by id = client id),
//! - `current_run_ids`: the organizer's current-run choice (on-prem only,
//!   never uploaded — the upload clears the web's current runs).
//!
//! The [`LocalStore::view`] (base with all overlays applied) is what the
//! UI shows, what the judge API serves, what results are computed from
//! and what a `.kido` export contains.
//!
//! - On-prem: every change stays an overlay (nothing is posted) until
//!   "Ins Web hochladen" ([`LocalStore::upload_payload`] /
//!   [`LocalStore::mark_synced`]).
//! - Live: a timing edit lives until its POST succeeds
//!   ([`LocalStore::acknowledge`]); failed posts keep the edit, so a later
//!   poll never hides a local time.
//! - Switching back to live: [`LocalStore::divergence`] lists timing edits
//!   that differ from the web, [`LocalStore::unsynced_summary`] the other
//!   overlays; the operator uploads or discards them.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cloud::types::{
    DisciplinePayload, LineupConfirmedBy, OnPremUploadRequest, OnPremUploadRun, Penalty,
    PenaltyStatus, Run, RunDivergence, RunResultRequest, RunStatus, TimeSource, UnsyncedSummary,
};

/// A locally recorded timing state of one run (same fields the live
/// result endpoint accepts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunEdit {
    pub run_number: u32,
    pub status: RunStatus,
    pub original_time_ms: Option<u32>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    /// Local clock when the change was recorded (RFC 3339).
    pub recorded_at: String,
}

impl RunEdit {
    pub fn from_request(body: &RunResultRequest, recorded_at: String) -> Self {
        Self {
            run_number: body.run_number,
            status: body.status,
            original_time_ms: body.original_time_ms,
            started_at: body.started_at.clone(),
            ended_at: body.ended_at.clone(),
            recorded_at,
        }
    }

    pub fn to_request(&self) -> RunResultRequest {
        RunResultRequest {
            run_number: self.run_number,
            status: self.status,
            original_time_ms: self.original_time_ms,
            started_at: self.started_at.clone(),
            ended_at: self.ended_at.clone(),
        }
    }

    /// Edit fully reflected in `run` (all timing fields).
    fn applied_in(&self, run: &Run) -> bool {
        run.status == self.status
            && run.original_time_ms == self.original_time_ms
            && run.started_at == self.started_at
            && run.ended_at == self.ended_at
    }

    /// Same result as `run` (status + time). Timestamps are ignored: the
    /// server may normalize their formatting.
    fn same_result_as(&self, run: &Run) -> bool {
        run.status == self.status && run.original_time_ms == self.original_time_ms
    }

    fn apply_to(&self, run: &mut Run) {
        run.status = self.status;
        run.original_time_ms = self.original_time_ms;
        run.started_at = self.started_at.clone();
        run.ended_at = self.ended_at.clone();
    }
}

/// Lineup recorded on the desktop (Startlinie judge or organizer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineupEdit {
    pub runner_ids: Vec<Uuid>,
    pub confirmed_at: Option<String>,
    pub confirmed_by: Option<LineupConfirmedBy>,
}

impl LineupEdit {
    fn applied_in(&self, run: &Run) -> bool {
        run.lineup == self.runner_ids
            && run.lineup_confirmed_at == self.confirmed_at
            && run.lineup_confirmed_by == self.confirmed_by
    }

    fn apply_to(&self, run: &mut Run) {
        run.lineup = self.runner_ids.clone();
        run.lineup_confirmed_at = self.confirmed_at.clone();
        run.lineup_confirmed_by = self.confirmed_by;
    }
}

/// Hand times (light barrier failed) or revert to the light barrier —
/// same semantics as the web's hand-times endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum HandTimesEdit {
    /// `time_source = handstopp`, `original_time_ms = hand_time_mean`.
    Set {
        hand_time_1_ms: u32,
        hand_time_2_ms: u32,
    },
    /// `time_source = lichtschranke`, hand times cleared. The time shown
    /// before (`keep_time_ms`, usually the hand-time mean) is kept until
    /// the timekeeper records a new one — same as the web.
    Clear {
        #[serde(default)]
        keep_time_ms: Option<u32>,
    },
}

impl HandTimesEdit {
    fn applied_in(&self, run: &Run) -> bool {
        match self {
            HandTimesEdit::Set {
                hand_time_1_ms,
                hand_time_2_ms,
            } => {
                run.time_source == TimeSource::Handstopp
                    && run.hand_time_1_ms == Some(*hand_time_1_ms)
                    && run.hand_time_2_ms == Some(*hand_time_2_ms)
                    && run.original_time_ms
                        == Some(kido_core::timing::hand_time_mean(
                            *hand_time_1_ms,
                            *hand_time_2_ms,
                        ))
            }
            HandTimesEdit::Clear { keep_time_ms } => {
                run.time_source == TimeSource::Lichtschranke
                    && run.hand_time_1_ms.is_none()
                    && run.hand_time_2_ms.is_none()
                    && keep_time_ms.is_none_or(|t| run.original_time_ms == Some(t))
            }
        }
    }

    fn apply_to(&self, run: &mut Run) {
        match self {
            HandTimesEdit::Set {
                hand_time_1_ms,
                hand_time_2_ms,
            } => {
                run.time_source = TimeSource::Handstopp;
                run.hand_time_1_ms = Some(*hand_time_1_ms);
                run.hand_time_2_ms = Some(*hand_time_2_ms);
                run.original_time_ms = Some(kido_core::timing::hand_time_mean(
                    *hand_time_1_ms,
                    *hand_time_2_ms,
                ));
                run.correction_ms = 0;
            }
            HandTimesEdit::Clear { keep_time_ms } => {
                run.time_source = TimeSource::Lichtschranke;
                run.hand_time_1_ms = None;
                run.hand_time_2_ms = None;
                if let Some(t) = keep_time_ms {
                    run.original_time_ms = Some(*t);
                }
            }
        }
    }
}

/// On-prem run fields besides timing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunExtras {
    #[serde(default)]
    pub lineup: Option<LineupEdit>,
    #[serde(default)]
    pub hand_times: Option<HandTimesEdit>,
}

impl RunExtras {
    fn is_empty(&self) -> bool {
        self.lineup.is_none() && self.hand_times.is_none()
    }

    /// Drop the parts `run` already reflects.
    fn prune(&mut self, run: &Run) {
        if self.lineup.as_ref().is_some_and(|l| l.applied_in(run)) {
            self.lineup = None;
        }
        if self.hand_times.as_ref().is_some_and(|h| h.applied_in(run)) {
            self.hand_times = None;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalStore {
    pub base: DisciplinePayload,
    #[serde(default)]
    pub run_edits: BTreeMap<Uuid, RunEdit>,
    /// Lineups / hand times recorded on this desktop (on-prem).
    #[serde(default)]
    pub run_extras: BTreeMap<Uuid, RunExtras>,
    /// Penalty rows created or reviewed on this desktop (on-prem).
    #[serde(default)]
    pub penalty_edits: BTreeMap<Uuid, Penalty>,
    /// Organizer's current-run choice (on-prem). `None` = the base's.
    #[serde(default)]
    pub current_run_ids: Option<Vec<Uuid>>,
    /// Access code of the on-prem judge server (128-bit hex). Kept per
    /// discipline so phones keep working after a restart.
    #[serde(default)]
    pub onprem_token: Option<String>,
    /// Last successful "Ins Web hochladen" (RFC 3339).
    #[serde(default)]
    pub last_upload_at: Option<String>,
}

impl LocalStore {
    pub fn new(base: DisciplinePayload) -> Self {
        Self {
            base,
            run_edits: BTreeMap::new(),
            run_extras: BTreeMap::new(),
            penalty_edits: BTreeMap::new(),
            current_run_ids: None,
            onprem_token: None,
            last_upload_at: None,
        }
    }

    pub fn discipline_id(&self) -> Uuid {
        self.base.discipline.id
    }

    /// Any change not acknowledged by the web (the current-run choice is
    /// desk-local and does not count).
    pub fn has_edits(&self) -> bool {
        !self.run_edits.is_empty() || !self.run_extras.is_empty() || !self.penalty_edits.is_empty()
    }

    /// Base snapshot with all local overlays applied.
    pub fn view(&self) -> DisciplinePayload {
        let mut view = self.base.clone();
        for run in &mut view.runs {
            if let Some(edit) = self.run_edits.get(&run.id) {
                edit.apply_to(run);
            }
            if let Some(extras) = self.run_extras.get(&run.id) {
                if let Some(lineup) = &extras.lineup {
                    lineup.apply_to(run);
                }
                if let Some(hand) = &extras.hand_times {
                    hand.apply_to(run);
                }
            }
        }

        if !self.penalty_edits.is_empty() {
            let mut touched_runs: BTreeSet<Uuid> = BTreeSet::new();
            for p in &mut view.penalties {
                if let Some(edit) = self.penalty_edits.get(&p.id) {
                    touched_runs.insert(p.run_id);
                    *p = edit.clone();
                    touched_runs.insert(p.run_id);
                }
            }
            let known: BTreeSet<Uuid> = view.penalties.iter().map(|p| p.id).collect();
            let mut added: Vec<&Penalty> = self
                .penalty_edits
                .values()
                .filter(|p| !known.contains(&p.id))
                .collect();
            added.sort_by(|a, b| (&a.created_at, a.id).cmp(&(&b.created_at, b.id)));
            for p in added {
                touched_runs.insert(p.run_id);
                view.penalties.push(p.clone());
            }
            // Legacy display value (see PR 1 contract): sum of approved
            // delta_ms — recomputed for runs whose penalties changed here.
            for run in view
                .runs
                .iter_mut()
                .filter(|r| touched_runs.contains(&r.id))
            {
                run.correction_ms = view
                    .penalties
                    .iter()
                    .filter(|p| p.run_id == run.id && p.status == PenaltyStatus::Approved)
                    .map(|p| p.delta_ms)
                    .sum();
            }
        }
        view.sync_approved_penalties();

        if let Some(ids) = &self.current_run_ids {
            view.event.current_run_ids = ids.clone();
            view.event.current_run_id = ids.first().copied();
        }
        view
    }

    fn base_run(&self, run_id: Uuid) -> Result<&Run, String> {
        self.base
            .runs
            .iter()
            .find(|r| r.id == run_id)
            .ok_or_else(|| format!("Lauf {run_id} ist im geladenen Wettkampf nicht vorhanden"))
    }

    /// Record a timing change. The run must exist in the snapshot. An edit
    /// that the base already reflects is dropped (nothing to sync).
    pub fn record(&mut self, run_id: Uuid, edit: RunEdit) -> Result<(), String> {
        let run = self.base_run(run_id)?;
        if edit.applied_in(run) {
            self.run_edits.remove(&run_id);
        } else {
            self.run_edits.insert(run_id, edit);
        }
        Ok(())
    }

    /// Record a lineup (already validated).
    pub fn set_lineup(&mut self, run_id: Uuid, lineup: LineupEdit) -> Result<(), String> {
        let run = self.base_run(run_id)?.clone();
        let extras = self.run_extras.entry(run_id).or_default();
        extras.lineup = Some(lineup);
        extras.prune(&run);
        if extras.is_empty() {
            self.run_extras.remove(&run_id);
        }
        Ok(())
    }

    /// Record hand times / revert to the light barrier.
    pub fn set_hand_times(&mut self, run_id: Uuid, hand: HandTimesEdit) -> Result<(), String> {
        let run = self.base_run(run_id)?.clone();
        let extras = self.run_extras.entry(run_id).or_default();
        extras.hand_times = Some(hand);
        extras.prune(&run);
        if extras.is_empty() {
            self.run_extras.remove(&run_id);
        }
        Ok(())
    }

    /// Insert / replace a penalty row. A row equal to the base's is not
    /// kept as an edit.
    pub fn upsert_penalty(&mut self, penalty: Penalty) {
        let same_as_base = self.base.penalties.contains(&penalty);
        if same_as_base {
            self.penalty_edits.remove(&penalty.id);
        } else {
            self.penalty_edits.insert(penalty.id, penalty);
        }
    }

    /// Penalty as currently visible (view semantics), by id.
    pub fn penalty(&self, id: Uuid) -> Option<Penalty> {
        self.penalty_edits
            .get(&id)
            .cloned()
            .or_else(|| self.base.penalties.iter().find(|p| p.id == id).cloned())
    }

    /// The penalty exists only on this desktop (never seen by the web).
    pub fn is_local_only_penalty(&self, id: Uuid) -> bool {
        self.penalty_edits.contains_key(&id) && !self.base.penalties.iter().any(|p| p.id == id)
    }

    /// Remove a desktop-only penalty (judge withdrew it). Returns whether
    /// something was removed; rows known to the web are never deleted.
    pub fn delete_local_penalty(&mut self, id: Uuid) -> bool {
        if self.is_local_only_penalty(id) {
            self.penalty_edits.remove(&id);
            true
        } else {
            false
        }
    }

    /// Set the organizer's current runs (already validated). Equal to the
    /// base → override removed.
    pub fn set_current_runs(&mut self, ids: Vec<Uuid>) {
        if ids == self.base.event.current_run_ids {
            self.current_run_ids = None;
        } else {
            self.current_run_ids = Some(ids);
        }
    }

    /// The web accepted a result POST: take its run row as the new base
    /// and drop the edit if it was the one posted (a newer local edit
    /// recorded meanwhile stays).
    pub fn acknowledge(&mut self, posted: &RunResultRequest, server_run: Run) {
        let run_id = server_run.id;
        if let Some(edit) = self.run_edits.get(&run_id) {
            if edit.to_request() == *posted || edit.same_result_as(&server_run) {
                self.run_edits.remove(&run_id);
            }
        }
        if let Some(slot) = self.base.runs.iter_mut().find(|r| r.id == run_id) {
            *slot = server_run;
        }
    }

    /// Drop overlays the base already reflects (after the base changed).
    fn prune_against_base(&mut self) {
        let base = &self.base;
        self.run_edits.retain(|run_id, edit| {
            base.runs
                .iter()
                .find(|r| r.id == *run_id)
                .is_none_or(|r| !edit.applied_in(r))
        });
        self.run_extras.retain(|run_id, extras| {
            if let Some(run) = base.runs.iter().find(|r| r.id == *run_id) {
                extras.prune(run);
            }
            !extras.is_empty()
        });
        self.penalty_edits
            .retain(|id, edit| !base.penalties.iter().any(|p| p.id == *id && p == edit));
    }

    /// Replace the base with a newer snapshot of the same discipline,
    /// keeping local overlays on top. Overlays the web already reflects
    /// are dropped; edits for runs that no longer exist are kept (they
    /// surface as divergence).
    pub fn adopt_remote(&mut self, remote: DisciplinePayload) {
        self.base = remote;
        self.prune_against_base();
        if self.current_run_ids.as_ref() == Some(&self.base.event.current_run_ids) {
            self.current_run_ids = None;
        }
    }

    /// Local timing edits whose result (status/time) differs from `remote`.
    pub fn divergence(&self, remote: &DisciplinePayload) -> Vec<RunDivergence> {
        self.run_edits
            .iter()
            .filter_map(|(run_id, edit)| {
                let remote_run = remote.runs.iter().find(|r| r.id == *run_id);
                if remote_run.is_some_and(|r| edit.same_result_as(r)) {
                    return None;
                }
                let team_id = remote_run
                    .or_else(|| self.base.runs.iter().find(|r| r.id == *run_id))
                    .map(|r| r.team_id);
                Some(RunDivergence {
                    run_id: *run_id,
                    run_number: edit.run_number,
                    team_id,
                    local_status: edit.status,
                    local_time_ms: edit.original_time_ms,
                    remote_status: remote_run.map(|r| r.status),
                    remote_time_ms: remote_run.and_then(|r| r.original_time_ms),
                })
            })
            .collect()
    }

    /// Penalties / lineups / hand times not on the web yet.
    pub fn unsynced_summary(&self) -> UnsyncedSummary {
        UnsyncedSummary {
            penalties: self.penalty_edits.len(),
            lineups: self
                .run_extras
                .values()
                .filter(|e| e.lineup.is_some())
                .count(),
            hand_times: self
                .run_extras
                .values()
                .filter(|e| e.hand_times.is_some())
                .count(),
        }
    }

    /// Drop timing edits whose result already matches `remote` (e.g.
    /// uploaded via another way) — used before adopting a remote snapshot.
    pub fn drop_edits_matching(&mut self, remote: &DisciplinePayload) {
        self.run_edits.retain(|run_id, edit| {
            !remote
                .runs
                .iter()
                .any(|r| r.id == *run_id && edit.same_result_as(r))
        });
    }

    /// "Lokale Änderungen verwerfen": the web's state wins.
    pub fn discard_edits(&mut self) {
        self.run_edits.clear();
        self.run_extras.clear();
        self.penalty_edits.clear();
        self.current_run_ids = None;
    }

    /// Body of the on-prem upload (contract A3): the runs changed on this
    /// desktop (timing, lineup or hand times; full A3 field set from the
    /// view, by run number) and **all** penalties of the view; the web's
    /// current runs are cleared.
    pub fn upload_payload(&self) -> OnPremUploadRequest {
        let view = self.view();
        let changed: BTreeSet<Uuid> = self
            .run_edits
            .keys()
            .chain(self.run_extras.keys())
            .copied()
            .collect();
        let mut runs: Vec<OnPremUploadRun> = view
            .runs
            .iter()
            .filter(|r| changed.contains(&r.id))
            .map(OnPremUploadRun::from_run)
            .collect();
        runs.sort_by_key(|r| (r.run_number, r.id));
        OnPremUploadRequest {
            runs,
            penalties: view.penalties,
            current_run_ids: Vec::new(),
        }
    }

    /// The web accepted `uploaded`: write it into the base and drop the
    /// overlays it covered. Overlays recorded after the payload was built
    /// (a phone submitted meanwhile) stay.
    pub fn mark_synced(&mut self, uploaded: &OnPremUploadRequest, now: String) {
        for up in &uploaded.runs {
            if let Some(run) = self.base.runs.iter_mut().find(|r| r.id == up.id) {
                up.apply_to(run);
            }
        }
        for p in &uploaded.penalties {
            match self.base.penalties.iter_mut().find(|b| b.id == p.id) {
                Some(slot) => *slot = p.clone(),
                None => self.base.penalties.push(p.clone()),
            }
        }
        self.base.event.current_run_ids = uploaded.current_run_ids.clone();
        self.base.event.current_run_id = uploaded.current_run_ids.first().copied();
        self.base.sync_approved_penalties();
        self.prune_against_base();
        self.last_upload_at = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{id, sample_payload, PENALTY_1, RUNNER_A1, RUN_A1, RUN_B1};
    use crate::cloud::types::{PenaltyKind, StationKind};

    fn completed(run_number: u32, ms: u32) -> RunEdit {
        RunEdit {
            run_number,
            status: RunStatus::Completed,
            original_time_ms: Some(ms),
            started_at: Some("2027-06-12T10:00:00Z".into()),
            ended_at: Some("2027-06-12T10:00:47Z".into()),
            recorded_at: "2027-06-12T10:00:48Z".into(),
        }
    }

    fn local_penalty(run: &str, status: PenaltyStatus, created_at: &str) -> Penalty {
        let base = sample_payload();
        let mut p = base.penalties[0].clone();
        p.id = Uuid::new_v4();
        p.run_id = id(run);
        p.status = status;
        p.created_at = Some(created_at.into());
        p.reviewed_at = None;
        p.reviewed_by = None;
        p
    }

    #[test]
    fn view_applies_edits() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        let view = store.view();
        let run = view.runs.iter().find(|r| r.id == id(RUN_A1)).unwrap();
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.original_time_ms, Some(47_240));
        // Base untouched, other runs untouched.
        assert_eq!(store.base.runs[0].status, RunStatus::Pending);
        assert_eq!(view.runs[1], store.base.runs[1]);
    }

    #[test]
    fn view_without_overlays_equals_base() {
        let store = LocalStore::new(sample_payload());
        assert_eq!(store.view(), store.base);
    }

    #[test]
    fn record_unknown_run_fails() {
        let mut store = LocalStore::new(sample_payload());
        assert!(store.record(Uuid::new_v4(), completed(9, 1)).is_err());
        assert!(!store.has_edits());
    }

    #[test]
    fn record_replaces_previous_edit_last_write_wins() {
        let mut store = LocalStore::new(sample_payload());
        let mut active = completed(1, 0);
        active.status = RunStatus::Active;
        active.original_time_ms = None;
        active.ended_at = None;
        store.record(id(RUN_A1), active).unwrap();
        store.record(id(RUN_A1), completed(1, 50_000)).unwrap();
        assert_eq!(store.run_edits.len(), 1);
        assert_eq!(store.run_edits[&id(RUN_A1)].original_time_ms, Some(50_000));
    }

    #[test]
    fn record_matching_base_is_dropped() {
        let mut store = LocalStore::new(sample_payload());
        let pending = RunEdit {
            run_number: 1,
            status: RunStatus::Pending,
            original_time_ms: None,
            started_at: None,
            ended_at: None,
            recorded_at: "x".into(),
        };
        store.record(id(RUN_A1), pending).unwrap();
        assert!(!store.has_edits());
    }

    #[test]
    fn acknowledge_drops_edit_and_takes_server_row() {
        let mut store = LocalStore::new(sample_payload());
        let edit = completed(1, 47_240);
        store.record(id(RUN_A1), edit.clone()).unwrap();
        let mut server = store.view().runs[0].clone();
        server.correction_ms = 15_000;
        server.ended_at = Some("2027-06-12T10:00:47+00:00".into()); // normalized
        store.acknowledge(&edit.to_request(), server.clone());
        assert!(!store.has_edits());
        assert_eq!(store.base.runs[0], server);
    }

    #[test]
    fn acknowledge_keeps_newer_edit() {
        let mut store = LocalStore::new(sample_payload());
        let first = completed(1, 47_240);
        store.record(id(RUN_A1), first.clone()).unwrap();
        let mut server = store.view().runs[0].clone();
        store.record(id(RUN_A1), completed(1, 48_000)).unwrap();
        server.original_time_ms = Some(47_240);
        store.acknowledge(&first.to_request(), server);
        assert_eq!(
            store.view().runs[0].original_time_ms,
            Some(48_000),
            "newer local edit must win"
        );
    }

    #[test]
    fn adopt_remote_keeps_local_edits_on_top() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        let mut remote = sample_payload();
        remote.teams[0].name = "FF Alpha (neu)".into();
        store.adopt_remote(remote);
        let view = store.view();
        assert_eq!(view.teams[0].name, "FF Alpha (neu)");
        assert_eq!(view.runs[0].original_time_ms, Some(47_240));
    }

    #[test]
    fn adopt_remote_drops_edits_the_web_already_has() {
        let mut store = LocalStore::new(sample_payload());
        let edit = completed(1, 47_240);
        store.record(id(RUN_A1), edit.clone()).unwrap();
        let mut remote = sample_payload();
        edit.apply_to(&mut remote.runs[0]);
        store.adopt_remote(remote);
        assert!(!store.has_edits());
    }

    #[test]
    fn divergence_lists_only_differing_results() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        store.record(id(RUN_B1), completed(2, 51_000)).unwrap();
        let mut remote = sample_payload();
        remote.runs[1].status = RunStatus::Completed;
        remote.runs[1].original_time_ms = Some(51_000);
        let div = store.divergence(&remote);
        assert_eq!(div.len(), 1);
        assert_eq!(div[0].run_id, id(RUN_A1));
        assert_eq!(div[0].local_status, RunStatus::Completed);
        assert_eq!(div[0].local_time_ms, Some(47_240));
        assert_eq!(div[0].remote_status, Some(RunStatus::Pending));
        assert_eq!(div[0].remote_time_ms, None);
    }

    #[test]
    fn divergence_for_run_missing_on_web() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        let mut remote = sample_payload();
        remote.runs.retain(|r| r.id != id(RUN_A1));
        let div = store.divergence(&remote);
        assert_eq!(div.len(), 1);
        assert_eq!(div[0].remote_status, None);
        assert!(div[0].team_id.is_some());
    }

    #[test]
    fn drop_matching_and_discard() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        store.record(id(RUN_B1), completed(2, 51_000)).unwrap();
        let mut remote = sample_payload();
        remote.runs[0].status = RunStatus::Completed;
        remote.runs[0].original_time_ms = Some(47_240);
        store.drop_edits_matching(&remote);
        assert_eq!(store.run_edits.len(), 1);
        store
            .set_hand_times(
                id(RUN_B1),
                HandTimesEdit::Set {
                    hand_time_1_ms: 1,
                    hand_time_2_ms: 1,
                },
            )
            .unwrap();
        store.upsert_penalty(local_penalty(RUN_A1, PenaltyStatus::Pending, "x"));
        store.set_current_runs(vec![id(RUN_B1)]);
        store.discard_edits();
        assert!(!store.has_edits());
        assert!(store.current_run_ids.is_none());
    }

    #[test]
    fn serde_round_trip() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        store
            .set_lineup(
                id(RUN_B1),
                LineupEdit {
                    runner_ids: vec![],
                    confirmed_at: Some("t".into()),
                    confirmed_by: Some(LineupConfirmedBy::Organizer),
                },
            )
            .unwrap();
        store
            .set_hand_times(
                id(RUN_A1),
                HandTimesEdit::Set {
                    hand_time_1_ms: 47_230,
                    hand_time_2_ms: 47_245,
                },
            )
            .unwrap();
        store.upsert_penalty(local_penalty(RUN_A1, PenaltyStatus::Pending, "2027"));
        store.set_current_runs(vec![id(RUN_B1)]);
        store.onprem_token = Some("abc".into());
        let json = serde_json::to_string(&store).unwrap();
        let back: LocalStore = serde_json::from_str(&json).unwrap();
        assert_eq!(back, store);
    }

    // ── lineup / hand times ──────────────────────────────────────────

    #[test]
    fn lineup_overlay_applies_and_counts_as_unsynced() {
        let mut store = LocalStore::new(sample_payload());
        let lineup = LineupEdit {
            runner_ids: vec![id(RUNNER_A1)],
            confirmed_at: Some("2027-06-12T10:00:00.000Z".into()),
            confirmed_by: Some(LineupConfirmedBy::Organizer),
        };
        store.set_lineup(id(RUN_A1), lineup).unwrap();
        let run = store.view().runs[0].clone();
        assert_eq!(run.lineup, vec![id(RUNNER_A1)]);
        assert_eq!(run.lineup_confirmed_by, Some(LineupConfirmedBy::Organizer));
        assert_eq!(store.unsynced_summary().lineups, 1);
        assert!(store.has_edits());
    }

    #[test]
    fn lineup_equal_to_base_is_not_an_edit() {
        let mut store = LocalStore::new(sample_payload());
        let run = store.base.runs[0].clone();
        store
            .set_lineup(
                run.id,
                LineupEdit {
                    runner_ids: run.lineup.clone(),
                    confirmed_at: run.lineup_confirmed_at.clone(),
                    confirmed_by: run.lineup_confirmed_by,
                },
            )
            .unwrap();
        assert!(!store.has_edits());
    }

    #[test]
    fn hand_times_set_uses_kido_core_mean() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_A1), completed(1, 99_000)).unwrap();
        store
            .set_hand_times(
                id(RUN_A1),
                HandTimesEdit::Set {
                    hand_time_1_ms: 47_230,
                    hand_time_2_ms: 47_245,
                },
            )
            .unwrap();
        let run = store.view().runs[0].clone();
        assert_eq!(run.time_source, TimeSource::Handstopp);
        assert_eq!(run.original_time_ms, Some(47_240));
        assert_eq!(run.hand_time_1_ms, Some(47_230));
        assert_eq!(run.correction_ms, 0);
        assert_eq!(store.unsynced_summary().hand_times, 1);
    }

    #[test]
    fn hand_times_clear_keeps_time() {
        let mut base = sample_payload();
        base.runs[0].time_source = TimeSource::Handstopp;
        base.runs[0].hand_time_1_ms = Some(47_230);
        base.runs[0].hand_time_2_ms = Some(47_245);
        base.runs[0].original_time_ms = Some(47_240);
        let mut store = LocalStore::new(base);
        store
            .set_hand_times(
                id(RUN_A1),
                HandTimesEdit::Clear {
                    keep_time_ms: Some(47_240),
                },
            )
            .unwrap();
        let run = store.view().runs[0].clone();
        assert_eq!(run.time_source, TimeSource::Lichtschranke);
        assert_eq!(run.hand_time_1_ms, None);
        assert_eq!(run.original_time_ms, Some(47_240));
    }

    #[test]
    fn hand_times_clear_on_light_barrier_run_is_noop() {
        let mut store = LocalStore::new(sample_payload());
        store
            .set_hand_times(id(RUN_A1), HandTimesEdit::Clear { keep_time_ms: None })
            .unwrap();
        assert!(!store.has_edits());
    }

    #[test]
    fn extras_for_unknown_run_fail() {
        let mut store = LocalStore::new(sample_payload());
        assert!(store
            .set_hand_times(Uuid::new_v4(), HandTimesEdit::Clear { keep_time_ms: None })
            .is_err());
        assert!(store
            .set_lineup(
                Uuid::new_v4(),
                LineupEdit {
                    runner_ids: vec![],
                    confirmed_at: None,
                    confirmed_by: None
                }
            )
            .is_err());
    }

    // ── penalties ────────────────────────────────────────────────────

    #[test]
    fn new_penalty_appears_in_view_and_approved_list_follows_status() {
        let mut store = LocalStore::new(sample_payload());
        let p = local_penalty(RUN_A1, PenaltyStatus::Pending, "2027-06-12T10:01:00.000Z");
        store.upsert_penalty(p.clone());
        let view = store.view();
        assert_eq!(view.penalties.len(), 2);
        assert_eq!(view.penalties[1].id, p.id);
        assert_eq!(view.approved_penalties.len(), 1, "pending not approved");
        assert_eq!(store.unsynced_summary().penalties, 1);

        let mut approved = p;
        approved.status = PenaltyStatus::Approved;
        store.upsert_penalty(approved);
        let view = store.view();
        assert_eq!(view.approved_penalties.len(), 2);
        // Legacy correction_ms recomputed: two approved SZ.
        assert_eq!(view.runs[0].correction_ms, 30_000);
    }

    #[test]
    fn edited_base_penalty_replaces_row_in_place() {
        let mut store = LocalStore::new(sample_payload());
        let mut p = store.base.penalties[0].clone();
        p.status = PenaltyStatus::Rejected;
        store.upsert_penalty(p);
        let view = store.view();
        assert_eq!(view.penalties.len(), 1);
        assert_eq!(view.penalties[0].status, PenaltyStatus::Rejected);
        assert!(view.approved_penalties.is_empty());
        assert_eq!(view.runs[0].correction_ms, 0);
    }

    #[test]
    fn penalty_equal_to_base_is_not_an_edit() {
        let mut store = LocalStore::new(sample_payload());
        let p = store.base.penalties[0].clone();
        store.upsert_penalty(p);
        assert!(!store.has_edits());
    }

    #[test]
    fn added_penalties_sorted_by_created_at() {
        let mut store = LocalStore::new(sample_payload());
        let late = local_penalty(RUN_A1, PenaltyStatus::Pending, "2027-06-12T10:05:00.000Z");
        let early = local_penalty(RUN_B1, PenaltyStatus::Pending, "2027-06-12T10:01:00.000Z");
        store.upsert_penalty(late.clone());
        store.upsert_penalty(early.clone());
        let ids: Vec<Uuid> = store.view().penalties.iter().map(|p| p.id).collect();
        assert_eq!(ids, vec![id(PENALTY_1), early.id, late.id]);
    }

    #[test]
    fn delete_only_local_penalties() {
        let mut store = LocalStore::new(sample_payload());
        let p = local_penalty(RUN_A1, PenaltyStatus::Pending, "x");
        store.upsert_penalty(p.clone());
        assert!(store.is_local_only_penalty(p.id));
        assert!(!store.is_local_only_penalty(id(PENALTY_1)));
        assert!(!store.delete_local_penalty(id(PENALTY_1)));
        assert!(store.delete_local_penalty(p.id));
        assert!(store.penalty(p.id).is_none());
        assert!(store.penalty(id(PENALTY_1)).is_some());
    }

    // ── current runs ─────────────────────────────────────────────────

    #[test]
    fn current_runs_override_and_reset() {
        let mut store = LocalStore::new(sample_payload());
        store.set_current_runs(vec![id(RUN_B1)]);
        let view = store.view();
        assert_eq!(view.event.current_run_ids, vec![id(RUN_B1)]);
        assert_eq!(view.event.current_run_id, Some(id(RUN_B1)));
        assert!(!store.has_edits(), "desk-local, never uploaded");
        store.set_current_runs(store.base.event.current_run_ids.clone());
        assert!(store.current_run_ids.is_none());
        store.set_current_runs(vec![]);
        assert!(store.view().event.current_run_ids.is_empty());
        assert_eq!(store.view().event.current_run_id, None);
    }

    // ── upload ───────────────────────────────────────────────────────

    #[test]
    fn upload_payload_has_only_changed_runs_and_all_penalties() {
        let mut store = LocalStore::new(sample_payload());
        assert!(store.upload_payload().runs.is_empty());
        assert_eq!(store.upload_payload().penalties.len(), 1);

        store.record(id(RUN_B1), completed(2, 51_000)).unwrap();
        store
            .set_lineup(
                id(RUN_A1),
                LineupEdit {
                    runner_ids: vec![id(RUNNER_A1)],
                    confirmed_at: Some("t".into()),
                    confirmed_by: Some(LineupConfirmedBy::Startlinie),
                },
            )
            .unwrap();
        let pending = local_penalty(RUN_B1, PenaltyStatus::Pending, "2027-06-12T10:01:00.000Z");
        let mut rejected =
            local_penalty(RUN_B1, PenaltyStatus::Rejected, "2027-06-12T10:02:00.000Z");
        rejected.kind = PenaltyKind::Fv;
        store.upsert_penalty(pending);
        store.upsert_penalty(rejected);

        let up = store.upload_payload();
        let run_ids: Vec<Uuid> = up.runs.iter().map(|r| r.id).collect();
        assert_eq!(run_ids, vec![id(RUN_A1), id(RUN_B1)], "by run number");
        assert_eq!(up.runs[0].lineup, vec![id(RUNNER_A1)]);
        assert_eq!(up.runs[1].original_time_ms, Some(51_000));
        assert_eq!(up.runs[1].status, RunStatus::Completed);
        let statuses: Vec<PenaltyStatus> = up.penalties.iter().map(|p| p.status).collect();
        assert_eq!(
            statuses,
            vec![
                PenaltyStatus::Approved,
                PenaltyStatus::Pending,
                PenaltyStatus::Rejected
            ]
        );
        assert!(up.current_run_ids.is_empty());
    }

    #[test]
    fn mark_synced_folds_upload_into_base() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        store
            .set_hand_times(
                id(RUN_B1),
                HandTimesEdit::Set {
                    hand_time_1_ms: 50_000,
                    hand_time_2_ms: 50_020,
                },
            )
            .unwrap();
        let p = local_penalty(RUN_A1, PenaltyStatus::Approved, "2027-06-12T10:01:00.000Z");
        store.upsert_penalty(p.clone());
        store.set_current_runs(vec![id(RUN_B1)]);
        let view_before = store.view();
        let up = store.upload_payload();
        store.mark_synced(&up, "2027-06-12T18:00:00.000Z".into());

        assert!(!store.has_edits());
        assert_eq!(
            store.last_upload_at.as_deref(),
            Some("2027-06-12T18:00:00.000Z")
        );
        assert_eq!(store.base.runs[0].original_time_ms, Some(47_240));
        assert_eq!(store.base.runs[1].original_time_ms, Some(50_010));
        assert!(store.base.penalties.iter().any(|b| b.id == p.id));
        assert_eq!(store.base.approved_penalties.len(), 2);
        assert!(store.base.event.current_run_ids.is_empty(), "web cleared");
        // The desk-local current-run choice stays for the rest of the day.
        assert_eq!(store.view().event.current_run_ids, vec![id(RUN_B1)]);
        let runs_after: Vec<_> = store
            .view()
            .runs
            .iter()
            .map(OnPremUploadRun::from_run)
            .collect();
        let runs_before: Vec<_> = view_before
            .runs
            .iter()
            .map(OnPremUploadRun::from_run)
            .collect();
        assert_eq!(runs_after, runs_before);
    }

    #[test]
    fn mark_synced_keeps_changes_recorded_after_the_payload() {
        let mut store = LocalStore::new(sample_payload());
        let first = local_penalty(RUN_A1, PenaltyStatus::Pending, "2027-06-12T10:01:00.000Z");
        store.upsert_penalty(first.clone());
        let up = store.upload_payload();
        // A phone submits while the upload is in flight …
        let late = local_penalty(RUN_A1, PenaltyStatus::Pending, "2027-06-12T10:02:00.000Z");
        store.upsert_penalty(late.clone());
        // … and the organizer approves the first one.
        let mut approved = first.clone();
        approved.status = PenaltyStatus::Approved;
        store.upsert_penalty(approved);
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        store.mark_synced(&up, "now".into());
        assert!(store.penalty_edits.contains_key(&late.id));
        assert_eq!(
            store.penalty_edits[&first.id].status,
            PenaltyStatus::Approved,
            "newer review not lost"
        );
        assert!(store.run_edits.contains_key(&id(RUN_A1)));
    }

    #[test]
    fn upload_twice_is_idempotent_locally() {
        let mut store = LocalStore::new(sample_payload());
        store.upsert_penalty(local_penalty(RUN_A1, PenaltyStatus::Pending, "x"));
        let up = store.upload_payload();
        store.mark_synced(&up, "a".into());
        let base_once = store.base.clone();
        store.mark_synced(&up, "b".into());
        assert_eq!(store.base, base_once);
    }

    #[test]
    fn adopt_remote_drops_overlays_the_web_already_has() {
        let mut store = LocalStore::new(sample_payload());
        let p = local_penalty(RUN_A1, PenaltyStatus::Pending, "x");
        store.upsert_penalty(p.clone());
        let lineup = LineupEdit {
            runner_ids: vec![id(RUNNER_A1)],
            confirmed_at: Some("t".into()),
            confirmed_by: Some(LineupConfirmedBy::Startlinie),
        };
        store.set_lineup(id(RUN_B1), lineup.clone()).unwrap();
        store.set_current_runs(vec![id(RUN_B1)]);

        let mut remote = sample_payload();
        remote.penalties.push(p);
        lineup.apply_to(&mut remote.runs[1]);
        remote.event.current_run_ids = vec![id(RUN_B1)];
        store.adopt_remote(remote);
        assert!(!store.has_edits());
        assert!(store.current_run_ids.is_none());
    }

    #[test]
    fn adopt_remote_keeps_differing_overlays() {
        let mut store = LocalStore::new(sample_payload());
        let mut p = store.base.penalties[0].clone();
        p.status = PenaltyStatus::Rejected;
        p.station = Some(StationKind::Knotengestell);
        store.upsert_penalty(p);
        store.adopt_remote(sample_payload());
        assert_eq!(store.unsynced_summary().penalties, 1);
    }
}
