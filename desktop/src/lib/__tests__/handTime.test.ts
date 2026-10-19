import { describe, expect, it } from "vitest";
import {
  formatTimeInput,
  handTimeRawText,
  isHandTime,
  parseTimeInput,
  validateHandTimes,
} from "../handTime";

describe("parseTimeInput", () => {
  it.each([
    ["47,23", 47_230],
    ["47.23", 47_230],
    ["0:47,23", 47_230],
    ["1:02,50", 62_500],
    ["62,5", 62_500],
    ["47", 47_000],
    ["47,235", 47_235],
    [" 47 , 23 ", 47_230],
  ])("%s → %i ms", (raw, ms) => {
    expect(parseTimeInput(raw)).toEqual({ ok: true, ms });
  });

  it("rejects empty, garbage, zero and seconds ≥ 60 with minutes", () => {
    expect(parseTimeInput("").ok).toBe(false);
    expect(parseTimeInput(null).ok).toBe(false);
    expect(parseTimeInput("abc").ok).toBe(false);
    expect(parseTimeInput("47,2345").ok).toBe(false);
    expect(parseTimeInput("0").ok).toBe(false);
    expect(parseTimeInput("0,00").ok).toBe(false);
    const r = parseTimeInput("1:75,00");
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.error).toContain("0 und 59");
    expect(parseTimeInput("99999999999").ok).toBe(false);
  });
});

describe("formatTimeInput", () => {
  it("formats with comma and optional minutes", () => {
    expect(formatTimeInput(47_230)).toBe("47,23");
    expect(formatTimeInput(62_500)).toBe("1:02,50");
    expect(formatTimeInput(47_235)).toBe("47,235");
    expect(formatTimeInput(5)).toBe("0,005");
    expect(formatTimeInput(null)).toBe("");
    expect(formatTimeInput(Number.NaN)).toBe("");
  });

  it("round-trips through parse", () => {
    for (const ms of [1_000, 47_230, 62_500, 47_235, 599_990]) {
      const p = parseTimeInput(formatTimeInput(ms));
      expect(p).toEqual({ ok: true, ms });
    }
  });
});

describe("validateHandTimes", () => {
  it("both valid → ok without warning", () => {
    expect(validateHandTimes("47,23", "47,25")).toEqual({
      ok: true,
      t1: 47_230,
      t2: 47_250,
      spreadMs: 20,
      warning: null,
    });
  });

  it("warns above 1 s spread (does not block)", () => {
    const v = validateHandTimes("47,00", "48,50");
    expect(v.ok).toBe(true);
    if (v.ok) expect(v.warning).toContain("1,50");
  });

  it("collects field errors", () => {
    const v = validateHandTimes("", "x");
    expect(v.ok).toBe(false);
    if (!v.ok) {
      expect(v.errors.t1).toBeTruthy();
      expect(v.errors.t2).toBeTruthy();
    }
    const one = validateHandTimes("47,23", "");
    expect(one.ok).toBe(false);
    if (!one.ok) {
      expect(one.errors.t1).toBeUndefined();
      expect(one.errors.t2).toBeTruthy();
    }
  });
});

describe("hand time display", () => {
  it("raw text only for handstopp", () => {
    expect(
      handTimeRawText({ time_source: "handstopp", hand_time_1_ms: 47_230, hand_time_2_ms: 47_250 }),
    ).toBe("47,23 / 47,25");
    expect(
      handTimeRawText({ time_source: "handstopp", hand_time_1_ms: null, hand_time_2_ms: 47_250 }),
    ).toBe("— / 47,25");
    expect(
      handTimeRawText({ time_source: "lichtschranke", hand_time_1_ms: 1, hand_time_2_ms: 2 }),
    ).toBeNull();
    expect(isHandTime({ time_source: "handstopp" })).toBe(true);
    expect(isHandTime({ time_source: "lichtschranke" })).toBe(false);
  });
});
