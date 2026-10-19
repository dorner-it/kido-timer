// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Results of one discipline (`GET /api/events/:id/disciplines/:did/results`,
 * shape in docs/api.md § Results): wire types, a lenient zod read schema
 * and the pure row builders used by the screen and the print view.
 *
 * All scoring happens in the backend (`kido_core::scoring`); this module
 * only orders and formats what it returns.
 */

import { z } from 'zod';

import type { TimerMode, TimeSource, Wertungsgruppe } from './api';
import { NO_TIME, formatDecimalDE, formatRaceTime } from './time-format';
import { WERTUNGSGRUPPE_DESCRIPTIONS, WERTUNGSGRUPPE_LABELS, WERTUNGSGRUPPEN } from './wg';

// ===== wire types =====

export type RankStatus = 'ranked' | 'all_invalid' | 'not_started';
export type InvalidReason = 'fehlversuch' | 'no_time';

export interface ResultRun {
  run_id: string;
  run_number: number;
  lane: number | null;
  time_source: TimeSource;
  hand_time_1_ms: number | null;
  hand_time_2_ms: number | null;
  measured_ms: number | null;
  sz_count: number;
  fv_count: number;
  total_ms: number | null;
  valid: boolean;
  invalid_reason: InvalidReason | null;
  provisional: boolean;
  is_best: boolean;
}

export interface ResultRow {
  team_id: string;
  team_name: string;
  start_number: number;
  place: number | null;
  status: RankStatus;
  best_run_id: string | null;
  best_total_ms: number | null;
  provisional: boolean;
  runs: ResultRun[];
}

export interface WgRanking {
  wg: Wertungsgruppe;
  wg_label: string;
  wg_label_de: string;
  rows: ResultRow[];
}

export interface TagessiegerResult {
  place: number;
  team_id: string;
  team_name: string;
  start_number: number;
  wg: Wertungsgruppe;
  wg_label: string;
  run_id: string;
  run_number: number | null;
  total_ms: number;
  ages: number[];
  avg_age: number;
  factor: number;
  gw_ms: number;
  lineup_fallback: boolean;
  provisional: boolean;
}

export interface TeamRef {
  team_id: string;
  team_name: string;
  start_number: number;
}

export interface DisciplineResults {
  event: { id: string; name: string; date: string; location: string | null };
  discipline: { id: string; name: string; mode: TimerMode };
  generated_at: string;
  provisional: boolean;
  wg_rankings: WgRanking[];
  tagessieger: TagessiegerResult[];
  missing_wg: TeamRef[];
}

// ===== read schema =====

const wgSchema = z.enum(['wg1', 'wg2', 'wg3', 'wg4', 'wg5']);
const nullableInt = z.number().nullable().catch(null);
const bool = z.boolean().catch(false);

const resultRunSchema = z.object({
  run_id: z.string(),
  run_number: z.number().int().catch(0),
  lane: nullableInt.optional().transform((v) => v ?? null),
  time_source: z.enum(['lichtschranke', 'handstopp']).catch('lichtschranke'),
  hand_time_1_ms: nullableInt.optional().transform((v) => v ?? null),
  hand_time_2_ms: nullableInt.optional().transform((v) => v ?? null),
  measured_ms: nullableInt.optional().transform((v) => v ?? null),
  sz_count: z.number().int().catch(0),
  fv_count: z.number().int().catch(0),
  total_ms: nullableInt,
  valid: bool,
  invalid_reason: z.enum(['fehlversuch', 'no_time']).nullable().catch(null),
  provisional: bool,
  is_best: bool,
});

const resultRowSchema = z.object({
  team_id: z.string(),
  team_name: z.string().catch('Unbekannte Mannschaft'),
  start_number: z.number().int().catch(0),
  place: nullableInt,
  status: z.enum(['ranked', 'all_invalid', 'not_started']),
  best_run_id: z.string().nullable().catch(null),
  best_total_ms: nullableInt,
  provisional: bool,
  runs: z.array(resultRunSchema).catch([]),
});

const wgRankingSchema = z
  .object({
    wg: wgSchema,
    wg_label: z.string().optional(),
    wg_label_de: z.string().optional(),
    rows: z.array(resultRowSchema).catch([]),
  })
  .transform((w) => ({
    ...w,
    wg_label: w.wg_label ?? WERTUNGSGRUPPE_LABELS[w.wg],
    wg_label_de: w.wg_label_de ?? WERTUNGSGRUPPE_DESCRIPTIONS[w.wg],
  }));

const tagessiegerSchema = z
  .object({
    place: z.number().int(),
    team_id: z.string(),
    team_name: z.string().catch('Unbekannte Mannschaft'),
    start_number: z.number().int().catch(0),
    wg: wgSchema,
    wg_label: z.string().optional(),
    run_id: z.string(),
    run_number: nullableInt.optional().transform((v) => v ?? null),
    total_ms: z.number(),
    ages: z.array(z.number()).catch([]),
    avg_age: z.number(),
    factor: z.number(),
    gw_ms: z.number(),
    lineup_fallback: bool,
    provisional: bool,
  })
  .transform((t) => ({ ...t, wg_label: t.wg_label ?? WERTUNGSGRUPPE_LABELS[t.wg] }));

export const disciplineResultsSchema = z.object({
  event: z.object({
    id: z.string(),
    name: z.string(),
    date: z.string(),
    location: z.string().nullable().catch(null),
  }),
  discipline: z.object({
    id: z.string(),
    name: z.string(),
    mode: z.enum(['gruppenstaffette', 'loeschangriff']),
  }),
  generated_at: z.string(),
  provisional: bool,
  wg_rankings: z.array(wgRankingSchema).catch([]),
  tagessieger: z.array(tagessiegerSchema).catch([]),
  missing_wg: z
    .array(
      z.object({
        team_id: z.string(),
        team_name: z.string().catch('Unbekannte Mannschaft'),
        start_number: z.number().int().catch(0),
      }),
    )
    .catch([]),
});

/** Parse an API response; throws a `ZodError` on a malformed header. */
export function normalizeResults(raw: unknown): DisciplineResults {
  return disciplineResultsSchema.parse(raw);
}

// ===== labels =====

export const NOT_STARTED_LABEL = 'n.a.';
export const FV_LABEL = 'FV';
export const INVALID_LABEL = 'ungültig';
export const HAND_TIME_MARKER = 'H';
export const PROVISIONAL_MARKER = '*';
export const LINEUP_FALLBACK_LABEL = 'Kader statt Besetzung';
export const TAGESSIEGER_LABEL = 'Tagessieger';

export const INVALID_REASON_LABELS: Record<InvalidReason, string> = {
  fehlversuch: FV_LABEL,
  no_time: INVALID_LABEL,
};

/** `"+1 SZ"`, `"+3 SZ"`; `null` for none. */
export function szLabel(count: number): string | null {
  return count > 0 ? `+${count} SZ` : null;
}

/** `"1."`; `null` place → `"n.a."`. */
export function placeLabel(place: number | null): string {
  return place == null ? NOT_STARTED_LABEL : `${place}.`;
}

// ===== sorting / grouping =====

const STATUS_ORDER: Record<RankStatus, number> = {
  ranked: 0,
  all_invalid: 1,
  not_started: 2,
};

/**
 * Ranked rows by place, then teams with only invalid runs (shared last
 * place), then teams without a completed run; ties by start number.
 * The backend already returns this order — sorting again keeps the view
 * correct for any input.
 */
export function sortRankRows(rows: readonly ResultRow[]): ResultRow[] {
  return [...rows].sort(
    (a, b) =>
      STATUS_ORDER[a.status] - STATUS_ORDER[b.status] ||
      (a.place ?? Number.MAX_SAFE_INTEGER) - (b.place ?? Number.MAX_SAFE_INTEGER) ||
      a.start_number - b.start_number ||
      a.team_name.localeCompare(b.team_name, 'de'),
  );
}

/** WG1 … WG5 order. */
export function sortWgRankings(rankings: readonly WgRanking[]): WgRanking[] {
  return [...rankings].sort((a, b) => WERTUNGSGRUPPEN.indexOf(a.wg) - WERTUNGSGRUPPEN.indexOf(b.wg));
}

export function sortTagessieger(rows: readonly TagessiegerResult[]): TagessiegerResult[] {
  return [...rows].sort((a, b) => a.place - b.place || a.start_number - b.start_number);
}

/** Number of run columns: the highest run number shown, at least 2. */
export function runColumnCount(rows: readonly ResultRow[]): number {
  let max = 2;
  for (const row of rows) {
    for (const run of row.runs) {
      if (run.run_number > max) max = run.run_number;
    }
  }
  return max;
}

// ===== provisional =====

/** Anything still provisional (a counting error is pending)? */
export function isProvisional(results: DisciplineResults | null | undefined): boolean {
  if (!results) return false;
  if (results.provisional) return true;
  for (const wg of results.wg_rankings) {
    for (const row of wg.rows) {
      if (row.provisional || row.runs.some((r) => r.provisional)) return true;
    }
  }
  return results.tagessieger.some((t) => t.provisional);
}

/** Total number of ranked/listed teams across all WGs. */
export function teamCount(results: DisciplineResults): number {
  return results.wg_rankings.reduce((n, w) => n + w.rows.length, 0);
}

// ===== WG table rows =====

export interface RunCell {
  /** Main text: total time, `"FV"`, `"ungültig"` or `"—"` (no completed run). */
  text: string;
  /** `"+2 SZ"` or `null`. */
  sz: string | null;
  invalid: boolean;
  hand: boolean;
  provisional: boolean;
  /** The team's counting run. */
  best: boolean;
  /** Tooltip with the breakdown (measured, SZ, hand times). */
  title: string;
}

export interface WgTableRow {
  key: string;
  status: RankStatus;
  place: string;
  startNumber: string;
  teamName: string;
  runs: RunCell[];
  best: string;
  provisional: boolean;
  /** First row of the "only invalid" or "not started" group (draw a divider). */
  groupStart: boolean;
}

const EMPTY_CELL: RunCell = {
  text: NO_TIME,
  sz: null,
  invalid: false,
  hand: false,
  provisional: false,
  best: false,
  title: '',
};

/** One run cell; `undefined` (run not completed) → `"—"`. */
export function buildRunCell(run: ResultRun | undefined): RunCell {
  if (!run) return { ...EMPTY_CELL };
  const hand = run.time_source === 'handstopp';
  const sz = szLabel(run.sz_count);
  const titleParts: string[] = [];
  if (run.measured_ms != null) titleParts.push(`Gemessen ${formatRaceTime(run.measured_ms)}`);
  if (run.sz_count > 0) titleParts.push(`${run.sz_count} × 15 s Strafzeit`);
  if (run.fv_count > 0) titleParts.push(`${run.fv_count} × Fehlversuch`);
  if (run.invalid_reason === 'no_time') titleParts.push('Keine Zeit erfasst');
  if (hand) {
    const h1 = formatRaceTime(run.hand_time_1_ms);
    const h2 = formatRaceTime(run.hand_time_2_ms);
    titleParts.push(`Handzeit (Mittel aus ${h1} / ${h2})`);
  }
  if (run.provisional) titleParts.push('Vorläufig – Fehler noch nicht bestätigt');
  const invalid = !run.valid;
  const text = invalid
    ? INVALID_REASON_LABELS[run.invalid_reason ?? 'no_time']
    : formatRaceTime(run.total_ms);
  return {
    text,
    sz,
    invalid,
    hand,
    provisional: run.provisional,
    best: run.is_best,
    title: titleParts.join(' · '),
  };
}

/** "Bestzeit" column. */
export function bestTimeLabel(row: ResultRow): string {
  switch (row.status) {
    case 'ranked':
      return formatRaceTime(row.best_total_ms);
    case 'all_invalid':
      return INVALID_LABEL;
    case 'not_started':
      return NOT_STARTED_LABEL;
  }
}

/** Display rows for one WG table with `columns` run columns. */
export function buildWgTableRows(
  rows: readonly ResultRow[],
  columns: number = runColumnCount(rows),
): WgTableRow[] {
  const sorted = sortRankRows(rows);
  return sorted.map((row, i) => {
    const byNumber = new Map(row.runs.map((r) => [r.run_number, r]));
    const runs: RunCell[] = [];
    for (let n = 1; n <= columns; n++) runs.push(buildRunCell(byNumber.get(n)));
    const prev = sorted[i - 1];
    return {
      key: row.team_id,
      status: row.status,
      place: row.status === 'not_started' ? NOT_STARTED_LABEL : placeLabel(row.place),
      startNumber: row.start_number > 0 ? String(row.start_number) : '—',
      teamName: row.team_name,
      runs,
      best: bestTimeLabel(row),
      provisional: row.provisional || row.runs.some((r) => r.provisional),
      groupStart: row.status !== 'ranked' && (prev === undefined || prev.status !== row.status),
    };
  });
}

// ===== Tagessieger rows =====

export interface TagessiegerTableRow {
  key: string;
  place: string;
  startNumber: string;
  teamName: string;
  wg: string;
  avgAge: string;
  factor: string;
  best: string;
  gw: string;
  /** `"Kader statt Besetzung"` when the ages come from the full roster. */
  fallback: string | null;
  provisional: boolean;
  /** `"Alter: 12, 13 (Lauf 2)"`. */
  title: string;
}

export function buildTagessiegerRows(rows: readonly TagessiegerResult[]): TagessiegerTableRow[] {
  return sortTagessieger(rows).map((t) => {
    const titleParts = [`Alter: ${t.ages.join(', ') || '—'}`];
    if (t.run_number != null) titleParts.push(`Lauf ${t.run_number}`);
    if (t.lineup_fallback) titleParts.push('Besetzung fehlt – Alter aus dem ganzen Kader');
    return {
      key: t.team_id,
      place: placeLabel(t.place),
      startNumber: t.start_number > 0 ? String(t.start_number) : '—',
      teamName: t.team_name,
      wg: t.wg_label,
      avgAge: formatDecimalDE(t.avg_age, 1),
      factor: formatDecimalDE(t.factor, 4),
      best: formatRaceTime(t.total_ms),
      gw: formatRaceTime(t.gw_ms),
      fallback: t.lineup_fallback ? LINEUP_FALLBACK_LABEL : null,
      provisional: t.provisional,
      title: titleParts.join(' · '),
    };
  });
}

// ===== tabs =====

export type ResultsTab = Wertungsgruppe | 'tagessieger';

export interface ResultsTabItem {
  key: ResultsTab;
  label: string;
  count: number;
}

/** One tab per WG present (WG order), then "Tagessieger". */
export function resultsTabs(results: DisciplineResults): ResultsTabItem[] {
  const tabs: ResultsTabItem[] = sortWgRankings(results.wg_rankings).map((w) => ({
    key: w.wg,
    label: w.wg_label,
    count: w.rows.length,
  }));
  tabs.push({ key: 'tagessieger', label: TAGESSIEGER_LABEL, count: results.tagessieger.length });
  return tabs;
}

/** Keep the selected tab if it still exists, else the first one. */
export function resolveTab(results: DisciplineResults, current: ResultsTab | null): ResultsTab {
  const tabs = resultsTabs(results);
  if (current && tabs.some((t) => t.key === current)) return current;
  return tabs[0]?.key ?? 'tagessieger';
}
