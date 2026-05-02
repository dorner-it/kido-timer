// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use aws_config::BehaviorVersion;
use aws_sdk_cognitoidentityprovider::Client as CognitoIdpClient;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde::Deserialize;

use crate::config::Config;
use crate::error::{AppError, AppResult};

/// Wraps both the AWS SDK client (admin operations like `DescribeUserPoolClient`)
/// and the OAuth2 token endpoint of the Cognito hosted UI domain.
pub struct CognitoClient {
    pub user_pool_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub hosted_ui_domain: String,
    pub redirect_uri: String,
    pub idp: CognitoIdpClient,
    pub http: reqwest::Client,
}

impl CognitoClient {
    /// Builds a client and fetches the user-pool client secret. Called once
    /// at Lambda cold start.
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
                    "Cognito user pool client has no secret — check the client is configured as confidential".into(),
                )
            })?;

        Ok(Self {
            user_pool_id: config.cognito_user_pool_id.clone(),
            client_id: config.cognito_client_id.clone(),
            client_secret,
            hosted_ui_domain: config.cognito_domain.clone(),
            redirect_uri: config.cognito_redirect_uri.clone(),
            idp,
            http: reqwest::Client::new(),
        })
    }

    /// Authorization Code → tokens. The PKCE `code_verifier` was stashed in
    /// DDB during /api/auth/login.
    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> AppResult<TokenResponse> {
        let url = format!("https://{}/oauth2/token", self.hosted_ui_domain);
        let basic = BASE64.encode(format!("{}:{}", self.client_id, self.client_secret));

        let params = [
            ("grant_type", "authorization_code"),
            ("client_id", &self.client_id),
            ("code", code),
            ("redirect_uri", &self.redirect_uri),
            ("code_verifier", code_verifier),
        ];

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Basic {basic}"))
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("token endpoint: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!(%status, %body, "Cognito token exchange failed");
            return Err(AppError::Unauthorized);
        }

        resp.json::<TokenResponse>()
            .await
            .map_err(|e| AppError::Internal(format!("token parse: {e}")))
    }

    /// Best-effort revoke during logout. Failures are logged but not raised
    /// — the cookie clear and session-row delete still happen.
    pub async fn revoke_refresh_token(&self, refresh_token: &str) -> AppResult<()> {
        let url = format!("https://{}/oauth2/revoke", self.hosted_ui_domain);
        let basic = BASE64.encode(format!("{}:{}", self.client_id, self.client_secret));

        let params = [
            ("token", refresh_token),
            ("client_id", &self.client_id),
        ];

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Basic {basic}"))
            .form(&params)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("revoke: {e}")))?;

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "Cognito revoke returned non-success");
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    #[serde(default)]
    pub expires_in: i64,
    #[serde(default)]
    pub token_type: String,
}
