import { describe, expect, it } from "vitest";
import {
  anyProvisional,
  buildTagessiegerRows,
  buildWgRows,
  formatRunCell,
  maxRunsPerTeam,
  teamsWithoutWgNames,
  type RankRow,
  type ResultsResponse,
  type RunResult,
  type WgRanking,
} from "../results";
import { RUN_A1, RUN_B1, run, samplePayload } from "./fixtures";

function rr(partial: Partial<RunResult> & Pick<RunResult, "run_id" | "team_id">): RunResult {
  return {
    run_number: 1,
    valid: true,
    invalid_reason: null,
    total_ms: 47_240,
    sz_count: 0,
    fv_count: 0,
    provisional: false,
    ...partial,
  };
}

function row(partial: Partial<RankRow> & Pick<RankRow, "team_id">): RankRow {
  return {
    start_number: 1,
    place: 1,
    status: "ranked",
    best_run_id: null,
    best_total_ms: null,
    runs: [],
    provisional: false,
    ...partial,
  };
}

const snapshot = samplePayload();

describe("formatRunCell", () => {
  it("valid run, with SZ", () => {
    expect(formatRunCell(rr({ run_id: RUN_A1, team_id: "team-a" }), snapshot)).toBe("47,24");
    expect(
      formatRunCell(
        rr({ run_id: RUN_A1, team_id: "team-a", total_ms: 77_240, sz_count: 2 }),
        snapshot,
      ),
    ).toBe("1:17,24 (+30 s)");
  });
  it("invalid runs", () => {
    expect(
      formatRunCell(
        rr({ run_id: RUN_A1, team_id: "team-a", valid: false, invalid_reason: "fehlversuch", total_ms: null }),
        snapshot,
      ),
    ).toBe("FV");
    expect(
      formatRunCell(
        rr({ run_id: RUN_A1, team_id: "team-a", valid: false, invalid_reason: "no_time", total_ms: null }),
        snapshot,
      ),
    ).toBe("o. Zeit");
  });
  it("marks hand times and provisional results", () => {
    const s = samplePayload();
    s.runs[0] = run({ id: RUN_A1, team_id: "team-a", run_number: 1, time_source: "handstopp" });
    expect(
      formatRunCell(rr({ run_id: RUN_A1, team_id: "team-a", provisional: true }), s),
    ).toBe("47,24 H*");
  });
});

describe("buildWgRows", () => {
  const ranking: WgRanking = {
    wg: "wg2",
    rows: [
      row({
        team_id: "team-a",
        place: 1,
        best_run_id: RUN_A1,
        best_total_ms: 47_240,
        runs: [
          rr({ run_id: RUN_A1, team_id: "team-a" }),
          rr({ run_id: "x2", team_id: "team-a", run_number: 2, total_ms: 50_000 }),
        ],
      }),
      row({
        team_id: "team-b",
        start_number: 2,
        place: 2,
        status: "all_invalid",
        runs: [
          rr({ run_id: RUN_B1, team_id: "team-b", valid: false, invalid_reason: "fehlversuch", total_ms: null }),
        ],
        provisional: true,
      }),
      row({ team_id: "team-c", start_number: 3, place: null, status: "not_started" }),
    ],
  };

  it("builds place, team, run columns, best and notes", () => {
    const rows = buildWgRows(ranking, snapshot);
    expect(maxRunsPerTeam(ranking)).toBe(2);
    expect(rows[0]).toMatchObject({
      place: "1.",
      startNumber: 1,
      teamName: "FF Alpha",
      runCells: ["47,24", "50,00"],
      best: "47,24",
      note: "",
    });
    expect(rows[1]).toMatchObject({
      place: "2.",
      teamName: "FF Beta",
      runCells: ["FV", ""],
      best: "—",
      note: "ungültig, vorläufig",
    });
    expect(rows[2]).toMatchObject({ place: "n.a.", runCells: ["", ""], note: "nicht angetreten" });
  });

  it("notes Handzeit for the counting run", () => {
    const s = samplePayload();
    s.runs[0] = run({ id: RUN_A1, team_id: "team-a", run_number: 1, time_source: "handstopp" });
    expect(buildWgRows(ranking, s)[0].note).toBe("Handzeit");
  });

  it("unknown team name falls back to ?", () => {
    const r: WgRanking = { wg: "wg1", rows: [row({ team_id: "ghost" })] };
    expect(buildWgRows(r, snapshot)[0].teamName).toBe("?");
  });
});

describe("buildTagessiegerRows", () => {
  it("formats age, factor and GW-Zeit", () => {
    const rows = buildTagessiegerRows(
      [
        {
          team_id: "team-a",
          start_number: 1,
          wg: "wg2",
          run_id: RUN_A1,
          total_ms: 50_000,
          ages: [12, 13],
          avg_age: 12.5,
          factor: 0.89,
          gw_ms: 44_500,
          place: 1,
          lineup_fallback: true,
          provisional: true,
        },
      ],
      snapshot,
    );
    expect(rows[0]).toEqual({
      key: "team-a",
      place: "1.",
      startNumber: 1,
      teamName: "FF Alpha",
      wg: "WG 2",
      time: "50,00",
      avgAge: "12,50",
      factor: "0,8900",
      gw: "44,50",
      note: "ohne Besetzung (ganzer Kader), vorläufig",
      provisional: true,
    });
  });
});

describe("response helpers", () => {
  const response: ResultsResponse = {
    results: { wg_rankings: [], tagessieger: [] },
    teams_without_wg: ["team-c"],
    runners_without_birth_date: [],
    competition_day: "2027-06-12",
  };
  it("names teams without WG", () => {
    expect(teamsWithoutWgNames(response, snapshot)).toEqual(["FF Gamma"]);
  });
  it("anyProvisional", () => {
    expect(anyProvisional(response.results)).toBe(false);
    expect(
      anyProvisional({
        wg_rankings: [{ wg: "wg1", rows: [row({ team_id: "a", provisional: true })] }],
        tagessieger: [],
      }),
    ).toBe(true);
  });
});
