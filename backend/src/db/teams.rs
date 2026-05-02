// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::client::DbClient;
use crate::db::keys;
use crate::error::{AppError, AppResult};
use crate::models::Team;

#[derive(Serialize, Deserialize)]
struct TeamItem {
    #[serde(rename = "PK")]
    pk: String,
    #[serde(rename = "SK")]
    sk: String,

    id: Uuid,
    competition_id: Uuid,
    name: String,
    start_number: u32,
}

impl TeamItem {
    fn from_team(t: &Team) -> Self {
        Self {
            pk: keys::competition_pk(t.competition_id),
            sk: keys::competition_sk_team(t.start_number, t.id),

            id: t.id,
            competition_id: t.competition_id,
            name: t.name.clone(),
            start_number: t.start_number,
        }
    }

    fn into_team(self) -> Team {
        Team {
            id: self.id,
            competition_id: self.competition_id,
            name: self.name,
            start_number: self.start_number,
        }
    }
}

pub async fn put(client: &DbClient, team: &Team) -> AppResult<()> {
    let item: HashMap<String, AttributeValue> = serde_dynamo::to_item(TeamItem::from_team(team))
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

pub async fn delete(client: &DbClient, team: &Team) -> AppResult<()> {
    client
        .inner
        .delete_item()
        .table_name(&client.table_name)
        .key(
            "PK",
            AttributeValue::S(keys::competition_pk(team.competition_id)),
        )
        .key(
            "SK",
            AttributeValue::S(keys::competition_sk_team(team.start_number, team.id)),
        )
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb delete_item: {e}")))?;
    Ok(())
}

pub async fn list_for_competition(
    client: &DbClient,
    competition_id: Uuid,
) -> AppResult<Vec<Team>> {
    let res = client
        .inner
        .query()
        .table_name(&client.table_name)
        .key_condition_expression("PK = :pk AND begins_with(SK, :prefix)")
        .expression_attribute_values(":pk", AttributeValue::S(keys::competition_pk(competition_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("TEAM#".to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("dynamodb query: {e}")))?;

    let items = res.items.unwrap_or_default();
    let mut teams = Vec::with_capacity(items.len());
    for item in items {
        let parsed: TeamItem = serde_dynamo::from_item(item)
            .map_err(|e| AppError::Internal(format!("serde_dynamo from_item: {e}")))?;
        teams.push(parsed.into_team());
    }
    Ok(teams)
}
