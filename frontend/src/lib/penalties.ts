// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Display helpers for penalties (SZ / FV) and the error catalogue.
 *
 * - SZ = Strafzeit, fixed +15 s, run stays valid.
 * - FV = Fehlversuch, run invalid.
 */

import type { CatalogueEntry, Penalty, PenaltyKind } from './api';
import { formatTimeMs } from './format';
import type { StationKind } from './stations';

export const SZ_DELTA_MS = 15_000;

export const PENALTY_KINDS: readonly PenaltyKind[] = ['sz', 'fv'];

export const PENALTY_KIND_SHORT: Record<PenaltyKind, string> = {
  sz: 'SZ',
  fv: 'FV',
};

export const PENALTY_KIND_LABELS: Record<PenaltyKind, string> = {
  sz: 'SZ (+15 s)',
  fv: 'FV (ungültig)',
};

export const PENALTY_KIND_TITLES: Record<PenaltyKind, string> = {
  sz: 'Strafzeit: +15 s, Lauf bleibt gültig',
  fv: 'Fehlversuch: Lauf ungültig',
};

export function isPenaltyKind(value: unknown): value is PenaltyKind {
  return value === 'sz' || value === 'fv';
}

/** Read-time migration: missing kind → `is_disqualification ? fv : sz`. */
export function penaltyKind(p: { kind?: unknown; is_disqualification?: boolean | null }): PenaltyKind {
  if (isPenaltyKind(p.kind)) return p.kind;
  return p.is_disqualification ? 'fv' : 'sz';
}

/** Tailwind classes for the kind badge. */
export function penaltyKindBadgeClasses(kind: PenaltyKind): string {
  return kind === 'fv'
    ? 'bg-status-unknown/15 text-status-unknown'
    : 'bg-status-armed/15 text-status-armed';
}

export type CatalogueIndex = ReadonlyMap<string, CatalogueEntry>;

export function indexCatalogue(entries: readonly CatalogueEntry[] | null | undefined): CatalogueIndex {
  return new Map((entries ?? []).map((e) => [e.code, e]));
}

/** Entries of one station, in catalogue (= sheet) order. */
export function catalogueForStation(
  entries: readonly CatalogueEntry[] | null | undefined,
  station: StationKind | null,
): CatalogueEntry[] {
  if (!station) return [];
  return (entries ?? []).filter((e) => e.station === station);
}

/** `+15.00` / `-2.50` / `+1:00.00`. */
export function formatDelta(deltaMs: number): string {
  return `${deltaMs >= 0 ? '+' : '-'}${formatTimeMs(Math.abs(deltaMs))}`;
}

type PenaltyLike = Pick<Penalty, 'code' | 'reason_code' | 'delta_ms'> & {
  kind?: unknown;
  is_disqualification?: boolean | null;
};

/**
 * Human-readable description of a penalty:
 * 1. catalogue label when `code` is known,
 * 2. the raw `code` when the catalogue doesn't know it (yet),
 * 3. legacy `reason_code`,
 * 4. the formatted delta.
 */
export function penaltyDescription(p: PenaltyLike, catalogue: CatalogueIndex): string {
  if (p.code) {
    return catalogue.get(p.code)?.label_de ?? p.code;
  }
  if (p.reason_code) return p.reason_code;
  if (penaltyKind(p) === 'fv') return PENALTY_KIND_LABELS.fv;
  return formatDelta(p.delta_ms);
}

/**
 * Value for the Δ column: FV has no time; SZ from the catalogue era is
 * always +15 s; legacy rows show their stored delta.
 */
export function penaltyDeltaText(p: PenaltyLike): string {
  if (penaltyKind(p) === 'fv') return '—';
  return formatDelta(p.delta_ms);
}

/** One line for the "Lauf abschließen" confirmation. */
export function penaltySummaryLine(
  p: PenaltyLike & { station: StationKind | null },
  catalogue: CatalogueIndex,
  stationLabel: (s: StationKind | null) => string,
): string {
  const kind = penaltyKind(p);
  const desc = penaltyDescription(p, catalogue);
  const delta = kind === 'sz' ? ` ${penaltyDeltaText(p)}` : '';
  return `• ${stationLabel(p.station)}: ${PENALTY_KIND_SHORT[kind]}${delta} – ${desc}`;
}
