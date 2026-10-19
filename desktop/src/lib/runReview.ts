/**
 * Organizer review of one run in the desktop (on-prem) — same rules and
 * wording as the web review (PR 3, "Organisator-Prüfung"):
 *
 * judge records an error → `pending`; the organizer checks with the team
 * leader → "Bestätigen" (`approved`) or "Zurücknehmen" (`rejected`, stays
 * visible crossed out). An FV makes the run invalid and needs an explicit
 * confirmation. "Übernehmen" finalizes the run: errors still pending
 * count as confirmed.
 *
 * Counting mirrors `kido_core::scoring`: pending + approved count
 * (pending → result provisional), rejected not. Time = measured + 15 s ×
 * counting SZ.
 */
import type {
  CatalogueItem,
  DisciplinePayload,
  Penalty,
  PenaltyKind,
  PenaltyStatus,
  Run,
  StationKind,
} from "./cloudTypes";
import { stationLabelDE } from "./i18n";
import { allPenalties, countsTowardsResult, penaltyLabel, SZ_PENALTY_MS } from "./penalties";

export const REVIEW_STATUS_LABELS: Record<PenaltyStatus, string> = {
  pending: "Offen",
  approved: "Bestätigt",
  rejected: "Zurückgenommen",
};

export const PENALTY_KIND_SHORT: Record<PenaltyKind, string> = { sz: "SZ", fv: "FV" };

/** Canonical station order (sheet order; Löschangriff stations last). */
export const STATION_ORDER: StationKind[] = [
  "startlinie",
  "verteilerlinie",
  "strahlrohrlinie",
  "knotengestell",
  "angriffslinie",
  "podest",
];

type PenaltyLite = Pick<Penalty, "status" | "kind">;

export interface ReviewSummary {
  total: number;
  pending: number;
  approved: number;
  rejected: number;
  sz: number;
  fv: number;
  penaltyMs: number;
  valid: boolean;
  provisional: boolean;
}

export function reviewSummary(penalties: readonly PenaltyLite[]): ReviewSummary {
  const s: ReviewSummary = {
    total: penalties.length,
    pending: 0,
    approved: 0,
    rejected: 0,
    sz: 0,
    fv: 0,
    penaltyMs: 0,
    valid: true,
    provisional: false,
  };
  for (const p of penalties) {
    s[p.status] += 1;
    if (!countsTowardsResult(p.status)) continue;
    if (p.kind === "sz") s.sz += 1;
    else s.fv += 1;
  }
  s.penaltyMs = s.sz * SZ_PENALTY_MS;
  s.valid = s.fv === 0;
  s.provisional = s.pending > 0;
  return s;
}

/** `"2 SZ (+30 s) · gültig"`, `"FV · ungültig"`, `"fehlerfrei · gültig"` (+ `" · vorläufig"`). */
export function reviewSummaryText(s: ReviewSummary): string {
  let text: string;
  if (!s.valid) text = "FV · ungültig";
  else if (s.sz === 0) text = "fehlerfrei · gültig";
  else text = `${s.sz} SZ (+${s.penaltyMs / 1000} s) · gültig`;
  return s.provisional ? `${text} · vorläufig` : text;
}

export type SummaryTone = "invalid" | "provisional" | "penalty" | "clean";

export function summaryTone(s: ReviewSummary): SummaryTone {
  if (!s.valid) return "invalid";
  if (s.provisional) return "provisional";
  if (s.sz > 0) return "penalty";
  return "clean";
}

/** Measured + 15 s × counting SZ; `null` when invalid or no time yet. */
export function resultTimeMs(
  run: Pick<Run, "original_time_ms">,
  s: Pick<ReviewSummary, "valid" | "penaltyMs">,
): number | null {
  if (!s.valid || run.original_time_ms == null) return null;
  return run.original_time_ms + s.penaltyMs;
}

export function penaltyStation(
  p: Pick<Penalty, "station" | "code">,
  catalogue: Map<string, CatalogueItem>,
): StationKind | null {
  if (p.station) return p.station;
  if (p.code) return catalogue.get(p.code)?.station ?? null;
  return null;
}

export interface StationGroup<T> {
  station: StationKind | null;
  label: string;
  items: T[];
}

/** Grouped by station (canonical order, unknown last), oldest first. */
export function groupByStation<T extends Pick<Penalty, "station" | "code" | "created_at">>(
  penalties: readonly T[],
  catalogue: Map<string, CatalogueItem>,
): StationGroup<T>[] {
  const buckets = new Map<StationKind | null, T[]>();
  for (const p of penalties) {
    const st = penaltyStation(p, catalogue);
    const arr = buckets.get(st);
    if (arr) arr.push(p);
    else buckets.set(st, [p]);
  }
  const out: StationGroup<T>[] = [];
  for (const st of [...STATION_ORDER, null]) {
    const items = buckets.get(st);
    if (!items || items.length === 0) continue;
    items.sort((a, b) => (a.created_at ?? "").localeCompare(b.created_at ?? ""));
    out.push({ station: st, label: st ? stationLabelDE(st) : "Ohne Station", items });
  }
  return out;
}

/** Only pending errors can be confirmed / withdrawn. */
export function canReview(p: Pick<Penalty, "status">): boolean {
  return p.status === "pending";
}

/** Confirming an FV needs an explicit confirmation dialog. */
export function needsFvConfirmation(p: Pick<Penalty, "kind">): boolean {
  return p.kind === "fv";
}

export interface TakeOverPlan<T> {
  pending: T[];
  pendingSz: number;
  pendingFv: number;
  needsFvAck: boolean;
  after: ReviewSummary;
}

export function takeOverPlan<T extends PenaltyLite>(penalties: readonly T[]): TakeOverPlan<T> {
  const pending = penalties.filter((p) => p.status === "pending");
  const pendingFv = pending.filter((p) => p.kind === "fv").length;
  const after = reviewSummary(
    penalties.map((p) => (p.status === "pending" ? { ...p, status: "approved" as const } : p)),
  );
  return {
    pending,
    pendingSz: pending.length - pendingFv,
    pendingFv,
    needsFvAck: pendingFv > 0,
    after,
  };
}

export function takeOverHeadline(
  plan: Pick<TakeOverPlan<unknown>, "pending" | "pendingSz" | "pendingFv">,
): string {
  const n = plan.pending.length;
  if (n === 0) return "Keine offenen Fehler – der Lauf wird so übernommen.";
  const parts: string[] = [];
  if (plan.pendingSz > 0) parts.push(`${plan.pendingSz} SZ`);
  if (plan.pendingFv > 0) parts.push(`${plan.pendingFv} FV`);
  return `${n} offene${n === 1 ? "r" : ""} Fehler (${parts.join(", ")}) ${
    n === 1 ? "wird" : "werden"
  } bestätigt.`;
}

/** `"Knotengestell: SZ – 1 Knoten falsch"`. */
export function penaltyLine(p: Penalty, catalogue: Map<string, CatalogueItem>): string {
  const st = penaltyStation(p, catalogue);
  const station = st ? stationLabelDE(st) : "Ohne Station";
  return `${station}: ${PENALTY_KIND_SHORT[p.kind]} – ${penaltyLabel(p, catalogue)}`;
}

/** Footer text next to "Übernehmen". */
export function takeOverFooter(s: ReviewSummary): string {
  return s.pending > 0
    ? `${s.pending} offen – zählen beim Übernehmen als bestätigt`
    : "Keine offenen Fehler";
}

/** `"Lauf 3 · FF Alpha"`. */
export function runTitle(snapshot: DisciplinePayload, run: Run): string {
  const team = snapshot.teams.find((t) => t.id === run.team_id);
  return `Lauf ${run.run_number} · ${team?.name ?? "Unbekannte Mannschaft"}`;
}

/** Runs with errors still pending (for the "Offene Prüfungen" hint). */
export function runsWithPendingErrors(snapshot: DisciplinePayload | null): string[] {
  const ids = new Set<string>();
  for (const p of allPenalties(snapshot)) if (p.status === "pending") ids.add(p.run_id);
  return Array.from(ids);
}
