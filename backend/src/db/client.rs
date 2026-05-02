// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::sync::Arc;

use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;

#[derive(Clone)]
pub struct DbClient {
    pub inner: Arc<Client>,
    pub table_name: String,
}

impl DbClient {
    /// Build a client using the standard AWS provider chain (env vars, ECS
    /// task role, IMDS, etc.). Called once at Lambda cold start.
    pub async fn from_env(table_name: String) -> Self {
        let cfg = aws_config::defaults(BehaviorVersion::latest()).load().await;
        Self {
            inner: Arc::new(Client::new(&cfg)),
            table_name,
        }
    }
}
