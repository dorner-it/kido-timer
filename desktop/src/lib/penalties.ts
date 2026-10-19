/**
 * Penalty display logic (pure). Rules mirror `kido_core::scoring`:
 * counting errors = `pending` + `approved` (`rejected` stays visible,
 * crossed out); valid = no counting FV; time penalty = 15 s per
 * counting SZ; any counting `pending` error → result is provisional.
 */
import type { CatalogueItem, DisciplinePayload, Penalty, PenaltyStatus } from "./cloudTypes";
import { stationLabelDE } from "./i18n";

export const SZ_PENALTY_MS = 15_000;

export function countsTowardsResult(status: PenaltyStatus): boolean {
  return status === "pending" || status === "approved";
}

/** Penalties of one run, oldest first. */
export function penaltiesForRun(
  snapshot: DisciplinePayload | null,
  runId: string,
): Penalty[] {
  if (!snapshot) return [];
  return snapshot.approved_penalties
    .filter((p) => p.run_id === runId)
    .sort((a, b) => (a.created_at ?? "").localeCompare(b.created_at ?? ""));
}

export interface RunSummary {
  szCount: number;
  fvCount: number;
  /** 15 s × counting SZ. */
  penaltyMs: number;
  valid: boolean;
  provisional: boolean;
}

export function summarizeRun(penalties: Penalty[]): RunSummary {
  const counting = penalties.filter((p) => countsTowardsResult(p.status));
  const szCount = counting.filter((p) => p.kind === "sz").length;
  const fvCount = counting.filter((p) => p.kind === "fv").length;
  return {
    szCount,
    fvCount,
    penaltyMs: szCount * SZ_PENALTY_MS,
    valid: fvCount === 0,
    provisional: counting.some((p) => p.status === "pending"),
  };
}

/** "+30 s · gültig", "FV · ungültig", "fehlerfrei · gültig" (+ " · vorläufig"). */
export function formatRunSummary(s: RunSummary): string {
  const parts: string[] = [];
  if (!s.valid) {
    parts.push("FV", "ungültig");
  } else if (s.szCount > 0) {
    parts.push(`+${s.penaltyMs / 1000} s`, "gültig");
  } else {
    parts.push("fehlerfrei", "gültig");
  }
  if (s.provisional) parts.push("vorläufig");
  return parts.join(" · ");
}

export function catalogueIndex(items: CatalogueItem[]): Map<string, CatalogueItem> {
  return new Map(items.map((c) => [c.code, c]));
}

/** Text of an error: catalogue label, else legacy reason / code / kind. */
export function penaltyLabel(
  p: Penalty,
  catalogue: Map<string, CatalogueItem>,
): string {
  if (p.code) {
    const entry = catalogue.get(p.code);
    if (entry) return entry.label_de;
  }
  if (p.reason_code) return p.reason_code;
  if (p.code) return p.code;
  return p.kind === "fv" ? "Fehlversuch" : `Strafzeit (+${Math.round(p.delta_ms / 1000)} s)`;
}

export function penaltyStationLabel(p: Penalty): string {
  return p.station ? stationLabelDE(p.station) : "—";
}

export function penaltyStatusLabel(s: PenaltyStatus): string {
  switch (s) {
    case "pending":
      return "offen";
    case "approved":
      return "bestätigt";
    case "rejected":
      return "zurückgenommen";
  }
}
