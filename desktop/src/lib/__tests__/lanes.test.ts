import { describe, expect, it } from "vitest";
import {
  currentRunsByLane,
  currentRunsOrdered,
  HARDWARE_CHANNELS,
  laneCountFor,
  laneGridClass,
  resolveLane,
  resolveRunForLane,
} from "../lanes";
import { RUN_A1, RUN_B1, RUN_C1, samplePayload } from "./fixtures";

describe("laneCountFor", () => {
  it("shows all hardware channels without a discipline", () => {
    expect(laneCountFor(null)).toBe(HARDWARE_CHANNELS);
    expect(HARDWARE_CHANNELS).toBe(4);
  });
  it("uses the discipline lane_count", () => {
    const s = samplePayload();
    expect(laneCountFor(s)).toBe(2);
    s.discipline.lane_count = 1;
    expect(laneCountFor(s)).toBe(1);
    s.discipline.mode = "loeschangriff";
    s.discipline.lane_count = 4;
    expect(laneCountFor(s)).toBe(4);
  });
  it("clamps nonsense values", () => {
    const s = samplePayload();
    s.discipline.lane_count = 0;
    expect(laneCountFor(s)).toBe(1);
    s.discipline.lane_count = 9;
    expect(laneCountFor(s)).toBe(4);
  });
});

describe("resolveLane", () => {
  it("maps current runs by run.lane", () => {
    const s = samplePayload();
    expect(resolveRunForLane(s, 1)?.id).toBe(RUN_A1);
    expect(resolveRunForLane(s, 2)?.id).toBe(RUN_B1);
    expect(resolveLane(s, 1)?.source).toBe("current");
  });

  it("does not depend on the order of current_run_ids", () => {
    const s = samplePayload();
    s.event.current_run_ids = [RUN_B1, RUN_A1];
    expect(resolveRunForLane(s, 1)?.id).toBe(RUN_A1);
    expect(resolveRunForLane(s, 2)?.id).toBe(RUN_B1);
  });

  it("current run wins over an operator override", () => {
    const s = samplePayload();
    expect(resolveRunForLane(s, 1, { 1: RUN_C1 })?.id).toBe(RUN_A1);
  });

  it("skips a completed current run (no overwrite of results)", () => {
    const s = samplePayload();
    s.runs[0].status = "completed";
    expect(resolveLane(s, 1)).toBeNull();
    expect(resolveLane(s, 1, { 1: RUN_C1 })?.source).toBe("override");
    s.runs[0].status = "cancelled";
    expect(resolveRunForLane(s, 1)?.id).toBe(RUN_A1);
    // The banner still lists it.
    s.runs[0].status = "completed";
    expect(currentRunsOrdered(s).map((r) => r.id)).toContain(RUN_A1);
  });

  it("uses the override when no current run is on the lane", () => {
    const s = samplePayload();
    s.event.current_run_ids = [RUN_A1];
    const r = resolveLane(s, 2, { 2: RUN_C1 });
    expect(r?.run.id).toBe(RUN_C1);
    expect(r?.source).toBe("override");
  });

  it("ignores overrides to unknown runs", () => {
    const s = samplePayload();
    s.event.current_run_ids = [];
    expect(resolveLane(s, 1, { 1: "nope" })).toBeNull();
  });

  it("falls back to the active run on the lane", () => {
    const s = samplePayload();
    s.event.current_run_ids = [];
    s.runs[1].status = "active";
    expect(resolveRunForLane(s, 1)).toBeNull();
    const r = resolveLane(s, 2);
    expect(r?.run.id).toBe(RUN_B1);
    expect(r?.source).toBe("active");
  });

  it("returns null without snapshot", () => {
    expect(resolveLane(null, 1)).toBeNull();
  });
});

describe("currentRunsByLane", () => {
  it("fills free lanes with current runs that have no lane", () => {
    const s = samplePayload();
    s.event.current_run_ids = [RUN_B1, RUN_C1];
    const map = currentRunsByLane(s);
    expect(map.get(2)?.id).toBe(RUN_B1);
    expect(map.get(1)?.id).toBe(RUN_C1);
  });

  it("ignores lanes beyond lane_count and unknown ids", () => {
    const s = samplePayload();
    s.discipline.lane_count = 1;
    s.event.current_run_ids = [RUN_B1, "unknown"];
    expect(currentRunsByLane(s).size).toBe(0);
  });

  it("orders current runs by lane for the banner", () => {
    const s = samplePayload();
    s.event.current_run_ids = [RUN_B1, RUN_A1];
    expect(currentRunsOrdered(s).map((r) => r.id)).toEqual([RUN_A1, RUN_B1]);
    expect(currentRunsOrdered(null)).toEqual([]);
  });
});

describe("laneGridClass", () => {
  it("lays out 1, 2 and 4 lanes", () => {
    expect(laneGridClass(1)).toContain("grid-cols-1");
    expect(laneGridClass(2)).toBe("grid-cols-2 grid-rows-1");
    expect(laneGridClass(4)).toBe("grid-cols-2 grid-rows-2");
  });
});
