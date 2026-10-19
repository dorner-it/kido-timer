// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! One-shot, optional migration: move runner rows from the legacy sort key
//! `RUNNER#<team_id>#<position:04>` to `RUNNER#<team_id>#<position:04>#<id>`.
//!
//! Not required for correctness — the backend reads both key shapes and
//! re-keys a runner on its next write. Run it to clean up old rows.
//! Not wired into Lambda — the production hot path must not scan.
//!
//! Usage:
//!
//! ```sh
//! # dry-run preview (no writes)
//! AWS_PROFILE=aws cargo run --bin migrate_runner_keys -- \
//!     --table ki-do-timer-prod --dry-run
//!
//! # actually apply
//! AWS_PROFILE=aws cargo run --bin migrate_runner_keys -- \
//!     --table ki-do-timer-prod
//! ```
//!
//! Idempotent: rows already on the new key are left alone.

use std::env;
use std::process::ExitCode;

use ki_do_timer_backend::db::DbClient;
use ki_do_timer_backend::db::migrations::migrate_runner_keys;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mut args = env::args().skip(1);
    let mut table: Option<String> = None;
    let mut dry_run = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--table" => table = args.next(),
            "--dry-run" => dry_run = true,
            "--help" | "-h" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("error: unrecognised argument {other}");
                print_usage();
                return ExitCode::from(2);
            }
        }
    }

    let Some(table) = table else {
        eprintln!("error: --table <name> is required");
        print_usage();
        return ExitCode::from(2);
    };

    let client = DbClient::from_env(table.clone()).await;
    tracing::info!(table = %table, dry_run, "starting runner key migration");

    match migrate_runner_keys(&client, dry_run).await {
        Ok(report) => {
            tracing::info!(
                inspected = report.inspected,
                migrated = report.migrated,
                already_current = report.already_current,
                stale_legacy_removed = report.stale_legacy_removed,
                skipped = report.skipped,
                dry_run,
                "migration done"
            );
            if report.skipped > 0 {
                eprintln!(
                    "warning: {} runner row(s) skipped; check logs",
                    report.skipped
                );
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!(
        "usage: migrate_runner_keys --table <table-name> [--dry-run]\n\
         \n\
         Moves runner rows from the legacy sort key RUNNER#<team>#<pos>\n\
         to RUNNER#<team>#<pos>#<id> in the named DynamoDB table.\n\
         \n\
         Authenticate via the standard AWS SDK chain, e.g. AWS_PROFILE=aws."
    );
}
