use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::types::{DisciplinePayload, KidoEnvelope};

const ENVELOPE_VERSION: u32 = 2;
const ALG: &str = "HS256";
const SCHEMA_VERSION: u32 = 2;

type HmacSha256 = Hmac<Sha256>;

fn decode_hmac_key(b64url: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD
        .decode(b64url.as_bytes())
        .map_err(|e| format!("hmac key decode failed: {e}"))
}

fn sign(key: &[u8], payload_b64: &str) -> Result<String, String> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| format!("hmac key invalid length: {e}"))?;
    mac.update(payload_b64.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn verify_sig(key: &[u8], payload_b64: &str, sig_b64url: &str) -> Result<(), String> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| format!("hmac key invalid length: {e}"))?;
    mac.update(payload_b64.as_bytes());
    let expected = URL_SAFE_NO_PAD
        .decode(sig_b64url.as_bytes())
        .map_err(|e| format!("signature decode failed: {e}"))?;
    mac.verify_slice(&expected)
        .map_err(|_| "signature mismatch".to_string())
}

fn decode_payload(payload_b64: &str) -> Result<DisciplinePayload, String> {
    let bytes = STANDARD
        .decode(payload_b64.as_bytes())
        .map_err(|e| format!("payload base64 decode failed: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("payload parse failed: {e}"))
}

fn now_rfc3339() -> Result<String, String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|e| format!("timestamp format failed: {e}"))
}

pub fn build_envelope(
    payload: &DisciplinePayload,
    hmac_key_b64: &str,
) -> Result<KidoEnvelope, String> {
    if payload.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema version {} (expected {SCHEMA_VERSION})",
            payload.schema_version
        ));
    }
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
    if envelope.v != ENVELOPE_VERSION {
        return Err(format!(
            "unsupported envelope version {} (expected {ENVELOPE_VERSION})",
            envelope.v
        ));
    }
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
    if payload.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported payload schema_version {} (expected {SCHEMA_VERSION})",
            payload.schema_version
        ));
    }
    if payload.owner_sub != expected_owner_sub {
        return Err(format!(
            "payload owner_sub {:?} does not match this account",
            payload.owner_sub
        ));
    }
    Ok(payload)
}

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
    use crate::cloud::types::{Discipline, DisciplinePayload, ExportedEvent, SyncMode, TimerMode};

    fn fake_payload() -> DisciplinePayload {
        let event_id = uuid::Uuid::nil();
        let discipline_id = uuid::Uuid::nil();
        DisciplinePayload {
            schema_version: SCHEMA_VERSION,
            exported_at: "2026-05-21T10:13:00Z".into(),
            owner_sub: "abc".into(),
            event: ExportedEvent {
                id: event_id,
                owner_sub: "abc".into(),
                name: "Test".into(),
                date: "2026-06-12".into(),
                location: None,
                is_active: true,
                current_discipline_id: Some(discipline_id),
                current_run_id: None,
                created_at: None,
                updated_at: None,
            },
            discipline: Discipline {
                id: discipline_id,
                event_id,
                name: "Löschangriff".into(),
                mode: TimerMode::SingleLane,
                sync_mode: SyncMode::Live,
                position: 1,
                created_at: None,
                updated_at: None,
            },
            teams: vec![],
            runners: vec![],
            team_entries: vec![],
            runs: vec![],
            approved_penalties: vec![],
        }
    }

    #[test]
    fn round_trip_envelope() {
        let key = URL_SAFE_NO_PAD.encode([7u8; 32]);
        let payload = fake_payload();
        let envelope = build_envelope(&payload, &key).expect("build");
        let decoded = verify_envelope(&envelope, &key, "abc").expect("verify");
        assert_eq!(decoded.discipline.name, "Löschangriff");
    }

    #[test]
    fn rejects_wrong_owner() {
        let key = URL_SAFE_NO_PAD.encode([7u8; 32]);
        let payload = fake_payload();
        let envelope = build_envelope(&payload, &key).expect("build");
        let err = verify_envelope(&envelope, &key, "other").unwrap_err();
        assert!(err.contains("owner_sub"));
    }

    #[test]
    fn rejects_tampered_payload() {
        let key = URL_SAFE_NO_PAD.encode([7u8; 32]);
        let payload = fake_payload();
        let mut envelope = build_envelope(&payload, &key).expect("build");
        let mut bytes = STANDARD.decode(&envelope.payload).unwrap();
        bytes[0] ^= 0x01;
        envelope.payload = STANDARD.encode(bytes);
        let err = verify_envelope(&envelope, &key, "abc").unwrap_err();
        assert!(err.contains("signature"));
    }
}
