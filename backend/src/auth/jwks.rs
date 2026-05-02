// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use jsonwebtoken::{DecodingKey, jwk::JwkSet};
use tokio::sync::RwLock;

use crate::error::{AppError, AppResult};

/// JWKS for the configured Cognito user pool. Keys are fetched lazily and
/// cached in memory; a cache miss for an unknown `kid` triggers a refetch
/// (handles Cognito key rotation transparently).
pub struct JwksCache {
    issuer_url: String,
    http: reqwest::Client,
    keys: RwLock<HashMap<String, DecodingKey>>,
}

impl JwksCache {
    pub fn new(issuer_url: String) -> Self {
        Self {
            issuer_url,
            http: reqwest::Client::new(),
            keys: RwLock::new(HashMap::new()),
        }
    }

    pub async fn key_for(&self, kid: &str) -> AppResult<DecodingKey> {
        if let Some(key) = self.keys.read().await.get(kid) {
            return Ok(key.clone());
        }

        self.refresh().await?;

        self.keys
            .read()
            .await
            .get(kid)
            .cloned()
            .ok_or(AppError::Unauthorized)
    }

    async fn refresh(&self) -> AppResult<()> {
        let url = format!("{}/.well-known/jwks.json", self.issuer_url);

        let jwks: JwkSet = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("jwks fetch: {e}")))?
            .error_for_status()
            .map_err(|e| AppError::Internal(format!("jwks status: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("jwks parse: {e}")))?;

        let mut next: HashMap<String, DecodingKey> = HashMap::with_capacity(jwks.keys.len());
        for jwk in &jwks.keys {
            if let Some(kid) = &jwk.common.key_id {
                let key = DecodingKey::from_jwk(jwk)
                    .map_err(|e| AppError::Internal(format!("decoding key for kid {kid}: {e}")))?;
                next.insert(kid.clone(), key);
            }
        }

        let mut guard = self.keys.write().await;
        *guard = next;
        Ok(())
    }
}
