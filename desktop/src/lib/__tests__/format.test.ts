import { describe, expect, it } from "vitest";
import { formatCorrection, formatTime } from "../format";
import { formatDecimal, formatResultTime, roundToCentis } from "../results";

describe("formatTime (lane cards)", () => {
  it("formats MM:SS.mmm", () => {
    expect(formatTime(3620, "confirmed")).toBe("00:03.620");
    expect(formatTime(125_400, "running")).toBe("02:05.400");
    expect(formatTime(65_535, "captured")).toBe("01:05.535");
  });
  it("shows dashes for inactive lanes", () => {
    expect(formatTime(1234, "inactive")).toBe("––:––.–––");
  });
  it("clamps negatives and the upper bound", () => {
    expect(formatTime(-5, "confirmed")).toBe("00:00.000");
    expect(formatTime(100 * 60_000, "confirmed")).toBe("99:59.999");
  });
});

describe("formatCorrection", () => {
  it("signs", () => {
    expect(formatCorrection(0)).toBe("±00:00.000");
    expect(formatCorrection(5000)).toBe("+00:05.000");
    expect(formatCorrection(-1500)).toBe("−00:01.500");
  });
});

describe("result time (1/100 s, German comma)", () => {
  it("rounds half up to 1/100 s like kido-core", () => {
    expect(roundToCentis(47_234)).toBe(47_230);
    expect(roundToCentis(47_235)).toBe(47_240);
    expect(roundToCentis(47_239)).toBe(47_240);
    expect(roundToCentis(0)).toBe(0);
  });
  it("formats seconds and minutes", () => {
    expect(formatResultTime(47_240)).toBe("47,24");
    expect(formatResultTime(47_235)).toBe("47,24");
    expect(formatResultTime(5_050)).toBe("5,05");
    expect(formatResultTime(65_300)).toBe("1:05,30");
    expect(formatResultTime(600_000)).toBe("10:00,00");
    expect(formatResultTime(59_995)).toBe("1:00,00");
  });
  it("placeholder for missing times", () => {
    expect(formatResultTime(null)).toBe("—");
    expect(formatResultTime(undefined)).toBe("—");
    expect(formatResultTime(Number.NaN)).toBe("—");
  });
  it("formatDecimal uses a comma", () => {
    expect(formatDecimal(12.5, 2)).toBe("12,50");
    expect(formatDecimal(0.88, 4)).toBe("0,8800");
  });
});
