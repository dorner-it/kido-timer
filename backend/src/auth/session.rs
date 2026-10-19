// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub const COOKIE_NAME: &str = "kido_session";
/// Non-httpOnly companion cookie. Carries no secret — only its presence
/// matters. The static frontend reads it via a blocking inline script in
/// `<head>` to gate auth-required pages without a roundtrip, preventing
/// the flash-of-unauth content that an in-React 401 redirect produces.
pub const PRESENCE_COOKIE_NAME: &str = "kido_session_present";
const SESSION_TTL_DAYS: i64 = 30;

pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

/// Returns (verifier, challenge) for the OAuth Authorization Code + PKCE flow.
/// challenge = base64url(SHA256(verifier)).
pub fn pkce_pair() -> (String, String) {
    let verifier = random_token(32);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    (verifier, challenge)
}

pub fn session_ttl() -> i64 {
    (Utc::now() + Duration::days(SESSION_TTL_DAYS)).timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_is_base64url_sha256_of_verifier() {
        let (verifier, challenge) = pkce_pair();
        assert!(verifier.len() >= 32);
        let mut h = Sha256::new();
        h.update(verifier.as_bytes());
        let expected = URL_SAFE_NO_PAD.encode(h.finalize());
        assert_eq!(challenge, expected);
    }

    #[test]
    fn random_token_is_url_safe() {
        let t = random_token(24);
        assert!(
            t.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }
}
