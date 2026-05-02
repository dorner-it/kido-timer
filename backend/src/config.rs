// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub env: String,
    pub aws_region: String,
    pub table_name: String,
    pub cognito_user_pool_id: String,
    pub cognito_client_id: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            env: env::var("ENV").unwrap_or_else(|_| "dev".to_string()),
            aws_region: env::var("AWS_REGION").unwrap_or_else(|_| "eu-central-1".to_string()),
            table_name: env::var("TABLE_NAME").unwrap_or_else(|_| "ki-do-timer-dev".to_string()),
            cognito_user_pool_id: env::var("COGNITO_USER_POOL_ID").unwrap_or_default(),
            cognito_client_id: env::var("COGNITO_CLIENT_ID").unwrap_or_default(),
        }
    }

    pub fn cognito_issuer_url(&self) -> String {
        format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            self.aws_region, self.cognito_user_pool_id
        )
    }
}
