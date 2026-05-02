// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use aws_config::BehaviorVersion;
use aws_sdk_cognitoidentityprovider::Client as CognitoIdpClient;
use aws_sdk_cognitoidentityprovider::types::{AuthFlowType, ChallengeNameType};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::config::Config;
use crate::error::{AppError, AppResult};

/// Backend-only Cognito client. The frontend never talks to Cognito directly;
/// the SPA POSTs `email` + `password` to the backend, which calls
/// `AdminInitiateAuth` and validates the returned `id_token`.
pub struct CognitoClient {
    pub user_pool_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub idp: CognitoIdpClient,
}

impl CognitoClient {
    pub async fn from_env(config: &Config) -> AppResult<Self> {
        let aws_cfg = aws_config::defaults(BehaviorVersion::latest()).load().await;
        let idp = CognitoIdpClient::new(&aws_cfg);

        let resp = idp
            .describe_user_pool_client()
            .user_pool_id(&config.cognito_user_pool_id)
            .client_id(&config.cognito_client_id)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("describe_user_pool_client: {e}")))?;

        let client_secret = resp
            .user_pool_client
            .and_then(|c| c.client_secret)
            .ok_or_else(|| {
                AppError::Internal(
                    "Cognito user pool client has no secret — confidential client expected"
                        .into(),
                )
            })?;

        Ok(Self {
            user_pool_id: config.cognito_user_pool_id.clone(),
            client_id: config.cognito_client_id.clone(),
            client_secret,
            idp,
        })
    }

    /// Initiate auth with username + password using the ADMIN_USER_PASSWORD_AUTH
    /// flow. On a NEW_PASSWORD_REQUIRED challenge (forced first-time password
    /// change), we return the Cognito session token so the frontend can send
    /// the new password back to `complete_new_password`.
    pub async fn sign_in(&self, email: &str, password: &str) -> AppResult<SignInOutcome> {
        let secret_hash = self.secret_hash(email);

        let result = self
            .idp
            .admin_initiate_auth()
            .user_pool_id(&self.user_pool_id)
            .client_id(&self.client_id)
            .auth_flow(AuthFlowType::AdminUserPasswordAuth)
            .auth_parameters("USERNAME", email)
            .auth_parameters("PASSWORD", password)
            .auth_parameters("SECRET_HASH", secret_hash)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "admin_initiate_auth failed");
                AppError::Unauthorized
            })?;

        if let Some(challenge_name) = &result.challenge_name {
            if *challenge_name == ChallengeNameType::NewPasswordRequired {
                let session = result.session.ok_or_else(|| {
                    AppError::Internal("NEW_PASSWORD_REQUIRED without session".into())
                })?;
                return Ok(SignInOutcome::NewPasswordRequired { session });
            }
            return Err(AppError::Internal(format!(
                "unhandled Cognito challenge: {challenge_name:?}"
            )));
        }

        let auth = result
            .authentication_result
            .ok_or_else(|| AppError::Internal("authentication_result missing".into()))?;

        Ok(SignInOutcome::Tokens(TokenSet::from_auth(auth)?))
    }

    pub async fn complete_new_password(
        &self,
        email: &str,
        new_password: &str,
        session: &str,
    ) -> AppResult<TokenSet> {
        let secret_hash = self.secret_hash(email);

        let result = self
            .idp
            .admin_respond_to_auth_challenge()
            .user_pool_id(&self.user_pool_id)
            .client_id(&self.client_id)
            .challenge_name(ChallengeNameType::NewPasswordRequired)
            .session(session)
            .challenge_responses("USERNAME", email)
            .challenge_responses("NEW_PASSWORD", new_password)
            .challenge_responses("SECRET_HASH", secret_hash)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "admin_respond_to_auth_challenge failed");
                AppError::Unauthorized
            })?;

        let auth = result
            .authentication_result
            .ok_or_else(|| AppError::Internal("authentication_result missing".into()))?;

        TokenSet::from_auth(auth)
    }

    fn secret_hash(&self, username: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.client_secret.as_bytes())
            .expect("HMAC-SHA256 accepts any key length");
        mac.update(username.as_bytes());
        mac.update(self.client_id.as_bytes());
        BASE64.encode(mac.finalize().into_bytes())
    }
}

pub enum SignInOutcome {
    Tokens(TokenSet),
    NewPasswordRequired { session: String },
}

pub struct TokenSet {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
}

impl TokenSet {
    fn from_auth(
        auth: aws_sdk_cognitoidentityprovider::types::AuthenticationResultType,
    ) -> AppResult<Self> {
        Ok(Self {
            id_token: auth
                .id_token
                .ok_or_else(|| AppError::Internal("id_token missing".into()))?,
            access_token: auth.access_token.unwrap_or_default(),
            refresh_token: auth
                .refresh_token
                .ok_or_else(|| AppError::Internal("refresh_token missing".into()))?,
        })
    }
}
