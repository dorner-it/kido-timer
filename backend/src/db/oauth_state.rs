// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};

/// Short-lived per-login record used to defend the Cognito callback against
/// CSRF and to round-trip the PKCE verifier.
///
/// Items are auto-purged by DynamoDB's TTL (`ttl` attribute, expiry epoch in
/// seconds) — the table has TTL enabled via Terraform.
#[derive(Serialize, Deserialize)]
pub struct OAuthState {
    pub state: String,
    pub code_verifier: String,
    pub redirect_after: String,
    /// Unix epoch seconds at which this row expires.
    pub ttl: i64,
}

#[derive(Serialize, Deserialize)]
struct OAuthStateItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,

    state: String,
    code_verifier: String,
    redirect_after: String,
    ttl: i64,
}

const TTL_MINUTES: i64 = 10;

pub async fn put(
    client: &DbClient,
    state: &str,
    code_verifier: &str,
    redirect_after: &str,
) -> AppResult<()> {
    let ttl = (Utc::now() + Duration::minutes(TTL_MINUTES)).timestamp();

    let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(OAuthStateItem {
        pk: keys::oauth_state_pk(state),
        sk: keys::SK_OAUTH_STATE.to_string(),

        state: state.to_string(),
        code_verifier: code_verifier.to_string(),
        redirect_after: redirect_after.to_string(),
        ttl,
    })
    .map_err(|e| AppError::Internal(format!("serde_dynamo to_item: {e}")))?;

    client
        .inner
        .put_item()
        .table_name(&client.table_name)
        .set_item(Some(item))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb put_item: {e}")))?;
    Ok(())
}

/// Fetch and delete the state row in two operations. We delete unconditionally
/// after read so a replayed callback hits a Not-Found and is rejected.
pub async fn take(client: &DbClient, state: &str) -> AppResult<Option<OAuthState>> {
    let pk = keys::oauth_state_pk(state);
    let sk = keys::SK_OAUTH_STATE.to_string();

    let res = client
        .inner
        .get_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(pk.clone()))
        .key("SK", AttributeValue::S(sk.clone()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb get_item: {e}")))?;

    let Some(item) = res.item else {
        return Ok(None);
    };

    let parsed: OAuthStateItem = serde_dynamo::from_item(item)
        .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;

    // Best-effort delete. If it races with TTL eviction we simply move on.
    let _ = client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key("PK", AttributeValue::S(pk))
        .key("SK", AttributeValue::S(sk))
        .send()
        .await;

    // Reject expired entries explicitly in case TTL hasn't kicked in yet.
    if parsed.ttl < Utc::now().timestamp() {
        return Ok(None);
    }

    Ok(Some(OAuthState {
        state: parsed.state,
        code_verifier: parsed.code_verifier,
        redirect_after: parsed.redirect_after,
        ttl: parsed.ttl,
    }))
}
