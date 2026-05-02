// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

use ki_do_timer_backend::app;
use lambda_http::{Error, run};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("LOG_LEVEL")
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .with_target(false)
        .with_current_span(false)
        .init();

    run(app::router()).await
}
