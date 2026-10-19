// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

//! Scenario tests for `kido_core::scoring`.

use chrono::NaiveDate;
use kido_core::scoring::{
    InvalidReason, PenaltyInput, RankRow, RankStatus, Results, RunInput, RunnerInput,
    TagessiegerRow, TeamInput, competition_ranks, compute_results, score_run, tagessieger_factor,
};
use kido_core::{PenaltyKind, PenaltyStatus, Wertungsgruppe};
use uuid::Uuid;

// ---------------------------------------------------------------- helpers

const SZ_OK: PenaltyInput = PenaltyInput {
    kind: PenaltyKind::Sz,
    status: PenaltyStatus::Approved,
};
const SZ_PENDING: PenaltyInput = PenaltyInput {
    kind: PenaltyKind::Sz,
    status: PenaltyStatus::Pending,
};
const SZ_REJECTED: PenaltyInput = PenaltyInput {
    kind: PenaltyKind::Sz,
    status: PenaltyStatus::Rejected,
};
const FV_OK: PenaltyInput = PenaltyInput {
    kind: PenaltyKind::Fv,
    status: PenaltyStatus::Approved,
};
const FV_PENDING: PenaltyInput = PenaltyInput {
    kind: PenaltyKind::Fv,
    status: PenaltyStatus::Pending,
};
const FV_REJECTED: PenaltyInput = PenaltyInput {
    kind: PenaltyKind::Fv,
    status: PenaltyStatus::Rejected,
};

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2027, 6, 12).unwrap()
}

/// Birth date giving exactly `age` completed years on [`day`].
fn born_for_age(age: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2027 - age, 1, 15).unwrap()
}

fn team_id(start_number: u32) -> Uuid {
    Uuid::from_u128(1_000 + u128::from(start_number))
}

fn run_id(start_number: u32, run_number: u32) -> Uuid {
    Uuid::from_u128(100_000 + u128::from(start_number) * 10 + u128::from(run_number))
}

fn runner_id(start_number: u32, idx: u32) -> Uuid {
    Uuid::from_u128(1_000_000 + u128::from(start_number) * 100 + u128::from(idx))
}

/// Team with a roster of the given ages (runner ids `runner_id(sn, i)`).
fn team_with_ages(start_number: u32, wg: Wertungsgruppe, ages: &[i32]) -> TeamInput {
    TeamInput {
        team_id: team_id(start_number),
        start_number,
        wg,
        runners: ages
            .iter()
            .enumerate()
            .map(|(i, &a)| RunnerInput {
                runner_id: runner_id(start_number, i as u32),
                birth_date: born_for_age(a),
            })
            .collect(),
    }
}

fn team(start_number: u32, wg: Wertungsgruppe) -> TeamInput {
    team_with_ages(start_number, wg, &[12, 12, 12, 12, 12, 12])
}

fn run(start_number: u32, run_number: u32, ms: Option<u32>, p: &[PenaltyInput]) -> RunInput {
    RunInput {
        run_id: run_id(start_number, run_number),
        team_id: team_id(start_number),
        run_number,
        completed: true,
        measured_ms: ms,
        penalties: p.to_vec(),
        lineup: Vec::new(),
    }
}

fn timed(start_number: u32, run_number: u32, ms: u32) -> RunInput {
    run(start_number, run_number, Some(ms), &[])
}

fn not_completed(start_number: u32, run_number: u32) -> RunInput {
    RunInput {
        completed: false,
        ..run(start_number, run_number, Some(40_000), &[])
    }
}

fn with_lineup(mut r: RunInput, lineup: Vec<Uuid>) -> RunInput {
    r.lineup = lineup;
    r
}

fn wg_rows(res: &Results, wg: Wertungsgruppe) -> &[RankRow] {
    &res.wg_rankings
        .iter()
        .find(|r| r.wg == wg)
        .unwrap_or_else(|| panic!("{wg:?} missing"))
        .rows
}

/// `(start_number, place, status)` per row.
fn summary(rows: &[RankRow]) -> Vec<(u32, Option<u32>, RankStatus)> {
    rows.iter()
        .map(|r| (r.start_number, r.place, r.status))
        .collect()
}

fn ts_summary(rows: &[TagessiegerRow]) -> Vec<(u32, u32, u32)> {
    rows.iter()
        .map(|r| (r.start_number, r.place, r.gw_ms))
        .collect()
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

use RankStatus::{AllInvalid, NotStarted, Ranked};
use Wertungsgruppe::{Wg1, Wg2, Wg3, Wg4, Wg5};

// --------------------------------------------------------------- score_run

#[test]
fn score_run_not_completed_is_none() {
    assert_eq!(score_run(&not_completed(1, 1)), None);
    let mut r = run(1, 1, None, &[FV_OK]);
    r.completed = false;
    assert_eq!(score_run(&r), None);
}

#[test]
fn score_run_no_penalties() {
    let r = score_run(&timed(1, 1, 47_231)).unwrap();
    assert!(r.valid);
    assert_eq!(r.invalid_reason, None);
    assert_eq!(r.total_ms, Some(47_231));
    assert_eq!((r.sz_count, r.fv_count), (0, 0));
    assert!(!r.provisional);
    assert_eq!(r.run_id, run_id(1, 1));
    assert_eq!(r.team_id, team_id(1));
    assert_eq!(r.run_number, 1);
}

#[test]
fn score_run_table() {
    // (penalties, measured, valid, reason, total, sz, fv, provisional)
    type Row = (
        Vec<PenaltyInput>,
        Option<u32>,
        bool,
        Option<InvalidReason>,
        Option<u32>,
        u32,
        u32,
        bool,
    );
    let cases: Vec<Row> = vec![
        (
            vec![SZ_OK],
            Some(40_000),
            true,
            None,
            Some(55_000),
            1,
            0,
            false,
        ),
        (
            vec![SZ_OK, SZ_OK, SZ_OK],
            Some(40_000),
            true,
            None,
            Some(85_000),
            3,
            0,
            false,
        ),
        (
            vec![SZ_REJECTED],
            Some(40_000),
            true,
            None,
            Some(40_000),
            0,
            0,
            false,
        ),
        (
            vec![FV_REJECTED],
            Some(40_000),
            true,
            None,
            Some(40_000),
            0,
            0,
            false,
        ),
        (
            vec![SZ_REJECTED, FV_REJECTED, SZ_OK],
            Some(40_000),
            true,
            None,
            Some(55_000),
            1,
            0,
            false,
        ),
        (
            vec![SZ_PENDING],
            Some(40_000),
            true,
            None,
            Some(55_000),
            1,
            0,
            true,
        ),
        (
            vec![SZ_PENDING, SZ_OK],
            Some(40_000),
            true,
            None,
            Some(70_000),
            2,
            0,
            true,
        ),
        (
            vec![FV_OK],
            Some(40_000),
            false,
            Some(InvalidReason::Fehlversuch),
            None,
            0,
            1,
            false,
        ),
        (
            vec![FV_PENDING],
            Some(40_000),
            false,
            Some(InvalidReason::Fehlversuch),
            None,
            0,
            1,
            true,
        ),
        (
            vec![SZ_OK, FV_OK, FV_OK],
            Some(40_000),
            false,
            Some(InvalidReason::Fehlversuch),
            None,
            1,
            2,
            false,
        ),
        (
            vec![],
            None,
            false,
            Some(InvalidReason::NoTime),
            None,
            0,
            0,
            false,
        ),
        (
            vec![SZ_OK],
            None,
            false,
            Some(InvalidReason::NoTime),
            None,
            1,
            0,
            false,
        ),
        // FV wins over missing time as the reason.
        (
            vec![FV_OK],
            None,
            false,
            Some(InvalidReason::Fehlversuch),
            None,
            0,
            1,
            false,
        ),
        // rejected pending-free FV + pending SZ: valid, provisional.
        (
            vec![FV_REJECTED, SZ_PENDING],
            Some(30_000),
            true,
            None,
            Some(45_000),
            1,
            0,
            true,
        ),
        // rejected entries never make a result provisional
        (
            vec![SZ_REJECTED, FV_REJECTED],
            Some(30_000),
            true,
            None,
            Some(30_000),
            0,
            0,
            false,
        ),
    ];
    for (i, (p, ms, valid, reason, total, sz, fv, prov)) in cases.into_iter().enumerate() {
        let r = score_run(&run(1, 1, ms, &p)).unwrap();
        assert_eq!(r.valid, valid, "case {i}");
        assert_eq!(r.invalid_reason, reason, "case {i}");
        assert_eq!(r.total_ms, total, "case {i}");
        assert_eq!(r.sz_count, sz, "case {i}");
        assert_eq!(r.fv_count, fv, "case {i}");
        assert_eq!(r.provisional, prov, "case {i}");
    }
}

#[test]
fn score_run_three_sz_is_plus_45s() {
    let r = score_run(&run(1, 1, Some(47_230), &[SZ_OK, SZ_OK, SZ_OK])).unwrap();
    assert_eq!(r.total_ms, Some(47_230 + 45_000));
}

#[test]
fn score_run_total_saturates() {
    let r = score_run(&run(1, 1, Some(u32::MAX - 1), &[SZ_OK])).unwrap();
    assert_eq!(r.total_ms, Some(u32::MAX));
}

// ------------------------------------------------------- competition_ranks

#[test]
fn competition_ranks_cases() {
    assert_eq!(competition_ranks::<u32>(&[]), Vec::<u32>::new());
    assert_eq!(competition_ranks(&[5]), vec![1]);
    assert_eq!(competition_ranks(&[1, 2, 3]), vec![1, 2, 3]);
    assert_eq!(competition_ranks(&[10, 10, 20]), vec![1, 1, 3]);
    assert_eq!(competition_ranks(&[10, 20, 20, 30]), vec![1, 2, 2, 4]);
    assert_eq!(competition_ranks(&[7, 7, 7, 7]), vec![1, 1, 1, 1]);
    assert_eq!(
        competition_ranks(&[1, 1, 2, 2, 2, 3]),
        vec![1, 1, 3, 3, 3, 6]
    );
    assert_eq!(competition_ranks(&["a", "b", "b"]), vec![1, 2, 2]);
}

// ------------------------------------------------------------- WG ranking

#[test]
fn empty_input_gives_empty_results() {
    let res = compute_results(&[], &[], day());
    assert!(res.wg_rankings.is_empty());
    assert!(res.tagessieger.is_empty());
}

#[test]
fn simple_ranking_by_total() {
    let teams = [team(1, Wg2), team(2, Wg2), team(3, Wg2)];
    let runs = [
        timed(1, 1, 50_000),
        timed(2, 1, 45_000),
        timed(3, 1, 48_000),
    ];
    let res = compute_results(&teams, &runs, day());
    let rows = wg_rows(&res, Wg2);
    assert_eq!(
        summary(rows),
        vec![
            (2, Some(1), Ranked),
            (3, Some(2), Ranked),
            (1, Some(3), Ranked)
        ]
    );
    assert_eq!(rows[0].best_run_id, Some(run_id(2, 1)));
    assert_eq!(rows[0].best_total_ms, Some(45_000));
}

#[test]
fn sz_affects_ranking() {
    // Team 1 is faster but gets 1 SZ (+15 s).
    let teams = [team(1, Wg2), team(2, Wg2)];
    let runs = [run(1, 1, Some(40_000), &[SZ_OK]), timed(2, 1, 50_000)];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        summary(wg_rows(&res, Wg2)),
        vec![(2, Some(1), Ranked), (1, Some(2), Ranked)]
    );
    assert_eq!(wg_rows(&res, Wg2)[1].best_total_ms, Some(55_000));
}

#[test]
fn ties_1_1_3() {
    let teams = [team(1, Wg2), team(2, Wg2), team(3, Wg2)];
    let runs = [
        timed(1, 1, 45_000),
        timed(2, 1, 45_000),
        timed(3, 1, 46_000),
    ];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        summary(wg_rows(&res, Wg2)),
        vec![
            (1, Some(1), Ranked),
            (2, Some(1), Ranked),
            (3, Some(3), Ranked)
        ]
    );
}

#[test]
fn ties_1_2_2_4() {
    let teams = [team(1, Wg3), team(2, Wg3), team(3, Wg3), team(4, Wg3)];
    let runs = [
        timed(1, 1, 50_000),
        timed(2, 1, 48_000),
        timed(3, 1, 48_000),
        timed(4, 1, 47_000),
    ];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        summary(wg_rows(&res, Wg3)),
        vec![
            (4, Some(1), Ranked),
            (2, Some(2), Ranked),
            (3, Some(2), Ranked),
            (1, Some(4), Ranked)
        ]
    );
}

#[test]
fn all_equal_and_single() {
    let teams = [team(3, Wg4), team(1, Wg4), team(2, Wg4)];
    let runs = [
        timed(1, 1, 45_000),
        timed(2, 1, 45_000),
        timed(3, 1, 45_000),
    ];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        summary(wg_rows(&res, Wg4)),
        vec![
            (1, Some(1), Ranked),
            (2, Some(1), Ranked),
            (3, Some(1), Ranked)
        ]
    );

    let res = compute_results(&[team(9, Wg5)], &[timed(9, 1, 60_000)], day());
    assert_eq!(summary(wg_rows(&res, Wg5)), vec![(9, Some(1), Ranked)]);
}

#[test]
fn tie_compared_at_centis() {
    // 47_231 and 47_234 both round to 47_230 → same place; the tie is
    // listed by start number even though team 2 is 3 ms faster.
    let teams = [team(1, Wg2), team(2, Wg2), team(3, Wg2)];
    let runs = [
        timed(1, 1, 47_234),
        timed(2, 1, 47_231),
        timed(3, 1, 47_236),
    ];
    let res = compute_results(&teams, &runs, day());
    let rows = wg_rows(&res, Wg2);
    assert_eq!(
        summary(rows),
        vec![
            (1, Some(1), Ranked),
            (2, Some(1), Ranked),
            (3, Some(3), Ranked)
        ]
    );
    // Raw totals are reported unrounded.
    assert_eq!(rows[0].best_total_ms, Some(47_234));
    assert_eq!(rows[1].best_total_ms, Some(47_231));
}

#[test]
fn different_centis_are_different_places() {
    // 47_234 → 47_230, 47_236 → 47_240.
    let teams = [team(1, Wg2), team(2, Wg2)];
    let runs = [timed(1, 1, 47_236), timed(2, 1, 47_234)];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        summary(wg_rows(&res, Wg2)),
        vec![(2, Some(1), Ranked), (1, Some(2), Ranked)]
    );
}

#[test]
fn best_run_lower_total_wins() {
    let teams = [team(1, Wg2)];
    let runs = [timed(1, 1, 50_000), timed(1, 2, 48_000)];
    let res = compute_results(&teams, &runs, day());
    let row = &wg_rows(&res, Wg2)[0];
    assert_eq!(row.best_run_id, Some(run_id(1, 2)));
    assert_eq!(row.best_total_ms, Some(48_000));
    let numbers: Vec<u32> = row.runs.iter().map(|r| r.run_number).collect();
    assert_eq!(numbers, [1, 2]);
}

#[test]
fn best_run_counts_penalties() {
    // Run 1 measured faster, but +15 s SZ makes run 2 the best.
    let teams = [team(1, Wg2)];
    let runs = [run(1, 1, Some(40_000), &[SZ_OK]), timed(1, 2, 50_000)];
    let res = compute_results(&teams, &runs, day());
    let row = &wg_rows(&res, Wg2)[0];
    assert_eq!(row.best_run_id, Some(run_id(1, 2)));
    assert_eq!(row.best_total_ms, Some(50_000));
}

#[test]
fn best_run_tie_takes_lower_run_number() {
    let teams = [team(1, Wg2)];
    // Passed in reverse order on purpose.
    let runs = [timed(1, 2, 48_000), timed(1, 1, 48_000)];
    let res = compute_results(&teams, &runs, day());
    let row = &wg_rows(&res, Wg2)[0];
    assert_eq!(row.best_run_id, Some(run_id(1, 1)));
    // Runs listed by run_number.
    assert_eq!(row.runs[0].run_number, 1);
    assert_eq!(row.runs[1].run_number, 2);
}

#[test]
fn best_run_tie_at_centis_takes_lower_run_number() {
    // 48_004 and 48_001 are equal at 1/100 s → run 1 counts.
    let teams = [team(1, Wg2)];
    let runs = [timed(1, 1, 48_004), timed(1, 2, 48_001)];
    let res = compute_results(&teams, &runs, day());
    let row = &wg_rows(&res, Wg2)[0];
    assert_eq!(row.best_run_id, Some(run_id(1, 1)));
    assert_eq!(row.best_total_ms, Some(48_004));
}

#[test]
fn invalid_run_ignored_for_best() {
    let teams = [team(1, Wg2)];
    let runs = [run(1, 1, Some(30_000), &[FV_OK]), timed(1, 2, 50_000)];
    let res = compute_results(&teams, &runs, day());
    let row = &wg_rows(&res, Wg2)[0];
    assert_eq!(row.status, Ranked);
    assert_eq!(row.best_run_id, Some(run_id(1, 2)));
    assert_eq!(row.runs.len(), 2);
    assert!(!row.runs[0].valid);
}

#[test]
fn all_invalid_shares_last_place_after_ranked() {
    let teams = [team(1, Wg2), team(2, Wg2), team(3, Wg2)];
    let runs = [
        run(1, 1, Some(40_000), &[FV_OK]),
        run(1, 2, None, &[]),
        timed(2, 1, 50_000),
        timed(3, 1, 49_000),
    ];
    let res = compute_results(&teams, &runs, day());
    let rows = wg_rows(&res, Wg2);
    assert_eq!(
        summary(rows),
        vec![
            (3, Some(1), Ranked),
            (2, Some(2), Ranked),
            (1, Some(3), AllInvalid)
        ]
    );
    let invalid = &rows[2];
    assert_eq!(invalid.best_run_id, None);
    assert_eq!(invalid.best_total_ms, None);
    assert_eq!(invalid.runs.len(), 2);
    assert_eq!(
        invalid.runs[0].invalid_reason,
        Some(InvalidReason::Fehlversuch)
    );
    assert_eq!(invalid.runs[1].invalid_reason, Some(InvalidReason::NoTime));
}

#[test]
fn two_all_invalid_teams_share_same_last_place() {
    let teams = [team(5, Wg2), team(4, Wg2), team(1, Wg2), team(2, Wg2)];
    let runs = [
        run(5, 1, Some(40_000), &[FV_OK]),
        run(4, 1, Some(40_000), &[FV_OK]),
        timed(1, 1, 50_000),
        timed(2, 1, 50_000),
    ];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        summary(wg_rows(&res, Wg2)),
        vec![
            (1, Some(1), Ranked),
            (2, Some(1), Ranked),
            (4, Some(3), AllInvalid),
            (5, Some(3), AllInvalid)
        ]
    );
}

#[test]
fn only_invalid_teams_get_place_1() {
    let teams = [team(1, Wg2), team(2, Wg2)];
    let runs = [run(1, 1, Some(1), &[FV_OK]), run(2, 1, None, &[])];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        summary(wg_rows(&res, Wg2)),
        vec![(1, Some(1), AllInvalid), (2, Some(1), AllInvalid)]
    );
}

#[test]
fn not_started_listed_last_without_place() {
    let teams = [
        team(7, Wg2),
        team(1, Wg2),
        team(2, Wg2),
        team(3, Wg2),
        team(6, Wg2),
    ];
    let runs = [
        timed(1, 1, 50_000),
        run(2, 1, Some(40_000), &[FV_OK]),
        // team 3: only a not-completed (pending/active/cancelled) run
        not_completed(3, 1),
        // team 6: two not-completed runs; team 7: no runs at all
        not_completed(6, 1),
        not_completed(6, 2),
    ];
    let res = compute_results(&teams, &runs, day());
    let rows = wg_rows(&res, Wg2);
    assert_eq!(
        summary(rows),
        vec![
            (1, Some(1), Ranked),
            (2, Some(2), AllInvalid),
            (3, None, NotStarted),
            (6, None, NotStarted),
            (7, None, NotStarted)
        ]
    );
    for r in &rows[2..] {
        assert!(r.runs.is_empty());
        assert_eq!(r.best_run_id, None);
        assert!(!r.provisional);
    }
}

#[test]
fn not_completed_run_ignored_beside_completed() {
    // A faster but not completed run must not count.
    let teams = [team(1, Wg2)];
    let mut fast = timed(1, 2, 10_000);
    fast.completed = false;
    let runs = [timed(1, 1, 50_000), fast];
    let res = compute_results(&teams, &runs, day());
    let row = &wg_rows(&res, Wg2)[0];
    assert_eq!(row.best_total_ms, Some(50_000));
    assert_eq!(row.runs.len(), 1);
}

#[test]
fn ordering_by_start_number_in_groups() {
    let teams = [
        team(9, Wg2),
        team(8, Wg2),
        team(3, Wg2),
        team(2, Wg2),
        team(5, Wg2),
        team(4, Wg2),
    ];
    let runs = [
        timed(9, 1, 45_000),
        timed(3, 1, 45_000),
        run(8, 1, None, &[]),
        run(2, 1, None, &[]),
    ];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        summary(wg_rows(&res, Wg2)),
        vec![
            (3, Some(1), Ranked),
            (9, Some(1), Ranked),
            (2, Some(3), AllInvalid),
            (8, Some(3), AllInvalid),
            (4, None, NotStarted),
            (5, None, NotStarted)
        ]
    );
}

#[test]
fn multiple_wgs_separated_and_sorted() {
    let teams = [
        team(1, Wg5),
        team(2, Wg1),
        team(3, Wg3),
        team(4, Wg1),
        team(5, Wg5),
    ];
    let runs = [
        timed(1, 1, 40_000),
        timed(2, 1, 60_000),
        timed(3, 1, 50_000),
        timed(4, 1, 55_000),
        timed(5, 1, 42_000),
    ];
    let res = compute_results(&teams, &runs, day());
    let wgs: Vec<Wertungsgruppe> = res.wg_rankings.iter().map(|r| r.wg).collect();
    assert_eq!(wgs, [Wg1, Wg3, Wg5]);
    assert_eq!(
        summary(wg_rows(&res, Wg1)),
        vec![(4, Some(1), Ranked), (2, Some(2), Ranked)]
    );
    assert_eq!(summary(wg_rows(&res, Wg3)), vec![(3, Some(1), Ranked)]);
    assert_eq!(
        summary(wg_rows(&res, Wg5)),
        vec![(1, Some(1), Ranked), (5, Some(2), Ranked)]
    );
}

#[test]
fn wg_with_only_not_started_teams_appears() {
    let teams = [team(1, Wg2), team(2, Wg4)];
    let runs = [timed(1, 1, 50_000)];
    let res = compute_results(&teams, &runs, day());
    let wgs: Vec<Wertungsgruppe> = res.wg_rankings.iter().map(|r| r.wg).collect();
    assert_eq!(wgs, [Wg2, Wg4]);
    assert_eq!(summary(wg_rows(&res, Wg4)), vec![(2, None, NotStarted)]);
}

#[test]
fn runs_of_unknown_teams_ignored() {
    let teams = [team(1, Wg2)];
    let runs = [timed(1, 1, 50_000), timed(99, 1, 10_000)];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(summary(wg_rows(&res, Wg2)), vec![(1, Some(1), Ranked)]);
    assert_eq!(res.tagessieger.len(), 1);
}

#[test]
fn provisional_propagates_to_row() {
    let teams = [team(1, Wg2), team(2, Wg2), team(3, Wg2)];
    let runs = [
        // pending SZ on the non-best run still makes the team provisional
        timed(1, 1, 45_000),
        run(1, 2, Some(50_000), &[SZ_PENDING]),
        timed(2, 1, 46_000),
        // pending FV → invalid but provisional
        run(3, 1, Some(40_000), &[FV_PENDING]),
    ];
    let res = compute_results(&teams, &runs, day());
    let rows = wg_rows(&res, Wg2);
    assert_eq!(
        summary(rows),
        vec![
            (1, Some(1), Ranked),
            (2, Some(2), Ranked),
            (3, Some(3), AllInvalid)
        ]
    );
    assert!(rows[0].provisional);
    assert!(!rows[1].provisional);
    assert!(rows[2].provisional);
    assert!(!rows[0].runs[0].provisional);
    assert!(rows[0].runs[1].provisional);
}

#[test]
fn results_serialize_snake_case() {
    let teams = [team(1, Wg2), team(2, Wg2)];
    let runs = [run(1, 1, Some(40_000), &[FV_OK])];
    let res = compute_results(&teams, &runs, day());
    let v = serde_json::to_value(&res).unwrap();
    let rows = &v["wg_rankings"][0]["rows"];
    assert_eq!(v["wg_rankings"][0]["wg"], "wg2");
    assert_eq!(rows[0]["status"], "all_invalid");
    assert_eq!(rows[0]["runs"][0]["invalid_reason"], "fehlversuch");
    assert_eq!(rows[1]["status"], "not_started");
    assert!(rows[1]["place"].is_null());
    let back: Results = serde_json::from_value(v).unwrap();
    assert_eq!(back, res);
}

// ------------------------------------------------------------ Tagessieger

#[test]
fn factor_formula() {
    assert!(approx(tagessieger_factor(12.0), 0.88));
    assert!(approx(tagessieger_factor(18.0), 1.0));
    assert!(approx(tagessieger_factor(10.5), 0.85));
    assert!(approx(tagessieger_factor(8.0), 0.80));
    assert!(approx(tagessieger_factor(19.0), 1.02));
}

#[test]
fn tagessieger_basic_avg_and_gw() {
    // Lineup ages 12 → factor 0.88; 50_000 × 0.88 = 44_000.
    let teams = [team_with_ages(1, Wg2, &[12, 12, 12, 12, 12, 12])];
    let lineup: Vec<Uuid> = (0..6).map(|i| runner_id(1, i)).collect();
    let runs = [with_lineup(timed(1, 1, 50_000), lineup)];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(res.tagessieger.len(), 1);
    let r = &res.tagessieger[0];
    assert_eq!(r.team_id, team_id(1));
    assert_eq!(r.wg, Wg2);
    assert_eq!(r.run_id, run_id(1, 1));
    assert_eq!(r.total_ms, 50_000);
    assert_eq!(r.ages, vec![12; 6]);
    assert!(approx(r.avg_age, 12.0));
    assert!(approx(r.factor, 0.88));
    assert_eq!(r.gw_ms, 44_000);
    assert_eq!(r.place, 1);
    assert!(!r.lineup_fallback);
    assert!(!r.provisional);
}

#[test]
fn tagessieger_avg_10_5_gives_085() {
    let teams = [team_with_ages(1, Wg1, &[10, 11])];
    let lineup = vec![runner_id(1, 0), runner_id(1, 1)];
    let runs = [with_lineup(timed(1, 1, 60_000), lineup)];
    let res = compute_results(&teams, &runs, day());
    let r = &res.tagessieger[0];
    assert_eq!(r.ages, vec![10, 11]);
    assert!(approx(r.avg_age, 10.5));
    assert!(approx(r.factor, 0.85));
    assert_eq!(r.gw_ms, 51_000);
}

#[test]
fn tagessieger_ages_on_competition_day_not_dec31() {
    // Born 2015-09-01: 31.12.-age 12, but on 2027-06-12 only 11.
    let mut t = team_with_ages(1, Wg2, &[]);
    t.runners = vec![
        RunnerInput {
            runner_id: runner_id(1, 0),
            birth_date: NaiveDate::from_ymd_opt(2015, 9, 1).unwrap(),
        },
        RunnerInput {
            runner_id: runner_id(1, 1),
            // birthday exactly on the competition day → completed
            birth_date: NaiveDate::from_ymd_opt(2015, 6, 12).unwrap(),
        },
    ];
    let runs = [with_lineup(
        timed(1, 1, 50_000),
        vec![runner_id(1, 0), runner_id(1, 1)],
    )];
    let res = compute_results(&[t], &runs, day());
    let r = &res.tagessieger[0];
    assert_eq!(r.ages, vec![11, 12]);
    assert!(approx(r.avg_age, 11.5));
    // 50_000 × 0.87 = 43_500
    assert_eq!(r.gw_ms, 43_500);
}

#[test]
fn tagessieger_uses_lineup_not_roster() {
    // Roster has a young substitute (8) who did not run.
    let teams = [team_with_ages(1, Wg2, &[14, 14, 14, 14, 14, 14, 8])];
    let lineup: Vec<Uuid> = (0..6).map(|i| runner_id(1, i)).collect();
    let runs = [with_lineup(timed(1, 1, 50_000), lineup)];
    let res = compute_results(&teams, &runs, day());
    let r = &res.tagessieger[0];
    assert_eq!(r.ages, vec![14; 6]);
    assert!(approx(r.factor, 0.92));
    assert_eq!(r.gw_ms, 46_000);
    assert!(!r.lineup_fallback);
}

#[test]
fn tagessieger_lineup_fallback_to_roster() {
    // Empty lineup → full roster (incl. substitute), flag set.
    let teams = [team_with_ages(1, Wg2, &[12, 12, 12, 12, 12, 12, 18])];
    let runs = [timed(1, 1, 70_000)];
    let res = compute_results(&teams, &runs, day());
    let r = &res.tagessieger[0];
    assert!(r.lineup_fallback);
    assert_eq!(r.ages.len(), 7);
    // sum = 90, n = 7, avg ≈ 12.857 → factor ≈ 0.897142…
    // gw = 70_000 × (64·7 + 180) / 700 = 70_000 × 628 / 700 = 62_800
    assert_eq!(r.gw_ms, 62_800);
    assert!(approx(r.avg_age, 90.0 / 7.0));
}

#[test]
fn tagessieger_unknown_lineup_ids_ignored() {
    let teams = [team_with_ages(1, Wg2, &[12, 16])];
    let stranger = Uuid::from_u128(42);
    let runs = [with_lineup(
        timed(1, 1, 50_000),
        vec![runner_id(1, 0), stranger, runner_id(1, 0)],
    )];
    let res = compute_results(&teams, &runs, day());
    let r = &res.tagessieger[0];
    // Only the known runner counts (once); no fallback.
    assert_eq!(r.ages, vec![12]);
    assert!(!r.lineup_fallback);
    assert_eq!(r.gw_ms, 44_000);
}

#[test]
fn tagessieger_only_unknown_lineup_ids_falls_back() {
    let teams = [team_with_ages(1, Wg2, &[12, 16])];
    let runs = [with_lineup(timed(1, 1, 50_000), vec![Uuid::from_u128(42)])];
    let res = compute_results(&teams, &runs, day());
    let r = &res.tagessieger[0];
    assert!(r.lineup_fallback);
    assert_eq!(r.ages, vec![12, 16]);
    // avg 14 → 0.92
    assert_eq!(r.gw_ms, 46_000);
}

#[test]
fn tagessieger_team_without_lineup_and_roster_left_out() {
    let teams = [team_with_ages(1, Wg2, &[]), team(2, Wg2)];
    let runs = [timed(1, 1, 40_000), timed(2, 1, 50_000)];
    let res = compute_results(&teams, &runs, day());
    // Still ranked in the WG …
    assert_eq!(
        summary(wg_rows(&res, Wg2)),
        vec![(1, Some(1), Ranked), (2, Some(2), Ranked)]
    );
    // … but not in the Tagessieger list.
    assert_eq!(res.tagessieger.len(), 1);
    assert_eq!(res.tagessieger[0].start_number, 2);
    assert_eq!(res.tagessieger[0].place, 1);
}

#[test]
fn tagessieger_team_without_valid_run_left_out() {
    let teams = [team(1, Wg2), team(2, Wg2), team(3, Wg2)];
    let runs = [run(1, 1, Some(30_000), &[FV_OK]), timed(2, 1, 50_000)];
    let res = compute_results(&teams, &runs, day());
    let sn: Vec<u32> = res.tagessieger.iter().map(|r| r.start_number).collect();
    assert_eq!(sn, [2]);
}

#[test]
fn tagessieger_variant_a_uses_best_time_run() {
    // Run 1: 50_000 ms with an older lineup (avg 16 → 0.96 → GW 48_000).
    // Run 2: 51_000 ms with a younger lineup (avg 10 → 0.84 → GW 42_840).
    // Run 2 would have the better GW-Zeit, but variant (a) takes the
    // best-time run (run 1) with its own lineup.
    let teams = [team_with_ages(1, Wg2, &[16, 16, 16, 10, 10, 10])];
    let old: Vec<Uuid> = (0..3).map(|i| runner_id(1, i)).collect();
    let young: Vec<Uuid> = (3..6).map(|i| runner_id(1, i)).collect();
    let runs = [
        with_lineup(timed(1, 1, 50_000), old),
        with_lineup(timed(1, 2, 51_000), young),
    ];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(wg_rows(&res, Wg2)[0].best_run_id, Some(run_id(1, 1)));
    let r = &res.tagessieger[0];
    assert_eq!(r.run_id, run_id(1, 1));
    assert_eq!(r.total_ms, 50_000);
    assert_eq!(r.ages, vec![16, 16, 16]);
    assert!(approx(r.factor, 0.96));
    assert_eq!(r.gw_ms, 48_000);
}

#[test]
fn tagessieger_gw_rounding_half_up_to_centis() {
    // factor 1.0 (avg 18): 47_235 → 4_723.5 cs → 47_240.
    // factor 0.88: 50_006 × 0.88 = 44_005.28 → 44_010;
    //              50_001 × 0.88 = 44_000.88 → 44_000.
    let teams = [
        team_with_ages(1, Wg4, &[18]),
        team_with_ages(2, Wg2, &[12]),
        team_with_ages(3, Wg2, &[12]),
        team_with_ages(4, Wg4, &[18]),
    ];
    let runs = [
        timed(1, 1, 47_235),
        timed(2, 1, 50_006),
        timed(3, 1, 50_001),
        timed(4, 1, 47_234),
    ];
    let res = compute_results(&teams, &runs, day());
    let gw = |sn: u32| {
        res.tagessieger
            .iter()
            .find(|r| r.start_number == sn)
            .unwrap()
            .gw_ms
    };
    assert_eq!(gw(1), 47_240);
    assert_eq!(gw(2), 44_010);
    assert_eq!(gw(3), 44_000);
    assert_eq!(gw(4), 47_230);
    assert!(res.tagessieger.iter().all(|r| r.gw_ms % 10 == 0));
}

#[test]
fn tagessieger_exact_integer_math_no_float_noise() {
    // avg 14.1 (ages 14×9 + 15) → factor 0.922; 10_000 × 0.922 = 9_220
    // exactly; floats would give 9219.999…
    let ages = [14, 14, 14, 14, 14, 14, 14, 14, 14, 15];
    let teams = [team_with_ages(1, Wg2, &ages)];
    let runs = [timed(1, 1, 10_000)];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(res.tagessieger[0].gw_ms, 9_220);
    // 12_345 × 0.88 = 10_863.6 ms = 1_086.36 cs → 10_860
    let teams = [team_with_ages(1, Wg2, &[12])];
    let res = compute_results(&teams, &[timed(1, 1, 12_345)], day());
    assert_eq!(res.tagessieger[0].gw_ms, 10_860);
    // 10_631 × 0.88 = 9_355.28 ms = 935.528 cs → 9_360
    let res = compute_results(&teams, &[timed(1, 1, 10_631)], day());
    assert_eq!(res.tagessieger[0].gw_ms, 9_360);
    // exactly 0.5 cs rounds up, 0.4 cs down (factor 1.0)
    let teams = [team_with_ages(1, Wg2, &[18])];
    let res = compute_results(&teams, &[timed(1, 1, 5)], day());
    assert_eq!(res.tagessieger[0].gw_ms, 10);
    let res = compute_results(&teams, &[timed(1, 1, 4)], day());
    assert_eq!(res.tagessieger[0].gw_ms, 0);
}

#[test]
fn tagessieger_ranking_ties_1_1_3_across_wgs() {
    // T1 (WG4, avg 18): 50_000 × 1.0 = 50_000
    // T2 (WG1, avg 8):  62_500 × 0.8 = 50_000
    // T3 (WG2, avg 12): 60_000 × 0.88 = 52_800
    // T4 (WG2, avg 12): 40_000 × 0.88 = 35_200
    let teams = [
        team_with_ages(3, Wg2, &[12]),
        team_with_ages(2, Wg1, &[8]),
        team_with_ages(1, Wg4, &[18]),
        team_with_ages(4, Wg2, &[12]),
    ];
    let runs = [
        timed(1, 1, 50_000),
        timed(2, 1, 62_500),
        timed(3, 1, 60_000),
        timed(4, 1, 40_000),
    ];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        ts_summary(&res.tagessieger),
        vec![
            (4, 1, 35_200),
            (1, 2, 50_000),
            (2, 2, 50_000),
            (3, 4, 52_800)
        ]
    );
    let wgs: Vec<Wertungsgruppe> = res.tagessieger.iter().map(|r| r.wg).collect();
    assert_eq!(wgs, [Wg2, Wg4, Wg1, Wg2]);
}

#[test]
fn tagessieger_ties_compared_after_rounding() {
    // 50_004 × 1.0 → 50_000 and 50_001 × 1.0 → 50_000: same place.
    let teams = [team_with_ages(2, Wg4, &[18]), team_with_ages(1, Wg4, &[18])];
    let runs = [timed(1, 1, 50_004), timed(2, 1, 50_001)];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        ts_summary(&res.tagessieger),
        vec![(1, 1, 50_000), (2, 1, 50_000)]
    );
}

#[test]
fn tagessieger_provisional_propagated() {
    let teams = [team(1, Wg2), team(2, Wg2), team(3, Wg2)];
    let runs = [
        run(1, 1, Some(40_000), &[SZ_PENDING]),
        timed(2, 1, 60_000),
        // pending SZ only on the non-counting run
        timed(3, 1, 45_000),
        run(3, 2, Some(50_000), &[SZ_PENDING]),
    ];
    let res = compute_results(&teams, &runs, day());
    let prov = |sn: u32| {
        res.tagessieger
            .iter()
            .find(|r| r.start_number == sn)
            .unwrap()
            .provisional
    };
    assert!(prov(1));
    assert!(!prov(2));
    assert!(prov(3));
}

#[test]
fn tagessieger_includes_sz_in_total() {
    let teams = [team_with_ages(1, Wg2, &[12])];
    let runs = [run(1, 1, Some(35_000), &[SZ_OK])];
    let res = compute_results(&teams, &runs, day());
    let r = &res.tagessieger[0];
    assert_eq!(r.total_ms, 50_000);
    assert_eq!(r.gw_ms, 44_000);
}

#[test]
fn full_competition_scenario() {
    // Two WGs, mixed outcomes, two runs per team.
    let teams = [
        team_with_ages(1, Wg1, &[9, 9, 10, 10, 8, 8]),
        team_with_ages(2, Wg1, &[10, 10, 10, 10, 10, 10]),
        team_with_ages(3, Wg1, &[9, 9, 9, 9, 9, 9]),
        team_with_ages(4, Wg2, &[14, 14, 13, 13, 12, 12]),
        team_with_ages(5, Wg2, &[13, 13, 13, 13, 13, 13]),
        team_with_ages(6, Wg2, &[14, 14, 14, 14, 14, 14]),
    ];
    let runs = [
        // WG1
        run(1, 1, Some(70_000), &[SZ_OK, SZ_REJECTED]), // 85_000
        timed(1, 2, 72_000),                            // 72_000 best
        run(2, 1, Some(60_000), &[FV_OK]),
        run(2, 2, Some(65_000), &[FV_PENDING]),
        timed(3, 1, 72_004), // 72_000 at cs → tie with team 1
        // WG2
        timed(4, 1, 55_000),
        run(4, 2, Some(50_000), &[SZ_OK]), // 65_000
        timed(5, 1, 52_000),
        not_completed(6, 1),
    ];
    let res = compute_results(&teams, &runs, day());
    assert_eq!(
        summary(wg_rows(&res, Wg1)),
        vec![
            (1, Some(1), Ranked),
            (3, Some(1), Ranked),
            (2, Some(3), AllInvalid)
        ]
    );
    assert!(wg_rows(&res, Wg1)[2].provisional);
    assert_eq!(
        summary(wg_rows(&res, Wg2)),
        vec![
            (5, Some(1), Ranked),
            (4, Some(2), Ranked),
            (6, None, NotStarted)
        ]
    );
    // Tagessieger (roster fallback, ages from roster):
    // T1: 72_000, avg 9   → 0.82 → 59_040
    // T3: 72_004, avg 9   → 0.82 → 59_043.28 → 59_040
    // T5: 52_000, avg 13  → 0.90 → 46_800
    // T4: 55_000, avg 13  → 0.90 → 49_500
    assert_eq!(
        ts_summary(&res.tagessieger),
        vec![
            (5, 1, 46_800),
            (4, 2, 49_500),
            (1, 3, 59_040),
            (3, 3, 59_040)
        ]
    );
    assert!(res.tagessieger.iter().all(|r| r.lineup_fallback));
}
