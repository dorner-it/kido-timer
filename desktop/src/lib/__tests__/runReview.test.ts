import { describe, expect, it } from "vitest";
import type { CatalogueItem } from "../cloudTypes";
import { catalogueIndex } from "../penalties";
import {
  canReview,
  groupByStation,
  needsFvConfirmation,
  penaltyLine,
  resultTimeMs,
  reviewSummary,
  reviewSummaryText,
  REVIEW_STATUS_LABELS,
  runsWithPendingErrors,
  runTitle,
  summaryTone,
  takeOverFooter,
  takeOverHeadline,
  takeOverPlan,
} from "../runReview";
import { penalty, RUN_A1, RUN_B1, samplePayload } from "./fixtures";

const CATALOGUE: CatalogueItem[] = [
  {
    code: "knotengestell.ein_knoten_falsch",
    station: "knotengestell",
    station_label: "Knotengestell",
    kind: "sz",
    label_de: "1 Knoten falsch",
  },
  {
    code: "knotengestell.mehrere_knoten_falsch",
    station: "knotengestell",
    station_label: "Knotengestell",
    kind: "fv",
    label_de: "Mehrere Knoten falsch",
  },
  {
    code: "startlinie.wk_startet_zu_frueh",
    station: "startlinie",
    station_label: "Startlinie/Zeitnehmer",
    kind: "sz",
    label_de: "Ein WK startet vor dem Abschlagen",
  },
];
const cat = catalogueIndex(CATALOGUE);

describe("wording (same as web PR 3)", () => {
  it("status labels", () => {
    expect(REVIEW_STATUS_LABELS).toEqual({
      pending: "Offen",
      approved: "Bestätigt",
      rejected: "Zurückgenommen",
    });
  });

  it("summary texts", () => {
    expect(reviewSummaryText(reviewSummary([]))).toBe("fehlerfrei · gültig");
    expect(
      reviewSummaryText(
        reviewSummary([penalty("a", "sz", "approved"), penalty("b", "sz", "approved")]),
      ),
    ).toBe("2 SZ (+30 s) · gültig");
    expect(reviewSummaryText(reviewSummary([penalty("a", "sz", "pending")]))).toBe(
      "1 SZ (+15 s) · gültig · vorläufig",
    );
    expect(
      reviewSummaryText(reviewSummary([penalty("a", "fv", "approved"), penalty("b", "sz", "pending")])),
    ).toBe("FV · ungültig · vorläufig");
  });
});

describe("reviewSummary", () => {
  it("rejected errors do not count but stay in the totals", () => {
    const s = reviewSummary([
      penalty("a", "sz", "rejected"),
      penalty("b", "fv", "rejected"),
      penalty("c", "sz", "approved"),
    ]);
    expect(s.total).toBe(3);
    expect(s.rejected).toBe(2);
    expect(s.sz).toBe(1);
    expect(s.fv).toBe(0);
    expect(s.valid).toBe(true);
    expect(s.penaltyMs).toBe(15_000);
    expect(s.provisional).toBe(false);
  });

  it("tones", () => {
    expect(summaryTone(reviewSummary([]))).toBe("clean");
    expect(summaryTone(reviewSummary([penalty("a", "sz", "approved")]))).toBe("penalty");
    expect(summaryTone(reviewSummary([penalty("a", "sz", "pending")]))).toBe("provisional");
    expect(summaryTone(reviewSummary([penalty("a", "fv", "pending")]))).toBe("invalid");
  });

  it("result time = measured + 15 s × counting SZ, null when invalid / no time", () => {
    const s = reviewSummary([penalty("a", "sz", "approved")]);
    expect(resultTimeMs({ original_time_ms: 47_240 }, s)).toBe(62_240);
    expect(resultTimeMs({ original_time_ms: null }, s)).toBeNull();
    expect(resultTimeMs({ original_time_ms: 47_240 }, reviewSummary([penalty("f", "fv", "approved")]))).toBeNull();
  });
});

describe("groupByStation", () => {
  it("canonical station order, oldest first, unknown last", () => {
    const list = [
      penalty("k2", "sz", "pending", { station: "knotengestell", created_at: "2027-06-12T10:00:09Z" }),
      penalty("s1", "sz", "pending", { station: "startlinie", created_at: "2027-06-12T10:00:05Z" }),
      penalty("k1", "fv", "pending", { station: "knotengestell", created_at: "2027-06-12T10:00:01Z" }),
      penalty("x", "sz", "approved", { station: null, code: null }),
      penalty("c", "sz", "approved", { station: null, code: "knotengestell.ein_knoten_falsch" }),
    ];
    const g = groupByStation(list, cat);
    expect(g.map((x) => x.label)).toEqual(["Startlinie/Zeitnehmer", "Knotengestell", "Ohne Station"]);
    // c (catalogue → Knotengestell) was created at …:01 like k1.
    expect(g[1]!.items.map((p) => p.id)).toEqual(["k1", "c", "k2"]);
    expect(g[2]!.items.map((p) => p.id)).toEqual(["x"]);
  });
});

describe("review actions", () => {
  it("only pending can be reviewed; FV needs confirmation", () => {
    expect(canReview(penalty("a", "sz", "pending"))).toBe(true);
    expect(canReview(penalty("a", "sz", "approved"))).toBe(false);
    expect(canReview(penalty("a", "sz", "rejected"))).toBe(false);
    expect(needsFvConfirmation(penalty("a", "fv", "pending"))).toBe(true);
    expect(needsFvConfirmation(penalty("a", "sz", "pending"))).toBe(false);
  });

  it("take-over plan and headline", () => {
    const list = [
      penalty("a", "sz", "pending"),
      penalty("b", "fv", "pending"),
      penalty("c", "sz", "rejected"),
      penalty("d", "sz", "approved"),
    ];
    const plan = takeOverPlan(list);
    expect(plan.pending.map((p) => p.id)).toEqual(["a", "b"]);
    expect(plan.pendingSz).toBe(1);
    expect(plan.pendingFv).toBe(1);
    expect(plan.needsFvAck).toBe(true);
    expect(reviewSummaryText(plan.after)).toBe("FV · ungültig");
    expect(takeOverHeadline(plan)).toBe("2 offene Fehler (1 SZ, 1 FV) werden bestätigt.");
    expect(takeOverHeadline(takeOverPlan([penalty("a", "sz", "pending")]))).toBe(
      "1 offener Fehler (1 SZ) wird bestätigt.",
    );
    expect(takeOverHeadline(takeOverPlan([]))).toBe(
      "Keine offenen Fehler – der Lauf wird so übernommen.",
    );
    expect(takeOverPlan([penalty("a", "sz", "pending")]).needsFvAck).toBe(false);
  });

  it("footer text", () => {
    expect(takeOverFooter(reviewSummary([penalty("a", "sz", "pending")]))).toBe(
      "1 offen – zählen beim Übernehmen als bestätigt",
    );
    expect(takeOverFooter(reviewSummary([]))).toBe("Keine offenen Fehler");
  });

  it("penalty line uses station and catalogue text", () => {
    expect(
      penaltyLine(
        penalty("a", "fv", "pending", {
          station: "knotengestell",
          code: "knotengestell.mehrere_knoten_falsch",
        }),
        cat,
      ),
    ).toBe("Knotengestell: FV – Mehrere Knoten falsch");
    expect(penaltyLine(penalty("a", "sz", "pending", { reason_code: "alt" }), cat)).toBe(
      "Ohne Station: SZ – alt",
    );
  });
});

describe("snapshot helpers", () => {
  it("run title and runs with pending errors", () => {
    const s = samplePayload();
    expect(runTitle(s, s.runs[0]!)).toBe("Lauf 1 · FF Alpha");
    s.penalties = [
      penalty("a", "sz", "pending", { run_id: RUN_B1 }),
      penalty("b", "sz", "approved", { run_id: RUN_A1 }),
    ];
    expect(runsWithPendingErrors(s)).toEqual([RUN_B1]);
    expect(runsWithPendingErrors(null)).toEqual([]);
  });
});
