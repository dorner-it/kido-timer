import { describe, expect, it } from "vitest";
import {
  allPenalties,
  catalogueIndex,
  countsTowardsResult,
  formatRunSummary,
  penaltiesForRun,
  penaltyLabel,
  penaltyStationLabel,
  penaltyStatusLabel,
  summarizeRun,
} from "../penalties";
import type { CatalogueItem, DisciplinePayload } from "../cloudTypes";
import { penalty, RUN_A1, RUN_B1, samplePayload } from "./fixtures";

describe("summarizeRun", () => {
  it("no errors → fehlerfrei · gültig", () => {
    const s = summarizeRun([]);
    expect(s).toEqual({ szCount: 0, fvCount: 0, penaltyMs: 0, valid: true, provisional: false });
    expect(formatRunSummary(s)).toBe("fehlerfrei · gültig");
  });

  it("two SZ → +30 s · gültig", () => {
    const s = summarizeRun([penalty("a", "sz", "approved"), penalty("b", "sz", "approved")]);
    expect(s.penaltyMs).toBe(30_000);
    expect(formatRunSummary(s)).toBe("+30 s · gültig");
  });

  it("FV → FV · ungültig (SZ irrelevant)", () => {
    const s = summarizeRun([penalty("a", "sz", "approved"), penalty("b", "fv", "approved")]);
    expect(s.valid).toBe(false);
    expect(formatRunSummary(s)).toBe("FV · ungültig");
  });

  it("rejected errors do not count", () => {
    const s = summarizeRun([penalty("a", "fv", "rejected"), penalty("b", "sz", "rejected")]);
    expect(s.valid).toBe(true);
    expect(s.szCount).toBe(0);
    expect(formatRunSummary(s)).toBe("fehlerfrei · gültig");
  });

  it("pending errors count and make the result provisional", () => {
    const s = summarizeRun([penalty("a", "sz", "pending")]);
    expect(s.provisional).toBe(true);
    expect(formatRunSummary(s)).toBe("+15 s · gültig · vorläufig");
    const fv = summarizeRun([penalty("a", "fv", "pending")]);
    expect(formatRunSummary(fv)).toBe("FV · ungültig · vorläufig");
  });

  it("a rejected pending-free list is not provisional", () => {
    expect(summarizeRun([penalty("a", "sz", "rejected")]).provisional).toBe(false);
  });

  it("countsTowardsResult", () => {
    expect(countsTowardsResult("pending")).toBe(true);
    expect(countsTowardsResult("approved")).toBe(true);
    expect(countsTowardsResult("rejected")).toBe(false);
  });
});

describe("penaltiesForRun", () => {
  it("prefers `penalties` (all statuses) over approved_penalties", () => {
    const s = samplePayload();
    s.approved_penalties = [penalty("p1", "sz", "approved")];
    s.penalties = [penalty("p1", "sz", "approved"), penalty("p2", "sz", "pending")];
    expect(allPenalties(s).map((p) => p.id)).toEqual(["p1", "p2"]);
    expect(penaltiesForRun(s, RUN_A1)).toHaveLength(2);
  });

  it("falls back to approved_penalties when penalties is missing", () => {
    const s = samplePayload() as Partial<DisciplinePayload> & DisciplinePayload;
    s.approved_penalties = [penalty("p1", "sz", "approved")];
    delete (s as Partial<DisciplinePayload>).penalties;
    expect(allPenalties(s).map((p) => p.id)).toEqual(["p1"]);
    expect(allPenalties(null)).toEqual([]);
  });

  it("filters by run and sorts oldest first", () => {
    const s = samplePayload();
    s.penalties = [
      penalty("p2", "sz", "approved", { created_at: "2027-06-12T10:00:05Z" }),
      penalty("p1", "sz", "approved", { created_at: "2027-06-12T10:00:01Z" }),
      penalty("p3", "fv", "approved", { run_id: RUN_B1 }),
    ];
    expect(penaltiesForRun(s, RUN_A1).map((p) => p.id)).toEqual(["p1", "p2"]);
    expect(penaltiesForRun(null, RUN_A1)).toEqual([]);
  });
});

describe("labels", () => {
  const cat: CatalogueItem[] = [
    {
      code: "knotengestell.ein_knoten_falsch",
      station: "knotengestell",
      station_label: "Knotengestell",
      kind: "sz",
      label_de: "1 Knoten falsch",
    },
  ];
  const idx = catalogueIndex(cat);

  it("uses the catalogue label", () => {
    const p = penalty("a", "sz", "approved", {
      code: "knotengestell.ein_knoten_falsch",
      station: "knotengestell",
    });
    expect(penaltyLabel(p, idx)).toBe("1 Knoten falsch");
    expect(penaltyStationLabel(p)).toBe("Knotengestell");
  });

  it("falls back for legacy rows", () => {
    expect(penaltyLabel(penalty("a", "sz", "approved", { reason_code: "frei" }), idx)).toBe("frei");
    expect(penaltyLabel(penalty("a", "sz", "approved", { code: "unbekannt.x" }), idx)).toBe(
      "unbekannt.x",
    );
    expect(penaltyLabel(penalty("a", "fv", "approved"), idx)).toBe("Fehlversuch");
    expect(penaltyLabel(penalty("a", "sz", "approved", { delta_ms: 5000 }), idx)).toBe(
      "Strafzeit (+5 s)",
    );
    expect(penaltyStationLabel(penalty("a", "sz", "approved"))).toBe("—");
  });

  it("status labels", () => {
    expect(penaltyStatusLabel("pending")).toBe("offen");
    expect(penaltyStatusLabel("approved")).toBe("bestätigt");
    expect(penaltyStatusLabel("rejected")).toBe("zurückgenommen");
  });
});
