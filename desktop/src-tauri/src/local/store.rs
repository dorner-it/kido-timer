//! In-memory local result store for one discipline.
//!
//! Model: `base` is the last snapshot received from the web (live poll,
//! picker selection) or read from a `.kido` file. `run_edits` are timing
//! changes recorded on this desktop that the web has not acknowledged yet.
//! The [`LocalStore::view`] (base with edits applied) is what the UI shows,
//! what results are computed from and what a `.kido` export contains.
//!
//! - On-prem: every recorded change stays an edit (nothing is posted).
//! - Live: an edit lives until its POST succeeds ([`LocalStore::acknowledge`]);
//!   failed posts keep the edit, so a later poll never hides a local time.
//! - Switching back to live: [`LocalStore::divergence`] lists edits that
//!   differ from the web, the operator uploads or discards them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cloud::types::{DisciplinePayload, Run, RunDivergence, RunResultRequest, RunStatus};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalStore {
    pub base: DisciplinePayload,
    #[serde(default)]
    pub run_edits: BTreeMap<Uuid, RunEdit>,
}

impl LocalStore {
    pub fn new(base: DisciplinePayload) -> Self {
        Self {
            base,
            run_edits: BTreeMap::new(),
        }
    }

    pub fn discipline_id(&self) -> Uuid {
        self.base.discipline.id
    }

    pub fn has_edits(&self) -> bool {
        !self.run_edits.is_empty()
    }

    /// Base snapshot with all local edits applied.
    pub fn view(&self) -> DisciplinePayload {
        let mut view = self.base.clone();
        for run in &mut view.runs {
            if let Some(edit) = self.run_edits.get(&run.id) {
                edit.apply_to(run);
            }
        }
        view
    }

    /// Record a timing change. The run must exist in the snapshot. An edit
    /// that the base already reflects is dropped (nothing to sync).
    pub fn record(&mut self, run_id: Uuid, edit: RunEdit) -> Result<(), String> {
        let run = self
            .base
            .runs
            .iter()
            .find(|r| r.id == run_id)
            .ok_or_else(|| format!("Lauf {run_id} ist im geladenen Wettkampf nicht vorhanden"))?;
        if edit.applied_in(run) {
            self.run_edits.remove(&run_id);
        } else {
            self.run_edits.insert(run_id, edit);
        }
        Ok(())
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

    /// Replace the base with a newer snapshot of the same discipline,
    /// keeping local edits on top. Edits for runs that no longer exist are
    /// kept (they surface as divergence).
    pub fn adopt_remote(&mut self, remote: DisciplinePayload) {
        self.base = remote;
        let base = &self.base;
        self.run_edits.retain(|run_id, edit| {
            base.runs
                .iter()
                .find(|r| r.id == *run_id)
                .is_none_or(|r| !edit.applied_in(r))
        });
    }

    /// Local edits whose result (status/time) differs from `remote`.
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

    /// Drop edits whose result already matches `remote` (e.g. uploaded via
    /// another way) — used before adopting a remote snapshot.
    pub fn drop_edits_matching(&mut self, remote: &DisciplinePayload) {
        self.run_edits.retain(|run_id, edit| {
            !remote
                .runs
                .iter()
                .any(|r| r.id == *run_id && edit.same_result_as(r))
        });
    }

    pub fn discard_edits(&mut self) {
        self.run_edits.clear();
    }

    /// Edits as result POST bodies, ordered by run number (upload order).
    pub fn pending_uploads(&self) -> Vec<(Uuid, RunResultRequest)> {
        let mut out: Vec<(Uuid, RunResultRequest)> = self
            .run_edits
            .iter()
            .map(|(id, e)| (*id, e.to_request()))
            .collect();
        out.sort_by_key(|(id, r)| (r.run_number, *id));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{id, sample_payload, RUN_A1, RUN_B1};

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
        // Server answers for the first POST, but a correction was recorded
        // meanwhile.
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
        // B1 already uploaded with the same result (timestamps differ).
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
        store.discard_edits();
        assert!(!store.has_edits());
    }

    #[test]
    fn pending_uploads_ordered_by_run_number() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_B1), completed(2, 51_000)).unwrap();
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        let uploads = store.pending_uploads();
        assert_eq!(uploads.len(), 2);
        assert_eq!(uploads[0].0, id(RUN_A1));
        assert_eq!(uploads[0].1.original_time_ms, Some(47_240));
        assert_eq!(uploads[1].0, id(RUN_B1));
    }

    #[test]
    fn serde_round_trip() {
        let mut store = LocalStore::new(sample_payload());
        store.record(id(RUN_A1), completed(1, 47_240)).unwrap();
        let json = serde_json::to_string(&store).unwrap();
        let back: LocalStore = serde_json::from_str(&json).unwrap();
        assert_eq!(back, store);
    }
}
