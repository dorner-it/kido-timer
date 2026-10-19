import { describe, expect, it } from "vitest";
import { modeLabelDE, stationLabelDE, wgLabelDE } from "../i18n";

describe("stationLabelDE", () => {
  it("canonical keys", () => {
    expect(stationLabelDE("startlinie")).toBe("Startlinie/Zeitnehmer");
    expect(stationLabelDE("angriffslinie")).toBe("Angriffslinie");
    expect(stationLabelDE("podest")).toBe("Podest/Wasserbehälter");
  });
  it("legacy aliases map to the same label", () => {
    expect(stationLabelDE("knoten")).toBe(stationLabelDE("knotengestell"));
    expect(stationLabelDE("kuebelspritze")).toBe("Angriffslinie");
    expect(stationLabelDE("start")).toBe("Startlinie/Zeitnehmer");
  });
});

describe("modeLabelDE", () => {
  it("only knows the v4 modes", () => {
    expect(modeLabelDE("gruppenstaffette", 1)).toBe("Gruppenstafette (1 Bahn)");
    expect(modeLabelDE("gruppenstaffette", 2)).toBe("Gruppenstafette (2 Bahnen)");
    expect(modeLabelDE("loeschangriff")).toBe("Löschangriff");
    expect(modeLabelDE("single_lane")).toBe("single_lane");
  });
  it("wg labels", () => {
    expect(wgLabelDE("wg3")).toBe("WG 3");
  });
});
