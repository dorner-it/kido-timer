import { describe, expect, it } from "vitest";
import type { OnPremStatus } from "../cloudTypes";
import {
  currentPicks,
  formatAgo,
  laneCandidates,
  nextRunForLane,
  picksToRunIds,
  showNoPhoneHint,
  withPick,
} from "../onprem";
import { run, RUN_A1, RUN_B1, RUN_C1, samplePayload } from "./fixtures";

const STATUS: OnPremStatus = {
  running: true,
  port: 8787,
  links: [],
  token: "t",
  ownerSub: "sub-1",
  uptimeSecs: 0,
  clients: [],
  error: null,
  judgeUiBuilt: true,
  uploadRuns: 0,
  uploadPenalties: 0,
  lastUploadAt: null,
};

describe("showNoPhoneHint", () => {
  it("only after the grace period without phones", () => {
    expect(showNoPhoneHint({ ...STATUS, uptimeSecs: 10 })).toBe(false);
    expect(showNoPhoneHint({ ...STATUS, uptimeSecs: 61 })).toBe(true);
    expect(
      showNoPhoneHint({
        ...STATUS,
        uptimeSecs: 120,
        clients: [{ ip: "192.168.1.5", lastSeenSecs: 3, requests: 9 }],
      }),
    ).toBe(false);
    expect(showNoPhoneHint({ ...STATUS, running: false, uptimeSecs: 120 })).toBe(false);
    expect(showNoPhoneHint({ ...STATUS, uptimeSecs: null })).toBe(false);
    expect(showNoPhoneHint({ ...STATUS, uptimeSecs: 5 }, 5)).toBe(true);
  });
});

describe("current-run picks", () => {
  it("reads current runs per lane", () => {
    expect(currentPicks(samplePayload())).toEqual([RUN_A1, RUN_B1]);
    expect(currentPicks(null)).toEqual([]);
    const s = samplePayload();
    s.event.current_run_ids = [RUN_B1];
    expect(currentPicks(s)).toEqual([null, RUN_B1]);
  });

  it("lane candidates: same lane or no lane, by run number", () => {
    const s = samplePayload();
    expect(laneCandidates(s, 1).map((r) => r.id)).toEqual([RUN_A1, RUN_C1]);
    expect(laneCandidates(s, 2).map((r) => r.id)).toEqual([RUN_B1, RUN_C1]);
  });

  it("picking a run on another lane moves it", () => {
    expect(withPick([RUN_A1, RUN_B1], 2, RUN_A1)).toEqual([null, RUN_A1]);
    expect(withPick([RUN_A1, RUN_B1], 1, null)).toEqual([null, RUN_B1]);
    expect(withPick([null, null], 1, RUN_C1)).toEqual([RUN_C1, null]);
  });

  it("picks → current_run_ids in lane order", () => {
    expect(picksToRunIds([RUN_A1, null])).toEqual([RUN_A1]);
    expect(picksToRunIds([null, RUN_B1])).toEqual([RUN_B1]);
    expect(picksToRunIds([])).toEqual([]);
  });

  it("next run for a lane skips completed and runs current elsewhere", () => {
    const s = samplePayload();
    s.runs.push(run({ id: "run-a2", team_id: "team-a", run_number: 4, lane: 1 }));
    s.runs.push(run({ id: "run-a3", team_id: "team-a", run_number: 5, lane: 1, status: "completed" }));
    expect(nextRunForLane(s, [RUN_A1, RUN_B1], 1)?.id).toBe(RUN_C1);
    expect(nextRunForLane(s, [RUN_A1, RUN_C1], 1)?.id).toBe("run-a2");
    expect(nextRunForLane(s, ["run-a2", RUN_B1], 1)).toBeNull();
    expect(nextRunForLane(s, [null, null], 2)?.id).toBe(RUN_B1);
  });
});

describe("formatAgo", () => {
  it("seconds and minutes", () => {
    expect(formatAgo(12)).toBe("vor 12 s");
    expect(formatAgo(180)).toBe("vor 3 min");
  });
});
