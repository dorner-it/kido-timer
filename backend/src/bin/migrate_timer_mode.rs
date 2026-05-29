// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! One-shot migration: rewrite v1 `TimerMode` strings to v3 values in DDB.
//!
//! Run locally against the real table once, before deploying the v3
//! backend. Not wired into Lambda — the production hot path must not scan.
//!
//! Usage:
//!
//! ```sh
//! # dry-run preview (no writes)
//! AWS_PROFILE=aws cargo run --bin migrate_timer_mode -- \
//!     --table ki-do-timer-prod --dry-run
//!
//! # actually apply the rewrite
//! AWS_PROFILE=aws cargo run --bin migrate_timer_mode -- \
//!     --table ki-do-timer-prod
//! ```
//!
//! Idempotent: items already on a v3 value are left alone, so re-running
//! after a partial run is safe.

use std::env;
use std::process::ExitCode;

use ki_do_timer_backend::db::DbClient;
use ki_do_timer_backend::db::migrations::migrate_timer_mode_inplace;

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
            "--table" => {
                table = args.next();
            }
            "--dry-run" => {
                dry_run = true;
            }
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

    tracing::info!(table = %table, dry_run, "starting timer_mode migration");

    match migrate_timer_mode_inplace(&client, dry_run).await {
        Ok(report) => {
            tracing::info!(
                inspected = report.inspected,
                updated = report.updated,
                already_v3 = report.already_v3,
                unknown = report.unknown,
                dry_run,
                "migration done"
            );
            if report.unknown > 0 {
                eprintln!(
                    "warning: {} item(s) had unknown mode strings; check logs",
                    report.unknown
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
        "usage: migrate_timer_mode --table <table-name> [--dry-run]\n\
         \n\
         Rewrites legacy TimerMode strings (single_lane, two_lane_parallel,\n\
         relay, individual) to their v3 equivalents (gruppenstaffette,\n\
         loeschangriff) in the named DynamoDB table.\n\
         \n\
         Authenticate via the standard AWS SDK chain, e.g. AWS_PROFILE=aws."
    );
}
