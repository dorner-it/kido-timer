//! `.kido` envelope (schema v4): HMAC-SHA256 signing / verification and
//! the version gate shared by file import, the live snapshot and polling.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::types::{DisciplinePayload, KidoEnvelope};

/// Envelope version (`v`), equals the payload schema version.
pub const ENVELOPE_VERSION: u32 = 4;
const ALG: &str = "HS256";
/// Payload `schema_version` this desktop build reads and writes.
pub const SCHEMA_VERSION: u32 = 4;

type HmacSha256 = Hmac<Sha256>;

/// Accept only [`SCHEMA_VERSION`]. The message tells the operator which
/// side is outdated.
pub fn check_schema_version(found: u32) -> Result<(), String> {
    if found == SCHEMA_VERSION {
        return Ok(());
    }
    if found < SCHEMA_VERSION {
        return Err(format!(
            "Veraltetes Datenformat v{found} — diese Desktop-Version erwartet v{SCHEMA_VERSION} (Gruppenstafette 2027). Bitte Web-App/Desktop aktualisieren und die Daten neu laden."
        ));
    }
    Err(format!(
        "Unbekanntes Datenformat v{found} — diese Desktop-Version erwartet v{SCHEMA_VERSION}. Bitte Web-App/Desktop aktualisieren."
    ))
}

/// Parse a payload (file payload or live snapshot body). The version is
/// probed **before** the full parse so an old payload (e.g. v2 with
/// legacy `TimerMode` values) yields the clear version error instead of a
/// cryptic parse error.
pub fn parse_payload(bytes: &[u8]) -> Result<DisciplinePayload, String> {
    #[derive(Deserialize)]
    struct VersionProbe {
        schema_version: u32,
    }
    let probe: VersionProbe = serde_json::from_slice(bytes)
        .map_err(|e| format!("Daten nicht lesbar (schema_version fehlt): {e}"))?;
    check_schema_version(probe.schema_version)?;
    serde_json::from_slice(bytes).map_err(|e| format!("Daten nicht lesbar: {e}"))
}

fn decode_hmac_key(b64url: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD
        .decode(b64url.as_bytes())
        .map_err(|e| format!("hmac key decode failed: {e}"))
}

fn sign(key: &[u8], payload_b64: &str) -> Result<String, String> {
    let mut mac =
        HmacSha256::new_from_slice(key).map_err(|e| format!("hmac key invalid length: {e}"))?;
    mac.update(payload_b64.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn verify_sig(key: &[u8], payload_b64: &str, sig_b64url: &str) -> Result<(), String> {
    let mut mac =
        HmacSha256::new_from_slice(key).map_err(|e| format!("hmac key invalid length: {e}"))?;
    mac.update(payload_b64.as_bytes());
    let expected = URL_SAFE_NO_PAD
        .decode(sig_b64url.as_bytes())
        .map_err(|e| format!("signature decode failed: {e}"))?;
    mac.verify_slice(&expected).map_err(|_| {
        "signature mismatch — Datei stammt nicht von diesem Konto oder wurde verändert".to_string()
    })
}

fn decode_payload(payload_b64: &str) -> Result<DisciplinePayload, String> {
    let bytes = STANDARD
        .decode(payload_b64.as_bytes())
        .map_err(|e| format!("payload base64 decode failed: {e}"))?;
    parse_payload(&bytes)
}

pub fn now_rfc3339() -> Result<String, String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|e| format!("timestamp format failed: {e}"))
}

pub fn build_envelope(
    payload: &DisciplinePayload,
    hmac_key_b64: &str,
) -> Result<KidoEnvelope, String> {
    check_schema_version(payload.schema_version)?;
    let key = decode_hmac_key(hmac_key_b64)?;
    let payload_json =
        serde_json::to_vec(payload).map_err(|e| format!("payload serialize failed: {e}"))?;
    let payload_b64 = STANDARD.encode(payload_json);
    let sig = sign(&key, &payload_b64)?;
    Ok(KidoEnvelope {
        v: ENVELOPE_VERSION,
        alg: ALG.to_string(),
        owner_sub: payload.owner_sub.clone(),
        issued_at: now_rfc3339()?,
        payload: payload_b64,
        sig,
    })
}

pub fn verify_envelope(
    envelope: &KidoEnvelope,
    hmac_key_b64: &str,
    expected_owner_sub: &str,
) -> Result<DisciplinePayload, String> {
    check_schema_version(envelope.v)?;
    if envelope.alg != ALG {
        return Err(format!(
            "unsupported envelope alg {:?} (expected {ALG})",
            envelope.alg
        ));
    }
    if envelope.owner_sub != expected_owner_sub {
        return Err(format!(
            "envelope owner_sub {:?} does not match this account",
            envelope.owner_sub
        ));
    }
    let key = decode_hmac_key(hmac_key_b64)?;
    verify_sig(&key, &envelope.payload, &envelope.sig)?;
    let payload = decode_payload(&envelope.payload)?;
    if payload.owner_sub != expected_owner_sub {
        return Err(format!(
            "payload owner_sub {:?} does not match this account",
            payload.owner_sub
        ));
    }
    Ok(payload)
}

/// Prepare a payload for `.kido` export: current schema, own clock, own
/// `owner_sub`. IDs are preserved.
pub fn restamp_for_export(
    payload: &DisciplinePayload,
    owner_sub: &str,
) -> Result<DisciplinePayload, String> {
    let mut next = payload.clone();
    next.schema_version = SCHEMA_VERSION;
    next.exported_at = now_rfc3339()?;
    next.owner_sub = owner_sub.to_string();
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::fixtures::{sample_payload, sample_payload_json};
    use crate::cloud::types::RunStatus;

    fn key() -> String {
        URL_SAFE_NO_PAD.encode([7u8; 32])
    }

    #[test]
    fn round_trip_envelope() {
        let payload = sample_payload();
        let envelope = build_envelope(&payload, &key()).expect("build");
        assert_eq!(envelope.v, 4);
        assert_eq!(envelope.alg, "HS256");
        let decoded = verify_envelope(&envelope, &key(), "sub-1").expect("verify");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn round_trip_preserves_recorded_results() {
        let mut payload = sample_payload();
        let run = &mut payload.runs[0];
        run.status = RunStatus::Completed;
        run.original_time_ms = Some(47_240);
        run.ended_at = Some("2027-06-12T10:01:00Z".into());
        let restamped = restamp_for_export(&payload, "sub-1").unwrap();
        let envelope = build_envelope(&restamped, &key()).unwrap();
        let decoded = verify_envelope(&envelope, &key(), "sub-1").unwrap();
        assert_eq!(decoded.runs[0].status, RunStatus::Completed);
        assert_eq!(decoded.runs[0].original_time_ms, Some(47_240));
        assert_eq!(decoded.schema_version, 4);
        assert_eq!(decoded.runs, payload.runs);
    }

    #[test]
    fn rejects_wrong_owner() {
        let envelope = build_envelope(&sample_payload(), &key()).expect("build");
        let err = verify_envelope(&envelope, &key(), "other").unwrap_err();
        assert!(err.contains("owner_sub"));
    }

    #[test]
    fn rejects_tampered_payload() {
        let mut envelope = build_envelope(&sample_payload(), &key()).expect("build");
        let mut bytes = STANDARD.decode(&envelope.payload).unwrap();
        bytes[0] ^= 0x01;
        envelope.payload = STANDARD.encode(bytes);
        let err = verify_envelope(&envelope, &key(), "sub-1").unwrap_err();
        assert!(err.contains("signature"));
    }

    #[test]
    fn rejects_old_envelope_version() {
        for v in [1, 2, 3] {
            let mut envelope = build_envelope(&sample_payload(), &key()).unwrap();
            envelope.v = v;
            let err = verify_envelope(&envelope, &key(), "sub-1").unwrap_err();
            assert!(err.contains(&format!("v{v}")), "{err}");
            assert!(err.contains("Bitte Web-App/Desktop aktualisieren"), "{err}");
        }
    }

    #[test]
    fn rejects_newer_envelope_version() {
        let mut envelope = build_envelope(&sample_payload(), &key()).unwrap();
        envelope.v = 5;
        let err = verify_envelope(&envelope, &key(), "sub-1").unwrap_err();
        assert!(err.contains("Unbekanntes Datenformat v5"), "{err}");
    }

    #[test]
    fn rejects_old_payload_version_inside_valid_envelope() {
        // A correctly signed v4 envelope carrying a v3 payload.
        let mut value: serde_json::Value = serde_json::from_str(&sample_payload_json()).unwrap();
        value["schema_version"] = 3.into();
        let payload_b64 = STANDARD.encode(serde_json::to_vec(&value).unwrap());
        let key_bytes = decode_hmac_key(&key()).unwrap();
        let envelope = KidoEnvelope {
            v: 4,
            alg: "HS256".into(),
            owner_sub: "sub-1".into(),
            issued_at: "2027-06-12T10:00:00Z".into(),
            sig: sign(&key_bytes, &payload_b64).unwrap(),
            payload: payload_b64,
        };
        let err = verify_envelope(&envelope, &key(), "sub-1").unwrap_err();
        assert!(err.contains("v3"), "{err}");
        assert!(err.contains("Bitte Web-App/Desktop aktualisieren"), "{err}");
    }

    #[test]
    fn parse_payload_probes_version_before_full_parse() {
        // v2 payload with a legacy TimerMode: must fail with the version
        // message, not with "unknown variant single_lane".
        let mut value: serde_json::Value = serde_json::from_str(&sample_payload_json()).unwrap();
        value["schema_version"] = 2.into();
        value["discipline"]["mode"] = "single_lane".into();
        let err = parse_payload(&serde_json::to_vec(&value).unwrap()).unwrap_err();
        assert!(err.contains("Veraltetes Datenformat v2"), "{err}");
    }

    #[test]
    fn parse_payload_rejects_missing_version() {
        let err = parse_payload(br#"{"owner_sub":"x"}"#).unwrap_err();
        assert!(err.contains("schema_version"), "{err}");
    }

    #[test]
    fn parse_payload_accepts_v4() {
        let p = parse_payload(sample_payload_json().as_bytes()).unwrap();
        assert_eq!(p.schema_version, 4);
    }

    #[test]
    fn build_envelope_refuses_non_v4_payload() {
        let mut payload = sample_payload();
        payload.schema_version = 2;
        assert!(build_envelope(&payload, &key()).is_err());
    }

    #[test]
    fn restamp_sets_owner_and_schema() {
        let mut payload = sample_payload();
        payload.schema_version = 4;
        let next = restamp_for_export(&payload, "sub-2").unwrap();
        assert_eq!(next.owner_sub, "sub-2");
        assert_eq!(next.schema_version, SCHEMA_VERSION);
        assert_eq!(next.discipline.id, payload.discipline.id);
    }

    #[test]
    fn check_schema_version_only_accepts_4() {
        assert!(check_schema_version(4).is_ok());
        assert!(check_schema_version(0).is_err());
        assert!(check_schema_version(3).is_err());
        assert!(check_schema_version(5).is_err());
    }

    #[test]
    fn export_contains_all_penalties_and_local_run_fields() {
        use crate::cloud::fixtures::{id, RUNNER_A1, RUN_A1, RUN_B1};
        use crate::cloud::types::{PenaltyStatus, StationKind, TimeSource};
        use crate::local::judge::{
            set_lineup, submit_penalty, StationLineupInput, SubmitStationPenaltyInput,
        };
        use crate::local::organizer;
        use crate::local::store::LocalStore;

        let mut store = LocalStore::new(sample_payload());
        let now = "2027-06-12T10:01:00.000Z";
        let (_, pending) = submit_penalty(
            &mut store,
            SubmitStationPenaltyInput {
                station: StationKind::Knotengestell,
                run_id: Some(id(RUN_B1)),
                code: Some("knotengestell.ein_knoten_falsch".into()),
                client_id: None,
            },
            uuid::Uuid::new_v4(),
            now,
        )
        .unwrap();
        let (_, rejected) = submit_penalty(
            &mut store,
            SubmitStationPenaltyInput {
                station: StationKind::Knotengestell,
                run_id: Some(id(RUN_B1)),
                code: Some("knotengestell.mehrere_knoten_falsch".into()),
                client_id: None,
            },
            uuid::Uuid::new_v4(),
            now,
        )
        .unwrap();
        organizer::review_penalty(&mut store, rejected.id, false, "fbl", now).unwrap();
        organizer::set_hand_times(&mut store, id(RUN_A1), 47_230, 47_245).unwrap();
        set_lineup(
            &mut store,
            id(RUN_A1),
            StationLineupInput {
                runner_ids: vec![id(RUNNER_A1)],
                station: None,
            },
            now,
        )
        .unwrap();

        let restamped = restamp_for_export(&store.view(), "sub-1").unwrap();
        let envelope = build_envelope(&restamped, &key()).unwrap();
        let decoded = verify_envelope(&envelope, &key(), "sub-1").unwrap();

        let mut statuses: Vec<&str> = decoded
            .penalties
            .iter()
            .map(|p| p.status.as_str())
            .collect();
        statuses.sort_unstable();
        assert_eq!(statuses, vec!["approved", "pending", "rejected"]);
        assert_eq!(
            decoded.penalties[0].status,
            PenaltyStatus::Approved,
            "base row first"
        );
        assert!(decoded.penalties.iter().any(|p| p.id == pending.id));
        assert_eq!(
            decoded.approved_penalties.len(),
            1,
            "legacy list = approved only"
        );
        let run = decoded.runs.iter().find(|r| r.id == id(RUN_A1)).unwrap();
        assert_eq!(run.time_source, TimeSource::Handstopp);
        assert_eq!(run.hand_time_1_ms, Some(47_230));
        assert_eq!(run.original_time_ms, Some(47_240));
        assert_eq!(run.lineup, vec![id(RUNNER_A1)]);
        // Raw JSON carries the A1 field.
        let raw = base64::engine::general_purpose::STANDARD
            .decode(envelope.payload.as_bytes())
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(v["penalties"].as_array().unwrap().len(), 3);
    }
}
