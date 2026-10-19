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
//! Every write goes to `<file>.tmp` first and is then renamed over the
//! target, so a crash mid-write never leaves a truncated file. Files are
//! kept per discipline: switching to another discipline (or deselecting)
//! never deletes recorded results.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::store::LocalStore;
use crate::cloud::kido::check_schema_version;
use crate::cloud::types::{FailedPost, SyncMode};

/// Version of the on-disk format (independent of the wire schema).
pub const FORMAT_VERSION: u32 = 1;
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
    if probe.format != FORMAT_VERSION {
        return Err(format!(
            "Lokale Sicherung {} hat Format {} (erwartet {FORMAT_VERSION})",
            path.display(),
            probe.format
        ));
    }
    let data: PersistedDiscipline = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Lokale Sicherung {} beschädigt: {e}", path.display()))?;
    check_schema_version(data.store.base.schema_version)?;
    if data.store.discipline_id() != id {
        return Err(format!(
            "Lokale Sicherung {} gehört zu einer anderen Disziplin",
            path.display()
        ));
    }
    Ok(Some(data))
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
