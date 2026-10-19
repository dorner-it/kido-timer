//! Crash-safe JSON persistence of the local result store.
//!
//! Layout under the Tauri app data dir (`<app_data>/local-store/`):
//!
//! ```text
//! local-store/
//!   current.json                 { "discipline_id": "<uuid>" }  — restored on start
//!   disciplines/<uuid>.json      PersistedDiscipline (one per discipline)
//! ```
//!
//! Every write goes to `<file>.tmp` first (fsync) and is then renamed over
//! the target, so a crash mid-write never leaves a truncated file. Files
//! are kept per discipline: switching to another discipline (or
//! deselecting) never deletes recorded results.
//!
//! Format history:
//! - **1** (0.6.0): `store = { base, run_edits }`.
//! - **2** (0.7.0, on-prem): `store` additionally holds `run_extras`
//!   (lineups, hand times), `penalty_edits` (penalties incl. status),
//!   `current_run_ids`, `onprem_token`, `last_upload_at`; `base` may carry
//!   `penalties` (A1). Format-1 files load unchanged (all additions
//!   default to empty) and are written back as format 2 on the next save.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::store::LocalStore;
use crate::cloud::kido::check_schema_version;
use crate::cloud::types::{FailedPost, SyncMode};

/// Version of the on-disk format (independent of the wire schema).
pub const FORMAT_VERSION: u32 = 2;
/// Older formats this build still reads (migrated on load).
pub const READABLE_FORMATS: &[u32] = &[1, 2];
pub const DIR_NAME: &str = "local-store";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedDiscipline {
    pub format: u32,
    pub saved_at: String,
    /// Effective sync mode when the file was written (desktop override
    /// already applied).
    pub effective_mode: SyncMode,
    pub store: LocalStore,
    #[serde(default)]
    pub failed_posts: Vec<FailedPost>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CurrentPointer {
    discipline_id: Uuid,
}

fn disciplines_dir(root: &Path) -> PathBuf {
    root.join("disciplines")
}

pub fn discipline_file(root: &Path, id: Uuid) -> PathBuf {
    disciplines_dir(root).join(format!("{id}.json"))
}

fn current_file(root: &Path) -> PathBuf {
    root.join("current.json")
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Ordner {} nicht anlegbar: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    // Flush to disk before the rename: power cuts at the venue are a real
    // scenario, and without sync_all the renamed file may end up empty.
    {
        use std::io::Write;
        let mut f = fs::File::create(&tmp)
            .map_err(|e| format!("{} nicht schreibbar: {e}", tmp.display()))?;
        f.write_all(bytes)
            .and_then(|()| f.sync_all())
            .map_err(|e| format!("{} nicht schreibbar: {e}", tmp.display()))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("{} nicht ersetzbar: {e}", path.display()))
}

pub fn save_discipline(root: &Path, data: &PersistedDiscipline) -> Result<PathBuf, String> {
    let path = discipline_file(root, data.store.discipline_id());
    let json = serde_json::to_vec_pretty(data)
        .map_err(|e| format!("Lokale Sicherung nicht serialisierbar: {e}"))?;
    write_atomic(&path, &json)?;
    Ok(path)
}

/// Load the saved state of one discipline. `Ok(None)` if nothing saved.
/// A file from an incompatible format or schema is an error (never
/// silently dropped — it may hold results).
pub fn load_discipline(root: &Path, id: Uuid) -> Result<Option<PersistedDiscipline>, String> {
    let path = discipline_file(root, id);
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{} nicht lesbar: {e}", path.display())),
    };
    #[derive(Deserialize)]
    struct Probe {
        format: u32,
    }
    let probe: Probe = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Lokale Sicherung {} beschädigt: {e}", path.display()))?;
    if !READABLE_FORMATS.contains(&probe.format) {
        return Err(format!(
            "Lokale Sicherung {} hat Format {} (erwartet {FORMAT_VERSION})",
            path.display(),
            probe.format
        ));
    }
    let data: PersistedDiscipline = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Lokale Sicherung {} beschädigt: {e}", path.display()))?;
    let data = migrate(data);
    check_schema_version(data.store.base.schema_version)?;
    if data.store.discipline_id() != id {
        return Err(format!(
            "Lokale Sicherung {} gehört zu einer anderen Disziplin",
            path.display()
        ));
    }
    Ok(Some(data))
}

/// Bring a loaded file to [`FORMAT_VERSION`]. Format 1 → 2 needs no data
/// change: every format-2 addition is `#[serde(default)]`, and a base
/// without `penalties` falls back to `approved_penalties` on parse.
pub fn migrate(mut data: PersistedDiscipline) -> PersistedDiscipline {
    if data.format < FORMAT_VERSION {
        data.format = FORMAT_VERSION;
    }
    data
}

/// Move an unreadable discipline file aside (`<uuid>.broken-<unix>.json`)
/// so it is never overwritten by the next save. Returns the new path.
pub fn quarantine_discipline(root: &Path, id: Uuid) -> Result<PathBuf, String> {
    let path = discipline_file(root, id);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let target = disciplines_dir(root).join(format!("{id}.broken-{stamp}.json"));
    fs::rename(&path, &target)
        .map_err(|e| format!("{} nicht verschiebbar: {e}", path.display()))?;
    Ok(target)
}

/// Remember which discipline to restore on the next start (`None` =
/// nothing selected).
pub fn set_current(root: &Path, id: Option<Uuid>) -> Result<(), String> {
    let path = current_file(root);
    match id {
        Some(discipline_id) => {
            let json = serde_json::to_vec(&CurrentPointer { discipline_id })
                .map_err(|e| format!("serialize: {e}"))?;
            write_atomic(&path, &json)
        }
        None => match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{} nicht löschbar: {e}", path.display())),
        },
    }
}

pub fn load_current(root: &Path) -> Option<Uuid> {
    let bytes = fs::read(current_file(root)).ok()?;
    serde_json::from_slice::<CurrentPointer>(&bytes)
        .ok()
        .map(|p| p.discipline_id)
}

/// State to restore on app start: the discipline that was selected when
/// the app last ran.
pub fn restore_current(root: &Path) -> Result<Option<PersistedDiscipline>, String> {
    match load_current(root) {
        Some(id) => load_discipline(root, id),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{id, sample_payload, DISCIPLINE, RUN_A1};
    use crate::cloud::types::RunStatus;
    use crate::local::store::RunEdit;

    fn persisted() -> PersistedDiscipline {
        let mut store = LocalStore::new(sample_payload());
        store
            .record(
                id(RUN_A1),
                RunEdit {
                    run_number: 1,
                    status: RunStatus::Completed,
                    original_time_ms: Some(47_240),
                    started_at: Some("2027-06-12T10:00:00Z".into()),
                    ended_at: Some("2027-06-12T10:00:47Z".into()),
                    recorded_at: "2027-06-12T10:00:48Z".into(),
                },
            )
            .unwrap();
        PersistedDiscipline {
            format: FORMAT_VERSION,
            saved_at: "2027-06-12T10:00:48Z".into(),
            effective_mode: SyncMode::Offline,
            store,
            failed_posts: vec![FailedPost {
                run_id: id(RUN_A1),
                discipline_id: id(DISCIPLINE),
                run_number: 1,
                status: RunStatus::Completed,
                original_time_ms: Some(47_240),
                started_at: None,
                ended_at: None,
                error: "request failed".into(),
                failed_at: "2027-06-12T10:00:49Z".into(),
            }],
        }
    }

    #[test]
    fn save_and_restore_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let data = persisted();
        let path = save_discipline(dir.path(), &data).unwrap();
        assert!(path.ends_with(format!("disciplines/{DISCIPLINE}.json")));
        set_current(dir.path(), Some(id(DISCIPLINE))).unwrap();

        let restored = restore_current(dir.path()).unwrap().expect("restored");
        assert_eq!(restored, data);
        // The recorded result survives the restart.
        let view = restored.store.view();
        assert_eq!(view.runs[0].original_time_ms, Some(47_240));
        assert_eq!(view.runs[0].status, RunStatus::Completed);
    }

    #[test]
    fn no_tmp_file_left_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = save_discipline(dir.path(), &persisted()).unwrap();
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn overwrite_keeps_latest() {
        let dir = tempfile::tempdir().unwrap();
        let mut data = persisted();
        save_discipline(dir.path(), &data).unwrap();
        data.effective_mode = SyncMode::Live;
        data.failed_posts.clear();
        save_discipline(dir.path(), &data).unwrap();
        let loaded = load_discipline(dir.path(), id(DISCIPLINE))
            .unwrap()
            .unwrap();
        assert_eq!(loaded.effective_mode, SyncMode::Live);
        assert!(loaded.failed_posts.is_empty());
    }

    #[test]
    fn missing_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_discipline(dir.path(), Uuid::new_v4())
            .unwrap()
            .is_none());
        assert!(restore_current(dir.path()).unwrap().is_none());
    }

    #[test]
    fn clearing_current_keeps_discipline_file() {
        let dir = tempfile::tempdir().unwrap();
        save_discipline(dir.path(), &persisted()).unwrap();
        set_current(dir.path(), Some(id(DISCIPLINE))).unwrap();
        set_current(dir.path(), None).unwrap();
        set_current(dir.path(), None).unwrap(); // idempotent
        assert!(restore_current(dir.path()).unwrap().is_none());
        assert!(load_discipline(dir.path(), id(DISCIPLINE))
            .unwrap()
            .is_some());
    }

    #[test]
    fn corrupt_file_is_an_error_not_silently_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = discipline_file(dir.path(), id(DISCIPLINE));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{ not json").unwrap();
        let err = load_discipline(dir.path(), id(DISCIPLINE)).unwrap_err();
        assert!(err.contains("beschädigt"), "{err}");
    }

    #[test]
    fn quarantine_moves_file_aside() {
        let dir = tempfile::tempdir().unwrap();
        let src = save_discipline(dir.path(), &persisted()).unwrap();
        let moved = quarantine_discipline(dir.path(), id(DISCIPLINE)).unwrap();
        assert!(!src.exists());
        assert!(moved.exists());
        assert!(moved
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains(".broken-"));
    }

    #[test]
    fn unknown_format_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut value = serde_json::to_value(persisted()).unwrap();
        value["format"] = 99.into();
        let path = discipline_file(dir.path(), id(DISCIPLINE));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let err = load_discipline(dir.path(), id(DISCIPLINE)).unwrap_err();
        assert!(err.contains("Format 99"), "{err}");
    }

    /// A format-1 file exactly as 0.6.0 wrote it (no penalties in the
    /// base, no on-prem fields).
    fn format1_json() -> serde_json::Value {
        let mut base = serde_json::to_value(sample_payload()).unwrap();
        base.as_object_mut().unwrap().remove("penalties");
        serde_json::json!({
            "format": 1,
            "saved_at": "2027-06-12T10:00:48Z",
            "effective_mode": "offline",
            "store": {
                "base": base,
                "run_edits": {
                    RUN_A1: {
                        "run_number": 1, "status": "completed",
                        "original_time_ms": 47240,
                        "started_at": "2027-06-12T10:00:00Z",
                        "ended_at": "2027-06-12T10:00:47Z",
                        "recorded_at": "2027-06-12T10:00:48Z"
                    }
                }
            },
            "failed_posts": []
        })
    }

    #[test]
    fn format1_file_is_migrated_to_format2() {
        let dir = tempfile::tempdir().unwrap();
        let path = discipline_file(dir.path(), id(DISCIPLINE));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, serde_json::to_vec(&format1_json()).unwrap()).unwrap();

        let loaded = load_discipline(dir.path(), id(DISCIPLINE))
            .unwrap()
            .expect("format 1 readable");
        assert_eq!(loaded.format, FORMAT_VERSION);
        assert_eq!(loaded.effective_mode, SyncMode::Offline);
        // Recorded result kept.
        assert_eq!(loaded.store.view().runs[0].original_time_ms, Some(47_240));
        // New overlays default to empty; penalties fall back to approved.
        assert!(loaded.store.run_extras.is_empty());
        assert!(loaded.store.penalty_edits.is_empty());
        assert!(loaded.store.current_run_ids.is_none());
        assert!(loaded.store.onprem_token.is_none());
        assert_eq!(loaded.store.base.penalties.len(), 1);
        assert_eq!(
            loaded.store.base.penalties,
            loaded.store.base.approved_penalties
        );

        // Next save writes format 2; reload is lossless.
        save_discipline(dir.path(), &loaded).unwrap();
        let raw: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["format"], 2);
        assert!(raw["store"]["base"]["penalties"].is_array());
        let again = load_discipline(dir.path(), id(DISCIPLINE))
            .unwrap()
            .unwrap();
        assert_eq!(again, loaded);
    }

    #[test]
    fn migrate_is_idempotent_and_bumps_format() {
        let mut data = persisted();
        data.format = 1;
        let migrated = migrate(data.clone());
        assert_eq!(migrated.format, FORMAT_VERSION);
        assert_eq!(migrate(migrated.clone()), migrated);
        assert_eq!(migrated.store, data.store);
    }

    #[test]
    fn on_prem_overlays_round_trip_through_disk() {
        use crate::cloud::fixtures::{RUNNER_A1, RUN_B1};
        use crate::cloud::types::{LineupConfirmedBy, PenaltyStatus};
        use crate::local::store::{HandTimesEdit, LineupEdit};

        let dir = tempfile::tempdir().unwrap();
        let mut data = persisted();
        let mut p = data.store.base.penalties[0].clone();
        p.id = Uuid::new_v4();
        p.status = PenaltyStatus::Pending;
        p.reviewed_at = None;
        p.reviewed_by = None;
        data.store.upsert_penalty(p.clone());
        let mut rejected = data.store.base.penalties[0].clone();
        rejected.status = PenaltyStatus::Rejected;
        data.store.upsert_penalty(rejected);
        data.store
            .set_lineup(
                id(RUN_B1),
                LineupEdit {
                    runner_ids: vec![],
                    confirmed_at: Some("2027-06-12T10:00:00.000Z".into()),
                    confirmed_by: Some(LineupConfirmedBy::Organizer),
                },
            )
            .unwrap();
        data.store
            .set_lineup(
                id(RUN_A1),
                LineupEdit {
                    runner_ids: vec![id(RUNNER_A1)],
                    confirmed_at: Some("2027-06-12T10:00:00.000Z".into()),
                    confirmed_by: Some(LineupConfirmedBy::Startlinie),
                },
            )
            .unwrap();
        data.store
            .set_hand_times(
                id(RUN_A1),
                HandTimesEdit::Set {
                    hand_time_1_ms: 47_230,
                    hand_time_2_ms: 47_245,
                },
            )
            .unwrap();
        data.store.set_current_runs(vec![id(RUN_B1)]);
        data.store.onprem_token = Some("0123456789abcdef0123456789abcdef".into());
        save_discipline(dir.path(), &data).unwrap();
        set_current(dir.path(), Some(id(DISCIPLINE))).unwrap();

        let restored = restore_current(dir.path()).unwrap().unwrap();
        assert_eq!(restored, data);
        let view = restored.store.view();
        assert_eq!(view.penalties.len(), 2);
        assert_eq!(view.penalties[0].status, PenaltyStatus::Rejected);
        assert_eq!(view.runs[0].original_time_ms, Some(47_240));
        assert_eq!(view.runs[0].lineup, vec![id(RUNNER_A1)]);
        assert_eq!(view.event.current_run_ids, vec![id(RUN_B1)]);
        assert_eq!(
            restored.store.onprem_token.as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
    }

    #[test]
    fn old_schema_snapshot_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut data = persisted();
        data.store.base.schema_version = 2;
        save_discipline(dir.path(), &data).unwrap();
        let err = load_discipline(dir.path(), id(DISCIPLINE)).unwrap_err();
        assert!(err.contains("v2"), "{err}");
    }

    #[test]
    fn file_of_other_discipline_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let data = persisted();
        let other = Uuid::new_v4();
        let src = save_discipline(dir.path(), &data).unwrap();
        std::fs::copy(&src, discipline_file(dir.path(), other)).unwrap();
        assert!(load_discipline(dir.path(), other).is_err());
    }
}
