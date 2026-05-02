// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::sync::Arc;

use crate::auth::{cognito::CognitoClient, jwks::JwksCache};
use crate::config::Config;
use crate::db::client::DbClient;
use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: DbClient,
    pub cognito: Arc<CognitoClient>,
    pub jwks: Arc<JwksCache>,
}

impl AppState {
    /// Build the full state at Lambda cold start. Bails with a typed error
    /// if Cognito's user-pool client lookup fails (e.g. wrong env vars).
    pub async fn from_env() -> AppResult<Self> {
        let config = Arc::new(Config::from_env());

        if config.cognito_user_pool_id.is_empty() || config.cognito_client_id.is_empty() {
            return Err(AppError::Internal(
                "COGNITO_USER_POOL_ID and COGNITO_CLIENT_ID must be set".into(),
            ));
        }

        let db = DbClient::from_env(config.table_name.clone()).await;
        let cognito = Arc::new(CognitoClient::from_env(&config).await?);
        let jwks = Arc::new(JwksCache::new(config.cognito_issuer_url()));

        Ok(Self {
            config,
            db,
            cognito,
            jwks,
        })
    }
}
