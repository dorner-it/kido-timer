import { describe, expect, it } from "vitest";
import {
  effectiveSyncMode,
  isOverridden,
  loadSyncOverrides,
  planSyncSwitch,
  sanitizeOverrides,
  snapshotSyncMode,
  syncModeLabel,
  withOverride,
} from "../syncMode";
import { samplePayload } from "./fixtures";

describe("effectiveSyncMode", () => {
  it("web default without override", () => {
    expect(effectiveSyncMode("live")).toBe("live");
    expect(effectiveSyncMode("offline", null)).toBe("offline");
  });
  it("override wins", () => {
    expect(effectiveSyncMode("live", "offline")).toBe("offline");
    expect(effectiveSyncMode("offline", "live")).toBe("live");
  });
  it("isOverridden only when different from the default", () => {
    expect(isOverridden("live", "offline")).toBe(true);
    expect(isOverridden("live", "live")).toBe(false);
    expect(isOverridden("live", null)).toBe(false);
  });
});

describe("snapshotSyncMode", () => {
  it("uses the loaded discipline and its override", () => {
    const s = samplePayload(); // web default offline
    expect(snapshotSyncMode(s, {})).toBe("offline");
    expect(snapshotSyncMode(s, { disc: "live" })).toBe("live");
    expect(snapshotSyncMode(s, { other: "live" })).toBe("offline");
    expect(snapshotSyncMode(null, {})).toBeNull();
  });
});

describe("withOverride", () => {
  it("sets an override that differs from the default", () => {
    expect(withOverride({}, "d1", "offline", "live")).toEqual({ d1: "offline" });
  });
  it("choosing the web default removes the override", () => {
    expect(withOverride({ d1: "offline" }, "d1", "live", "live")).toEqual({});
  });
  it("null resets", () => {
    expect(withOverride({ d1: "offline", d2: "live" }, "d1", null, "live")).toEqual({
      d2: "live",
    });
  });
  it("does not mutate the input", () => {
    const input = { d1: "offline" as const };
    withOverride(input, "d1", null, "live");
    expect(input).toEqual({ d1: "offline" });
  });
});

describe("persistence helpers", () => {
  it("sanitizes stored data", () => {
    expect(sanitizeOverrides({ a: "live", b: "offline", c: "on_prem", d: 3 })).toEqual({
      a: "live",
      b: "offline",
    });
    expect(sanitizeOverrides(null)).toEqual({});
    expect(sanitizeOverrides("x")).toEqual({});
  });
  it("loading without storage yields no overrides", () => {
    expect(loadSyncOverrides()).toEqual({});
  });
  it("labels", () => {
    expect(syncModeLabel("live")).toBe("Live");
    expect(syncModeLabel("offline")).toBe("On-prem");
  });
});

describe("planSyncSwitch", () => {
  it("override to live on an offline web default switches the web (A2)", () => {
    const plan = planSyncSwitch({ webDefault: "offline", target: "live", paired: true, hasUnsynced: false });
    expect(plan.override).toBe("live");
    expect(plan.switchesWebToLive).toBe(true);
    expect(plan.startsLocalServer).toBe(false);
    expect(plan.notes.join(" ")).toContain("auf Live umgestellt");
  });

  it("live without pairing cannot switch the web", () => {
    const plan = planSyncSwitch({ webDefault: "offline", target: "live", paired: false, hasUnsynced: false });
    expect(plan.switchesWebToLive).toBe(false);
    expect(plan.notes.join(" ")).toContain("Nicht gekoppelt");
  });

  it("live with unsynced local changes warns about the decision", () => {
    const plan = planSyncSwitch({ webDefault: "live", target: "live", paired: true, hasUnsynced: true });
    expect(plan.override).toBeNull();
    expect(plan.switchesWebToLive).toBe(false);
    expect(plan.notes.join(" ")).toContain("hochladen oder verwerfen");
  });

  it("on-prem starts the local server; choosing the default clears the override", () => {
    const plan = planSyncSwitch({ webDefault: "live", target: "offline", paired: true, hasUnsynced: false });
    expect(plan.override).toBe("offline");
    expect(plan.startsLocalServer).toBe(true);
    expect(plan.switchesWebToLive).toBe(false);
    expect(plan.notes[0]).toContain("lokalen Server");
    const reset = planSyncSwitch({ webDefault: "offline", target: null, paired: true, hasUnsynced: false });
    expect(reset.override).toBeNull();
    expect(reset.startsLocalServer).toBe(true);
  });
});
