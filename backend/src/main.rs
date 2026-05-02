// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use ki_do_timer_backend::{app, state::AppState};
use lambda_http::{Error, run};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .with_target(false)
        .with_current_span(false)
        .init();

    let state = AppState::from_env()
        .await
        .expect("AppState::from_env failed at cold start; check env vars and IAM");

    tracing::info!(
        env = %state.config.env,
        region = %state.config.aws_region,
        table = %state.config.table_name,
        cognito_pool = %state.config.cognito_user_pool_id,
        "starting up"
    );

    run(app::router(state)).await
}
