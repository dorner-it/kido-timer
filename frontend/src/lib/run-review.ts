// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Organizer review of one run (PR 3, "Organisator-Prüfung").
 *
 * Workflow (Mirko/FBL): judge records an error → `pending` (red flag);
 * FBL checks with the team leader → confirms (`approved`) or withdraws
 * (`rejected`, stays visible crossed out). FV makes the run invalid and
 * needs an explicit confirmation. "Übernehmen" (bulk-approve) finalizes
 * the run: errors still pending count as confirmed.
 *
 * Counting mirrors `kido_core::penalty::PenaltyStatus::counts`: pending
 * and approved count (pending → result is *provisional*), rejected not.
 * Time = measured + 15 s × counting SZ (`kido_core::scoring`); legacy
 * `delta_ms` / `correction_ms` are never used.
 */

import type { Penalty, PenaltyKind, PenaltyStatus, Run } from './api';
import { PENALTY_KIND_SHORT, type CatalogueIndex, penaltyDescription, penaltyKind, SZ_DELTA_MS } from './penalties';
import { STATION_KINDS, STATION_KIND_LABELS, type StationKind } from './stations';

/** Review wording used in the organizer UI (FBL terms). */
export const REVIEW_STATUS_LABELS: Record<PenaltyStatus, string> = {
  pending: 'Offen',
  approved: 'Bestätigt',
  rejected: 'Zurückgenommen',
};

export function countsTowardsResult(status: PenaltyStatus): boolean {
  return status === 'pending' || status === 'approved';
}

type PenaltyLite = Pick<Penalty, 'status' | 'kind' | 'is_disqualification'>;

export interface RunSummary {
  total: number;
  pending: number;
  approved: number;
  rejected: number;
  /** SZ that count (pending + approved). */
  sz: number;
  /** FV that count (pending + approved). */
  fv: number;
  pendingSz: number;
  pendingFv: number;
  confirmedSz: number;
  confirmedFv: number;
  /** 15 s × counting SZ. */
  penaltyMs: number;
  /** No counting FV. */
  valid: boolean;
  /** Something is still pending → result may change. */
  provisional: boolean;
}

export function summarizeRun(penalties: readonly PenaltyLite[]): RunSummary {
  const s: RunSummary = {
    total: penalties.length,
    pending: 0,
    approved: 0,
    rejected: 0,
    sz: 0,
    fv: 0,
    pendingSz: 0,
    pendingFv: 0,
    confirmedSz: 0,
    confirmedFv: 0,
    penaltyMs: 0,
    valid: true,
    provisional: false,
  };
  for (const p of penalties) {
    const kind = penaltyKind(p);
    s[p.status] += 1;
    if (!countsTowardsResult(p.status)) continue;
    if (kind === 'sz') {
      s.sz += 1;
      if (p.status === 'pending') s.pendingSz += 1;
      else s.confirmedSz += 1;
    } else {
      s.fv += 1;
      if (p.status === 'pending') s.pendingFv += 1;
      else s.confirmedFv += 1;
    }
  }
  s.penaltyMs = s.sz * SZ_DELTA_MS;
  s.valid = s.fv === 0;
  s.provisional = s.pending > 0;
  return s;
}

/**
 * Short live summary, e.g. `"2 SZ (+30 s) · gültig"`, `"FV · ungültig"`,
 * `"fehlerfrei · gültig"`; `" · vorläufig"` is appended while errors are
 * pending.
 */
export function summaryText(s: RunSummary): string {
  let text: string;
  if (!s.valid) {
    text = 'FV · ungültig';
  } else if (s.sz === 0) {
    text = 'fehlerfrei · gültig';
  } else {
    text = `${s.sz} SZ (+${s.penaltyMs / 1000} s) · gültig`;
  }
  return s.provisional ? `${text} · vorläufig` : text;
}

/** Tone for the summary badge. */
export function summaryTone(s: RunSummary): 'invalid' | 'provisional' | 'penalty' | 'clean' {
  if (!s.valid) return 'invalid';
  if (s.provisional) return 'provisional';
  if (s.sz > 0) return 'penalty';
  return 'clean';
}

/**
 * Result time: measured (light barrier or hand-time mean, both stored in
 * `original_time_ms`) + 15 s × counting SZ. `null` when the run is
 * invalid or has no time yet.
 */
export function resultTimeMs(
  run: Pick<Run, 'original_time_ms'>,
  summary: Pick<RunSummary, 'valid' | 'penaltyMs'>,
): number | null {
  if (!summary.valid || run.original_time_ms == null) return null;
  return run.original_time_ms + summary.penaltyMs;
}

/** Penalties grouped per run id. */
export function penaltiesByRun<T extends Pick<Penalty, 'run_id'>>(
  penalties: readonly T[],
): Map<string, T[]> {
  const out = new Map<string, T[]>();
  for (const p of penalties) {
    const arr = out.get(p.run_id);
    if (arr) arr.push(p);
    else out.set(p.run_id, [p]);
  }
  return out;
}

export interface StationGroup<T> {
  station: StationKind | null;
  label: string;
  items: T[];
}

type GroupablePenalty = Pick<Penalty, 'station' | 'code' | 'created_at'>;

/** Station of a penalty; falls back to the catalogue entry of its code. */
export function penaltyStation(p: Pick<Penalty, 'station' | 'code'>, catalogue: CatalogueIndex): StationKind | null {
  if (p.station) return p.station;
  if (p.code) return catalogue.get(p.code)?.station ?? null;
  return null;
}

/**
 * Groups by station in canonical station order (unknown station last);
 * items sorted by `created_at` ascending (order of recording).
 */
export function groupByStation<T extends GroupablePenalty>(
  penalties: readonly T[],
  catalogue: CatalogueIndex,
): StationGroup<T>[] {
  const buckets = new Map<StationKind | null, T[]>();
  for (const p of penalties) {
    const st = penaltyStation(p, catalogue);
    const arr = buckets.get(st);
    if (arr) arr.push(p);
    else buckets.set(st, [p]);
  }
  const order: (StationKind | null)[] = [...STATION_KINDS, null];
  const out: StationGroup<T>[] = [];
  for (const st of order) {
    const items = buckets.get(st);
    if (!items || items.length === 0) continue;
    items.sort((a, b) => (a.created_at < b.created_at ? -1 : a.created_at > b.created_at ? 1 : 0));
    out.push({ station: st, label: st ? STATION_KIND_LABELS[st] : 'Ohne Station', items });
  }
  return out;
}

/** Only pending errors can be confirmed / withdrawn. */
export function canReview(p: Pick<Penalty, 'status'>): boolean {
  return p.status === 'pending';
}

/**
 * Compare two RFC 3339 instants with full sub-millisecond precision (the
 * backend writes nanoseconds; `Date.parse` alone truncates to ms).
 * Returns <0, 0, >0. `NaN` when either cannot be parsed.
 */
export function compareInstants(a: string, b: string): number {
  const split = (s: string): [number, string] => {
    const m = /\.(\d+)/.exec(s);
    const frac = (m?.[1] ?? '').padEnd(9, '0').slice(0, 9);
    const secMs = Date.parse(m ? s.replace(m[0], '') : s);
    return [secMs, frac];
  };
  const [sa, fa] = split(a);
  const [sb, fb] = split(b);
  if (Number.isNaN(sa) || Number.isNaN(sb)) return Number.NaN;
  if (sa !== sb) return sa - sb;
  return fa < fb ? -1 : fa > fb ? 1 : 0;
}

/**
 * "Rückgängig" allowed? Mirrors `kido_core::penalty::can_undo_review`:
 * decided (`approved` / `rejected`) and the run's "Übernehmen" has not
 * frozen it — `penalties_accepted_at` is null or the decision was made
 * strictly after it. Penalties approved by Übernehmen share its
 * timestamp and are therefore not undoable.
 */
export function canUndoReview(
  p: Pick<Penalty, 'status' | 'reviewed_at'>,
  run: Pick<Run, 'penalties_accepted_at'> | null | undefined,
): boolean {
  if (p.status === 'pending') return false;
  const accepted = run?.penalties_accepted_at ?? null;
  if (accepted == null) return true;
  if (p.reviewed_at == null) return false;
  return compareInstants(p.reviewed_at, accepted) > 0;
}

/** Confirming an FV needs an explicit confirmation dialog. */
export function needsFvConfirmation(p: Pick<Penalty, 'kind' | 'is_disqualification'>): boolean {
  return penaltyKind(p) === 'fv';
}

export interface TakeOverPlan<T> {
  /** Errors that "Übernehmen" will confirm. */
  pending: T[];
  pendingSz: number;
  pendingFv: number;
  /** FV among the pending ones → the dialog asks for an explicit tick. */
  needsFvAck: boolean;
  /** Summary after taking over (= the provisional summary, finalized). */
  after: RunSummary;
}

/** What "Übernehmen" (bulk-approve) will do for one run. */
export function takeOverPlan<T extends PenaltyLite>(penalties: readonly T[]): TakeOverPlan<T> {
  const pending = penalties.filter((p) => p.status === 'pending');
  const pendingFv = pending.filter((p) => penaltyKind(p) === 'fv').length;
  const after = summarizeRun(
    penalties.map((p) => (p.status === 'pending' ? { ...p, status: 'approved' as const } : p)),
  );
  return {
    pending,
    pendingSz: pending.length - pendingFv,
    pendingFv,
    needsFvAck: pendingFv > 0,
    after,
  };
}

/** Headline of the take-over dialog. */
export function takeOverHeadline(plan: Pick<TakeOverPlan<unknown>, 'pending' | 'pendingSz' | 'pendingFv'>): string {
  const n = plan.pending.length;
  if (n === 0) return 'Keine offenen Fehler – der Lauf wird so übernommen.';
  const parts: string[] = [];
  if (plan.pendingSz > 0) parts.push(`${plan.pendingSz} SZ`);
  if (plan.pendingFv > 0) parts.push(`${plan.pendingFv} FV`);
  return `${n} offene${n === 1 ? 'r' : ''} Fehler (${parts.join(', ')}) ${n === 1 ? 'wird' : 'werden'} bestätigt.`;
}

/** One line per penalty for confirmation dialogs. */
export function penaltyLine(
  p: Pick<Penalty, 'station' | 'code' | 'reason_code' | 'delta_ms' | 'kind' | 'is_disqualification'>,
  catalogue: CatalogueIndex,
): string {
  const kind: PenaltyKind = penaltyKind(p);
  const st = penaltyStation(p, catalogue);
  const station = st ? STATION_KIND_LABELS[st] : 'Ohne Station';
  return `${station}: ${PENALTY_KIND_SHORT[kind]} – ${penaltyDescription(p, catalogue)}`;
}

/** Replace one penalty in a list (optimistic update after approve/reject). */
export function replacePenalty<T extends Pick<Penalty, 'id'>>(list: readonly T[], updated: T): T[] {
  let found = false;
  const out = list.map((p) => {
    if (p.id !== updated.id) return p;
    found = true;
    return updated;
  });
  return found ? out : [...out, updated];
}

/** Locally mark all pending penalties of a run approved (after bulk-approve). */
export function markRunApproved<T extends Pick<Penalty, 'run_id' | 'status'>>(
  list: readonly T[],
  runId: string,
): T[] {
  return list.map((p) =>
    p.run_id === runId && p.status === 'pending' ? { ...p, status: 'approved' as const } : p,
  );
}
