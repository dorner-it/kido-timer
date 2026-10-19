//! Judge API of the on-prem server — pure handlers (no sockets, no I/O).
//!
//! Mirrors the cloud station flow (`backend/src/routes/decider.rs`,
//! `backend/src/routes/catalogue.rs`) so the very same judge UI works
//! against the desktop: same JSON shapes, same validation order, same
//! error envelope `{ "error": { "code", "message" } }`. Differences:
//!
//! - auth: `owner_sub` must be the loaded snapshot's owner (else `404`,
//!   like an unknown owner on the web) and `token` the on-prem access code
//!   of the discipline (else `401`),
//! - the state is never `disabled` (on-prem *is* the active mode),
//! - a judge withdrawing a penalty that the web already knows (synced
//!   earlier) marks it `rejected` instead of deleting it — the upload
//!   (A3) can only upsert, never delete.
//!
//! All writes go through [`LocalStore`]; the caller persists afterwards.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::store::{LineupEdit, LocalStore};
use crate::cloud::types::{
    DisciplinePayload, JudgeMode, LineupConfirmedBy, Penalty, PenaltyKind, PenaltyStatus, Run,
    Runner, StationKind, Team, TimerMode,
};

/// `reviewed_by` of a penalty a judge withdrew after it was synced.
pub const WITHDRAWN_BY: &str = "Strafrichter (zurückgezogen)";

// ===== errors =================================================================

/// Same variants / codes / messages as the backend's `AppError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    NotFound(String),
    Validation(String),
    Unauthorized,
    Forbidden,
    Conflict(String),
    RateLimited,
    PayloadTooLarge,
    Internal(String),
}

impl ApiError {
    pub fn status(&self) -> u16 {
        match self {
            ApiError::NotFound(_) => 404,
            ApiError::Validation(_) => 400,
            ApiError::Unauthorized => 401,
            ApiError::Forbidden => 403,
            ApiError::Conflict(_) => 409,
            ApiError::RateLimited => 429,
            ApiError::PayloadTooLarge => 413,
            ApiError::Internal(_) => 500,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ApiError::NotFound(_) => "not_found",
            ApiError::Validation(_) => "validation",
            ApiError::Unauthorized => "unauthorized",
            ApiError::Forbidden => "forbidden",
            ApiError::Conflict(_) => "conflict",
            ApiError::RateLimited => "rate_limited",
            ApiError::PayloadTooLarge => "payload_too_large",
            ApiError::Internal(_) => "internal",
        }
    }

    /// Public message (internal details are never exposed).
    pub fn message(&self) -> String {
        match self {
            ApiError::NotFound(m) => format!("not found: {m}"),
            ApiError::Validation(m) => format!("validation: {m}"),
            ApiError::Unauthorized => "unauthorized".into(),
            ApiError::Forbidden => "forbidden".into(),
            ApiError::Conflict(m) => format!("conflict: {m}"),
            ApiError::RateLimited => "rate limited".into(),
            ApiError::PayloadTooLarge => "request body too large".into(),
            ApiError::Internal(_) => "internal server error".into(),
        }
    }

    pub fn body(&self) -> serde_json::Value {
        serde_json::json!({ "error": { "code": self.code(), "message": self.message() } })
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

// ===== wire types =============================================================

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogueItem {
    pub code: &'static str,
    pub station: StationKind,
    pub kind: PenaltyKind,
    pub label_de: &'static str,
}

/// `GET /api/catalogue` — identical to the backend.
pub fn catalogue_items() -> Vec<CatalogueItem> {
    kido_core::catalogue::CATALOGUE
        .iter()
        .map(|e| CatalogueItem {
            code: e.code,
            station: e.station,
            kind: e.kind,
            label_de: e.label_de,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PublicEvent {
    pub id: Uuid,
    pub name: String,
    pub date: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PublicDiscipline {
    pub id: Uuid,
    pub name: String,
    pub mode: TimerMode,
    pub lane_count: u8,
    pub judge_mode: JudgeMode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeciderRunView {
    pub run: Run,
    pub team: Team,
    /// Legacy: the run's single runner. Usually `null`.
    pub runner: Option<Runner>,
    /// The team's full roster by position (Startlinie ticks the lineup).
    pub runners: Vec<Runner>,
}

/// Response of `GET /api/decider/{owner_sub}/{token}/state`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StationDeciderState {
    pub event: PublicEvent,
    pub discipline: Option<PublicDiscipline>,
    /// Kept for old clients = `current_runs[0]`.
    pub current_run: Option<DeciderRunView>,
    /// One entry per current run, ordered by lane.
    pub current_runs: Vec<DeciderRunView>,
    pub disabled: bool,
    pub allowed_stations: Vec<StationKind>,
    /// Penalties of the current runs (only `?station=` when given),
    /// oldest first.
    pub penalties: Vec<Penalty>,
}

#[derive(Debug, Default, Deserialize)]
pub struct StationQuery {
    #[serde(default)]
    pub station: Option<StationKind>,
}

/// Body of the station submit (`kind` / `delta_ms` come from the
/// catalogue; client values are not accepted).
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitStationPenaltyInput {
    pub station: StationKind,
    #[serde(default)]
    pub run_id: Option<Uuid>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub client_id: Option<Uuid>,
}

/// Body of the judge lineup endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct StationLineupInput {
    pub runner_ids: Vec<Uuid>,
    #[serde(default)]
    pub station: Option<StationKind>,
}

// ===== pure helpers (same as the backend) =====================================

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Owner + access-code check. `view` = loaded discipline (if any),
/// `expected_token` = its on-prem access code.
pub fn authorize<'a>(
    view: Option<&'a DisciplinePayload>,
    expected_token: Option<&str>,
    owner_sub: &str,
    token: &str,
) -> ApiResult<&'a DisciplinePayload> {
    let view = view.ok_or_else(|| ApiError::NotFound("no active event".into()))?;
    if view.owner_sub != owner_sub {
        return Err(ApiError::NotFound("no active event".into()));
    }
    let expected = expected_token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| ApiError::Conflict("kein Zugangscode erzeugt".into()))?;
    if !constant_time_eq(token.as_bytes(), expected.as_bytes()) {
        return Err(ApiError::Unauthorized);
    }
    Ok(view)
}

/// Keep only the runs listed in `current_ids` and order them by lane
/// (runs without a lane last, ties in `current_ids` order).
pub fn order_current_runs<'a>(runs: &'a [Run], current_ids: &[Uuid]) -> Vec<&'a Run> {
    let mut picked: Vec<(usize, &Run)> = runs
        .iter()
        .filter_map(|r| {
            current_ids
                .iter()
                .position(|id| *id == r.id)
                .map(|i| (i, r))
        })
        .collect();
    picked.sort_by_key(|(i, r)| (r.lane.unwrap_or(u8::MAX), *i));
    picked.into_iter().map(|(_, r)| r).collect()
}

/// Resolve the run a judge submission targets (backend semantics).
pub fn resolve_target_run(requested: Option<Uuid>, current_ids: &[Uuid]) -> ApiResult<Uuid> {
    if current_ids.is_empty() {
        return Err(ApiError::Conflict("no run is currently active".into()));
    }
    match requested {
        Some(id) if current_ids.contains(&id) => Ok(id),
        Some(id) => Err(ApiError::Conflict(format!(
            "run {id} ist nicht (mehr) aktuell"
        ))),
        None if current_ids.len() == 1 => Ok(current_ids[0]),
        None => Err(ApiError::Validation(
            "run_id ist erforderlich, wenn mehrere Läufe aktuell sind".into(),
        )),
    }
}

/// Station allowed for the mode; code exists and belongs to the station.
pub fn validate_station_submission(
    mode: TimerMode,
    station: StationKind,
    code: &str,
) -> ApiResult<&'static kido_core::CatalogueEntry> {
    if !station.allowed_for(mode) {
        return Err(ApiError::Validation(format!(
            "station {} is not valid for mode {mode:?}",
            station.as_str()
        )));
    }
    let entry = kido_core::catalogue::lookup(code)
        .ok_or_else(|| ApiError::Validation(format!("unbekannter Fehlercode {code}")))?;
    if entry.station != station {
        return Err(ApiError::Validation(format!(
            "Fehlercode {code} gehört nicht zur Station {}",
            station.as_str()
        )));
    }
    Ok(entry)
}

/// `delta_ms` written for a catalogue kind (legacy mirror).
pub fn catalogue_delta_ms(kind: PenaltyKind) -> i32 {
    match kind {
        PenaltyKind::Sz => kido_core::SZ_PENALTY_MS as i32,
        PenaltyKind::Fv => 0,
    }
}

/// Every id must belong to the run's team and appear at most once; an
/// empty list clears the lineup.
pub fn validate_lineup(
    runner_ids: &[Uuid],
    roster: &[Runner],
    team_id: Uuid,
) -> Result<(), String> {
    let team_runners: HashSet<Uuid> = roster
        .iter()
        .filter(|r| r.team_id == team_id)
        .map(|r| r.id)
        .collect();
    let mut seen = HashSet::with_capacity(runner_ids.len());
    for id in runner_ids {
        if !seen.insert(*id) {
            return Err(format!("Läufer {id} ist doppelt in der Besetzung"));
        }
        if !team_runners.contains(id) {
            return Err(format!("Läufer {id} gehört nicht zur Mannschaft des Laufs"));
        }
    }
    Ok(())
}

/// Lineup overlay for `runner_ids` (empty → confirmation cleared).
pub fn lineup_edit(runner_ids: Vec<Uuid>, by: LineupConfirmedBy, now: &str) -> LineupEdit {
    let empty = runner_ids.is_empty();
    LineupEdit {
        runner_ids,
        confirmed_at: (!empty).then(|| now.to_string()),
        confirmed_by: (!empty).then_some(by),
    }
}

fn roster_of(view: &DisciplinePayload, team_id: Uuid) -> Vec<Runner> {
    let mut roster: Vec<Runner> = view
        .runners
        .iter()
        .filter(|r| r.team_id == team_id)
        .cloned()
        .collect();
    roster.sort_by_key(|r| r.position);
    roster
}

fn find_run(view: &DisciplinePayload, run_id: Uuid) -> ApiResult<&Run> {
    view.runs
        .iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| ApiError::Internal("current_run_ids references unknown run".into()))
}

// ===== handlers ===============================================================

/// Current runs with team + roster, ordered by lane.
pub fn current_run_views(view: &DisciplinePayload) -> ApiResult<Vec<DeciderRunView>> {
    let current = order_current_runs(&view.runs, &view.event.current_run_ids);
    let mut out = Vec::with_capacity(current.len());
    for run in current {
        let team = view
            .teams
            .iter()
            .find(|t| t.id == run.team_id)
            .cloned()
            .ok_or_else(|| ApiError::Internal("run references unknown team".into()))?;
        let runners = roster_of(view, run.team_id);
        let runner = run
            .runner_id
            .and_then(|rid| runners.iter().find(|r| r.id == rid).cloned());
        out.push(DeciderRunView {
            run: run.clone(),
            team,
            runner,
            runners,
        });
    }
    Ok(out)
}

/// `GET …/state?station=`.
pub fn station_state(
    view: &DisciplinePayload,
    station: Option<StationKind>,
) -> ApiResult<StationDeciderState> {
    let current_runs = current_run_views(view)?;
    let current_ids: Vec<Uuid> = current_runs.iter().map(|c| c.run.id).collect();
    let mut penalties: Vec<Penalty> = if current_runs.is_empty() {
        Vec::new()
    } else {
        view.penalties
            .iter()
            .filter(|p| current_ids.contains(&p.run_id))
            .filter(|p| station.is_none_or(|s| p.station == Some(s)))
            .cloned()
            .collect()
    };
    penalties.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    let d = &view.discipline;
    Ok(StationDeciderState {
        event: PublicEvent {
            id: view.event.id,
            name: view.event.name.clone(),
            date: view.event.date.clone(),
            location: view.event.location.clone(),
        },
        discipline: Some(PublicDiscipline {
            id: d.id,
            name: d.name.clone(),
            mode: d.mode,
            lane_count: d.lane_count,
            judge_mode: d.judge_mode,
        }),
        current_run: current_runs.first().cloned(),
        current_runs,
        disabled: false,
        allowed_stations: StationKind::for_mode(d.mode).to_vec(),
        penalties,
    })
}

/// `POST …/penalties`. Returns `(created, penalty)` — `created = false`
/// for an idempotent re-submit (same `client_id`), which answers `200`.
pub fn submit_penalty(
    store: &mut LocalStore,
    input: SubmitStationPenaltyInput,
    new_id: Uuid,
    now: &str,
) -> ApiResult<(bool, Penalty)> {
    let view = store.view();
    let code = input
        .code
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| ApiError::Validation("code (Fehlercode) muss angegeben werden".into()))?;
    let entry = validate_station_submission(view.discipline.mode, input.station, code)?;
    let run_id = resolve_target_run(input.run_id, &view.event.current_run_ids)?;
    let run = find_run(&view, run_id)?;

    let penalty_id = input.client_id.unwrap_or(new_id);
    let existing = store.penalty(penalty_id);
    if let Some(existing) = existing.as_ref() {
        if existing.status != PenaltyStatus::Pending {
            return Ok((false, existing.clone()));
        }
    }
    let penalty = Penalty {
        id: penalty_id,
        event_id: run.event_id,
        discipline_id: run.discipline_id,
        run_id: run.id,
        team_id: run.team_id,
        runner_id: run.runner_id,
        kind: entry.kind,
        code: Some(entry.code.to_string()),
        station: Some(input.station),
        is_disqualification: entry.kind == PenaltyKind::Fv,
        decider_slot: existing.as_ref().map(|p| p.decider_slot).unwrap_or(0),
        delta_ms: catalogue_delta_ms(entry.kind),
        reason_code: None,
        status: PenaltyStatus::Pending,
        created_at: existing
            .as_ref()
            .and_then(|p| p.created_at.clone())
            .or_else(|| Some(now.to_string())),
        reviewed_at: None,
        reviewed_by: None,
    };
    store.upsert_penalty(penalty.clone());
    Ok((existing.is_none(), penalty))
}

/// `DELETE …/penalties/{id}?station=`.
pub fn withdraw_penalty(
    store: &mut LocalStore,
    penalty_id: Uuid,
    station: Option<StationKind>,
    now: &str,
) -> ApiResult<()> {
    let penalty = store
        .penalty(penalty_id)
        .ok_or_else(|| ApiError::NotFound(format!("penalty {penalty_id}")))?;
    if penalty.status != PenaltyStatus::Pending {
        return Err(ApiError::Conflict(
            "only pending penalties can be withdrawn".into(),
        ));
    }
    if let (Some(requested), Some(actual)) = (station, penalty.station) {
        if requested != actual {
            return Err(ApiError::Forbidden);
        }
    }
    if !store.delete_local_penalty(penalty_id) {
        // Known to the web: cannot be deleted by an upsert-only upload.
        let mut withdrawn = penalty;
        withdrawn.status = PenaltyStatus::Rejected;
        withdrawn.reviewed_at = Some(now.to_string());
        withdrawn.reviewed_by = Some(WITHDRAWN_BY.to_string());
        store.upsert_penalty(withdrawn);
    }
    Ok(())
}

/// Only the Startlinie judge may confirm a lineup.
pub fn check_lineup_station(station: Option<StationKind>) -> ApiResult<()> {
    match station {
        Some(s) if !s.records_lineup() => Err(ApiError::Forbidden),
        _ => Ok(()),
    }
}

/// `PUT …/runs/{run_id}/lineup` (Startlinie). The run must be current.
pub fn set_lineup(
    store: &mut LocalStore,
    run_id: Uuid,
    input: StationLineupInput,
    now: &str,
) -> ApiResult<Run> {
    check_lineup_station(input.station)?;
    let view = store.view();
    let run_id = resolve_target_run(Some(run_id), &view.event.current_run_ids)?;
    let run = find_run(&view, run_id)?;
    let roster = roster_of(&view, run.team_id);
    validate_lineup(&input.runner_ids, &roster, run.team_id).map_err(ApiError::Validation)?;
    store
        .set_lineup(
            run_id,
            lineup_edit(input.runner_ids, LineupConfirmedBy::Startlinie, now),
        )
        .map_err(ApiError::Internal)?;
    store
        .view()
        .runs
        .into_iter()
        .find(|r| r.id == run_id)
        .ok_or_else(|| ApiError::Internal("run vanished".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{
        id, sample_payload, PENALTY_1, RUNNER_A1, RUNNER_A2, RUNNER_B1, RUN_A1, RUN_B1, RUN_C1,
        TEAM_A,
    };

    const NOW: &str = "2027-06-12T10:01:00.000Z";
    const KNOTEN_SZ: &str = "knotengestell.ein_knoten_falsch";
    const KNOTEN_FV: &str = "knotengestell.mehrere_knoten_falsch";

    fn store() -> LocalStore {
        LocalStore::new(sample_payload())
    }

    fn submit_input(code: Option<&str>, run: Option<&str>) -> SubmitStationPenaltyInput {
        SubmitStationPenaltyInput {
            station: StationKind::Knotengestell,
            run_id: run.map(id),
            code: code.map(str::to_string),
            client_id: None,
        }
    }

    // ── errors ───────────────────────────────────────────────────────

    #[test]
    fn error_envelope_matches_backend() {
        let e = ApiError::Validation("x".into());
        assert_eq!(e.status(), 400);
        assert_eq!(
            e.body(),
            serde_json::json!({"error": {"code": "validation", "message": "validation: x"}})
        );
        assert_eq!(ApiError::Unauthorized.status(), 401);
        assert_eq!(ApiError::Unauthorized.code(), "unauthorized");
        assert_eq!(ApiError::Forbidden.status(), 403);
        assert_eq!(ApiError::NotFound("p".into()).status(), 404);
        assert_eq!(ApiError::Conflict("c".into()).status(), 409);
        assert_eq!(ApiError::RateLimited.status(), 429);
        assert_eq!(ApiError::RateLimited.code(), "rate_limited");
        assert_eq!(ApiError::PayloadTooLarge.status(), 413);
        let internal = ApiError::Internal("secret detail".into());
        assert_eq!(internal.status(), 500);
        assert!(!internal.message().contains("secret"));
    }

    // ── catalogue ────────────────────────────────────────────────────

    #[test]
    fn catalogue_has_backend_shape() {
        let items = catalogue_items();
        assert_eq!(items.len(), 30);
        let v = serde_json::to_value(&items[0]).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys.len(), 4);
        for k in ["code", "station", "kind", "label_de"] {
            assert!(keys.contains(&k), "{k}");
        }
        assert_eq!(v["station"], "startlinie");
    }

    // ── authorize ────────────────────────────────────────────────────

    #[test]
    fn authorize_accepts_owner_and_token() {
        let p = sample_payload();
        assert!(authorize(Some(&p), Some("tok"), "sub-1", "tok").is_ok());
    }

    #[test]
    fn authorize_wrong_token_is_401() {
        let p = sample_payload();
        assert_eq!(
            authorize(Some(&p), Some("tok"), "sub-1", "nope").unwrap_err(),
            ApiError::Unauthorized
        );
        assert_eq!(
            authorize(Some(&p), Some("tok"), "sub-1", "toka").unwrap_err(),
            ApiError::Unauthorized
        );
        assert_eq!(
            authorize(Some(&p), Some("tok"), "sub-1", "").unwrap_err(),
            ApiError::Unauthorized
        );
    }

    #[test]
    fn authorize_unknown_owner_or_no_discipline_is_404() {
        let p = sample_payload();
        assert_eq!(
            authorize(Some(&p), Some("tok"), "other", "tok")
                .unwrap_err()
                .status(),
            404
        );
        assert_eq!(
            authorize(None, Some("tok"), "sub-1", "tok")
                .unwrap_err()
                .status(),
            404
        );
    }

    #[test]
    fn authorize_without_token_is_409() {
        let p = sample_payload();
        assert_eq!(
            authorize(Some(&p), None, "sub-1", "x")
                .unwrap_err()
                .status(),
            409
        );
        assert_eq!(
            authorize(Some(&p), Some(""), "sub-1", "")
                .unwrap_err()
                .status(),
            409
        );
    }

    // ── helpers ──────────────────────────────────────────────────────

    #[test]
    fn order_current_runs_by_lane_then_list_order() {
        let mut p = sample_payload();
        p.runs[2].lane = None;
        let ids = vec![id(RUN_C1), id(RUN_B1), id(RUN_A1)];
        let ordered: Vec<Uuid> = order_current_runs(&p.runs, &ids)
            .iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ordered, vec![id(RUN_A1), id(RUN_B1), id(RUN_C1)]);
        assert!(order_current_runs(&p.runs, &[Uuid::new_v4()]).is_empty());
    }

    #[test]
    fn resolve_target_run_matrix() {
        let a = id(RUN_A1);
        let b = id(RUN_B1);
        assert_eq!(resolve_target_run(None, &[]).unwrap_err().status(), 409);
        assert_eq!(resolve_target_run(Some(a), &[a, b]).unwrap(), a);
        assert_eq!(
            resolve_target_run(Some(id(RUN_C1)), &[a, b])
                .unwrap_err()
                .status(),
            409
        );
        assert_eq!(resolve_target_run(None, &[a]).unwrap(), a);
        assert_eq!(resolve_target_run(None, &[a, b]).unwrap_err().status(), 400);
    }

    #[test]
    fn validate_station_submission_rules() {
        let gs = TimerMode::Gruppenstaffette;
        assert!(validate_station_submission(gs, StationKind::Knotengestell, KNOTEN_SZ).is_ok());
        let e = validate_station_submission(gs, StationKind::Podest, KNOTEN_SZ).unwrap_err();
        assert!(e.message().contains("not valid for mode"));
        let e = validate_station_submission(gs, StationKind::Knotengestell, "x.y").unwrap_err();
        assert!(e.message().contains("unbekannter Fehlercode"));
        let e =
            validate_station_submission(gs, StationKind::Verteilerlinie, KNOTEN_SZ).unwrap_err();
        assert!(e.message().contains("gehört nicht zur Station"));
    }

    #[test]
    fn validate_lineup_rules() {
        let p = sample_payload();
        let team = id(TEAM_A);
        assert!(validate_lineup(&[], &p.runners, team).is_ok());
        assert!(validate_lineup(&[id(RUNNER_A1), id(RUNNER_A2)], &p.runners, team).is_ok());
        let e = validate_lineup(&[id(RUNNER_A1), id(RUNNER_A1)], &p.runners, team).unwrap_err();
        assert!(e.contains("doppelt"));
        let e = validate_lineup(&[id(RUNNER_B1)], &p.runners, team).unwrap_err();
        assert!(e.contains("gehört nicht zur Mannschaft"));
    }

    #[test]
    fn lineup_edit_clears_confirmation_when_empty() {
        let e = lineup_edit(vec![], LineupConfirmedBy::Startlinie, NOW);
        assert_eq!(e.confirmed_at, None);
        assert_eq!(e.confirmed_by, None);
        let e = lineup_edit(vec![id(RUNNER_A1)], LineupConfirmedBy::Organizer, NOW);
        assert_eq!(e.confirmed_at.as_deref(), Some(NOW));
        assert_eq!(e.confirmed_by, Some(LineupConfirmedBy::Organizer));
    }

    // ── state ────────────────────────────────────────────────────────

    #[test]
    fn state_lists_current_runs_by_lane_with_roster() {
        let s = station_state(&sample_payload(), None).unwrap();
        assert!(!s.disabled);
        assert_eq!(s.current_runs.len(), 2);
        assert_eq!(s.current_runs[0].run.id, id(RUN_A1));
        assert_eq!(s.current_runs[1].run.id, id(RUN_B1));
        assert_eq!(s.current_run.as_ref().unwrap().run.id, id(RUN_A1));
        assert_eq!(s.current_runs[0].team.name, "FF Alpha");
        let roster: Vec<Uuid> = s.current_runs[0].runners.iter().map(|r| r.id).collect();
        assert_eq!(roster, vec![id(RUNNER_A1), id(RUNNER_A2)]);
        assert_eq!(s.allowed_stations.len(), 5);
        let d = s.discipline.as_ref().unwrap();
        assert_eq!(d.lane_count, 2);
        assert_eq!(d.judge_mode, JudgeMode::PerStation);
        assert_eq!(s.penalties.len(), 1);
    }

    #[test]
    fn state_json_shape_matches_backend() {
        let v = serde_json::to_value(station_state(&sample_payload(), None).unwrap()).unwrap();
        for key in [
            "event",
            "discipline",
            "current_run",
            "current_runs",
            "disabled",
            "allowed_stations",
            "penalties",
        ] {
            assert!(v.get(key).is_some(), "{key}");
        }
        assert_eq!(v["event"]["date"], "2027-06-12");
        assert_eq!(v["discipline"]["lane_count"], 2);
        assert_eq!(v["discipline"]["judge_mode"], "per_station");
        assert!(v["current_runs"][0]["runner"].is_null());
        assert!(v["current_runs"][0]["runners"].is_array());
        assert_eq!(v["allowed_stations"][0], "startlinie");
    }

    #[test]
    fn state_filters_penalties_by_station_and_current_runs() {
        let mut st = store();
        let (_, p) = submit_penalty(
            &mut st,
            submit_input(Some(KNOTEN_SZ), Some(RUN_B1)),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        let view = st.view();
        let all = station_state(&view, None).unwrap();
        assert_eq!(all.penalties.len(), 2);
        let knoten = station_state(&view, Some(StationKind::Knotengestell)).unwrap();
        assert_eq!(knoten.penalties.len(), 2);
        let start = station_state(&view, Some(StationKind::Startlinie)).unwrap();
        assert!(start.penalties.is_empty());

        // Run A1 no longer current → its penalty disappears.
        st.set_current_runs(vec![id(RUN_B1)]);
        let s = station_state(&st.view(), None).unwrap();
        assert_eq!(s.penalties.len(), 1);
        assert_eq!(s.penalties[0].id, p.id);
    }

    #[test]
    fn state_without_current_runs_has_no_penalties() {
        let mut st = store();
        st.set_current_runs(vec![]);
        let s = station_state(&st.view(), None).unwrap();
        assert!(s.current_runs.is_empty());
        assert!(s.current_run.is_none());
        assert!(s.penalties.is_empty());
        assert!(!s.disabled);
    }

    #[test]
    fn state_loeschangriff_allows_podest_and_angriffslinie() {
        let mut p = sample_payload();
        p.discipline.mode = TimerMode::Loeschangriff;
        let s = station_state(&p, None).unwrap();
        assert_eq!(
            s.allowed_stations,
            vec![StationKind::Podest, StationKind::Angriffslinie]
        );
    }

    #[test]
    fn state_with_unknown_team_is_internal_error() {
        let mut p = sample_payload();
        p.teams.clear();
        assert_eq!(station_state(&p, None).unwrap_err().status(), 500);
    }

    // ── submit ───────────────────────────────────────────────────────

    #[test]
    fn submit_creates_pending_penalty_from_catalogue() {
        let mut st = store();
        let new_id = Uuid::new_v4();
        let (created, p) = submit_penalty(
            &mut st,
            submit_input(Some(KNOTEN_SZ), Some(RUN_A1)),
            new_id,
            NOW,
        )
        .unwrap();
        assert!(created);
        assert_eq!(p.id, new_id);
        assert_eq!(p.kind, PenaltyKind::Sz);
        assert_eq!(p.delta_ms, 15_000);
        assert!(!p.is_disqualification);
        assert_eq!(p.status, PenaltyStatus::Pending);
        assert_eq!(p.station, Some(StationKind::Knotengestell));
        assert_eq!(p.code.as_deref(), Some(KNOTEN_SZ));
        assert_eq!(p.decider_slot, 0);
        assert_eq!(p.created_at.as_deref(), Some(NOW));
        assert_eq!(p.team_id, id(TEAM_A));
        assert!(st.view().penalties.iter().any(|x| x.id == new_id));
    }

    #[test]
    fn submit_fv_has_zero_delta_and_dq_flag() {
        let mut st = store();
        let (_, p) = submit_penalty(
            &mut st,
            submit_input(Some(KNOTEN_FV), Some(RUN_A1)),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        assert_eq!(p.kind, PenaltyKind::Fv);
        assert_eq!(p.delta_ms, 0);
        assert!(p.is_disqualification);
    }

    #[test]
    fn submit_uses_client_id_and_is_idempotent() {
        let mut st = store();
        let cid = Uuid::new_v4();
        let mut input = submit_input(Some(KNOTEN_SZ), Some(RUN_A1));
        input.client_id = Some(cid);
        let (created, first) = submit_penalty(&mut st, input.clone(), Uuid::new_v4(), NOW).unwrap();
        assert!(created);
        assert_eq!(first.id, cid);
        let (created, again) =
            submit_penalty(&mut st, input, Uuid::new_v4(), "2027-06-12T10:09:00.000Z").unwrap();
        assert!(!created, "re-submit answers 200");
        assert_eq!(again.created_at, first.created_at, "created_at kept");
        assert_eq!(st.view().penalties.len(), 2);
    }

    #[test]
    fn resubmit_of_reviewed_penalty_returns_it_unchanged() {
        let mut st = store();
        let mut input = submit_input(Some(KNOTEN_SZ), Some(RUN_A1));
        input.client_id = Some(id(PENALTY_1));
        let (created, p) = submit_penalty(&mut st, input, Uuid::new_v4(), NOW).unwrap();
        assert!(!created);
        assert_eq!(p.status, PenaltyStatus::Approved);
        assert!(!st.has_edits());
    }

    #[test]
    fn submit_without_code_is_400() {
        let mut st = store();
        for code in [None, Some(""), Some("   ")] {
            let e = submit_penalty(
                &mut st,
                submit_input(code, Some(RUN_A1)),
                Uuid::new_v4(),
                NOW,
            )
            .unwrap_err();
            assert_eq!(e.status(), 400);
            assert!(e.message().contains("code"));
        }
    }

    #[test]
    fn submit_code_is_trimmed() {
        let mut st = store();
        let code = format!("  {KNOTEN_SZ} ");
        let (_, p) = submit_penalty(
            &mut st,
            submit_input(Some(&code), Some(RUN_A1)),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        assert_eq!(p.code.as_deref(), Some(KNOTEN_SZ));
    }

    #[test]
    fn submit_unknown_code_is_400() {
        let mut st = store();
        let e = submit_penalty(
            &mut st,
            submit_input(Some("knotengestell.gibt_es_nicht"), Some(RUN_A1)),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap_err();
        assert_eq!(e.status(), 400);
    }

    #[test]
    fn submit_code_of_other_station_is_400() {
        let mut st = store();
        let mut input = submit_input(Some(KNOTEN_SZ), Some(RUN_A1));
        input.station = StationKind::Startlinie;
        let e = submit_penalty(&mut st, input, Uuid::new_v4(), NOW).unwrap_err();
        assert_eq!(e.status(), 400);
        assert!(e.message().contains("gehört nicht zur Station"));
    }

    #[test]
    fn submit_station_not_allowed_for_mode_is_400() {
        let mut st = store();
        let mut input = submit_input(Some("podest.x"), Some(RUN_A1));
        input.station = StationKind::Podest;
        let e = submit_penalty(&mut st, input, Uuid::new_v4(), NOW).unwrap_err();
        assert_eq!(e.status(), 400);
        assert!(e.message().contains("not valid for mode"));
    }

    #[test]
    fn submit_for_non_current_run_is_409() {
        let mut st = store();
        let e = submit_penalty(
            &mut st,
            submit_input(Some(KNOTEN_SZ), Some(RUN_C1)),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap_err();
        assert_eq!(e.status(), 409);
    }

    #[test]
    fn submit_without_run_id_and_two_current_runs_is_400() {
        let mut st = store();
        let e = submit_penalty(
            &mut st,
            submit_input(Some(KNOTEN_SZ), None),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap_err();
        assert_eq!(e.status(), 400);
    }

    #[test]
    fn submit_without_run_id_and_one_current_run_targets_it() {
        let mut st = store();
        st.set_current_runs(vec![id(RUN_B1)]);
        let (_, p) = submit_penalty(
            &mut st,
            submit_input(Some(KNOTEN_SZ), None),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        assert_eq!(p.run_id, id(RUN_B1));
    }

    #[test]
    fn submit_without_current_runs_is_409() {
        let mut st = store();
        st.set_current_runs(vec![]);
        let e = submit_penalty(
            &mut st,
            submit_input(Some(KNOTEN_SZ), None),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap_err();
        assert_eq!(e.status(), 409);
    }

    #[test]
    fn submit_input_wire_shape() {
        let input: SubmitStationPenaltyInput = serde_json::from_value(serde_json::json!({
            "station": "knoten", "run_id": RUN_A1, "code": KNOTEN_SZ,
            "client_id": "00000000-0000-4000-8000-0000000000ff",
            "kind": "fv", "delta_ms": 1
        }))
        .unwrap();
        assert_eq!(input.station, StationKind::Knotengestell, "legacy alias");
        let mut st = store();
        let (_, p) = submit_penalty(&mut st, input, Uuid::new_v4(), NOW).unwrap();
        assert_eq!(p.kind, PenaltyKind::Sz, "client kind ignored");
        assert_eq!(p.delta_ms, 15_000, "client delta ignored");
    }

    // ── withdraw ─────────────────────────────────────────────────────

    #[test]
    fn withdraw_local_pending_penalty_deletes_it() {
        let mut st = store();
        let (_, p) = submit_penalty(
            &mut st,
            submit_input(Some(KNOTEN_SZ), Some(RUN_A1)),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        withdraw_penalty(&mut st, p.id, Some(StationKind::Knotengestell), NOW).unwrap();
        assert!(st.penalty(p.id).is_none());
        assert!(!st.has_edits());
    }

    #[test]
    fn withdraw_unknown_penalty_is_404() {
        let mut st = store();
        assert_eq!(
            withdraw_penalty(&mut st, Uuid::new_v4(), None, NOW)
                .unwrap_err()
                .status(),
            404
        );
    }

    #[test]
    fn withdraw_reviewed_penalty_is_409() {
        let mut st = store();
        let e = withdraw_penalty(&mut st, id(PENALTY_1), None, NOW).unwrap_err();
        assert_eq!(e.status(), 409);
    }

    #[test]
    fn withdraw_with_other_station_is_403() {
        let mut st = store();
        let (_, p) = submit_penalty(
            &mut st,
            submit_input(Some(KNOTEN_SZ), Some(RUN_A1)),
            Uuid::new_v4(),
            NOW,
        )
        .unwrap();
        let e = withdraw_penalty(&mut st, p.id, Some(StationKind::Startlinie), NOW).unwrap_err();
        assert_eq!(e, ApiError::Forbidden);
        assert!(st.penalty(p.id).is_some());
        // Without station the soft check is skipped.
        withdraw_penalty(&mut st, p.id, None, NOW).unwrap();
    }

    #[test]
    fn withdraw_synced_pending_penalty_marks_it_rejected() {
        let mut base = sample_payload();
        base.penalties[0].status = PenaltyStatus::Pending;
        let mut st = LocalStore::new(base);
        withdraw_penalty(&mut st, id(PENALTY_1), None, NOW).unwrap();
        let p = st.penalty(id(PENALTY_1)).unwrap();
        assert_eq!(p.status, PenaltyStatus::Rejected);
        assert_eq!(p.reviewed_by.as_deref(), Some(WITHDRAWN_BY));
        assert_eq!(p.reviewed_at.as_deref(), Some(NOW));
    }

    // ── lineup ───────────────────────────────────────────────────────

    #[test]
    fn startlinie_lineup_is_recorded() {
        let mut st = store();
        let run = set_lineup(
            &mut st,
            id(RUN_B1),
            StationLineupInput {
                runner_ids: vec![id(RUNNER_B1)],
                station: Some(StationKind::Startlinie),
            },
            NOW,
        )
        .unwrap();
        assert_eq!(run.lineup, vec![id(RUNNER_B1)]);
        assert_eq!(run.lineup_confirmed_by, Some(LineupConfirmedBy::Startlinie));
        assert_eq!(run.lineup_confirmed_at.as_deref(), Some(NOW));
        assert_eq!(st.unsynced_summary().lineups, 1);
    }

    #[test]
    fn lineup_without_station_is_allowed() {
        let mut st = store();
        let input = StationLineupInput {
            runner_ids: vec![],
            station: None,
        };
        let run = set_lineup(&mut st, id(RUN_A1), input, NOW).unwrap();
        assert!(run.lineup.is_empty());
        assert_eq!(run.lineup_confirmed_at, None);
    }

    #[test]
    fn lineup_from_other_station_is_403() {
        let mut st = store();
        let input = StationLineupInput {
            runner_ids: vec![],
            station: Some(StationKind::Knotengestell),
        };
        assert_eq!(
            set_lineup(&mut st, id(RUN_A1), input, NOW).unwrap_err(),
            ApiError::Forbidden
        );
    }

    #[test]
    fn lineup_for_non_current_run_is_409() {
        let mut st = store();
        let input = StationLineupInput {
            runner_ids: vec![],
            station: None,
        };
        assert_eq!(
            set_lineup(&mut st, id(RUN_C1), input, NOW)
                .unwrap_err()
                .status(),
            409
        );
    }

    #[test]
    fn lineup_with_foreign_or_duplicate_runner_is_400() {
        let mut st = store();
        for ids in [vec![id(RUNNER_B1)], vec![id(RUNNER_A1), id(RUNNER_A1)]] {
            let input = StationLineupInput {
                runner_ids: ids,
                station: Some(StationKind::Startlinie),
            };
            assert_eq!(
                set_lineup(&mut st, id(RUN_A1), input, NOW)
                    .unwrap_err()
                    .status(),
                400
            );
        }
    }
}
