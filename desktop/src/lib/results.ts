/**
 * Results view model. Types mirror the JSON of `kido_core::scoring::Results`
 * returned by the `compute_results` Tauri command; the builders turn them
 * into table rows (pure, unit-tested).
 *
 * Columns (concept §5):
 * - WG ranking: Platz · Start-Nr. · Mannschaft · Lauf 1..n · Wertung · Bemerkung
 * - Tagessieger: Platz · Start-Nr. · Mannschaft · WG · Zeit · Ø Alter · Faktor · GW-Zeit · Bemerkung
 */
import type { DisciplinePayload, Wertungsgruppe } from "./cloudTypes";
import { wgLabelDE } from "./i18n";

export type InvalidReason = "fehlversuch" | "no_time";
export type RankStatus = "ranked" | "all_invalid" | "not_started";

export interface RunResult {
  run_id: string;
  team_id: string;
  run_number: number;
  valid: boolean;
  invalid_reason: InvalidReason | null;
  total_ms: number | null;
  sz_count: number;
  fv_count: number;
  provisional: boolean;
}

export interface RankRow {
  team_id: string;
  start_number: number;
  place: number | null;
  status: RankStatus;
  best_run_id: string | null;
  best_total_ms: number | null;
  runs: RunResult[];
  provisional: boolean;
}

export interface WgRanking {
  wg: Wertungsgruppe;
  rows: RankRow[];
}

export interface TagessiegerRow {
  team_id: string;
  start_number: number;
  wg: Wertungsgruppe;
  run_id: string;
  total_ms: number;
  ages: number[];
  avg_age: number;
  factor: number;
  gw_ms: number;
  place: number;
  lineup_fallback: boolean;
  provisional: boolean;
}

export interface Results {
  wg_rankings: WgRanking[];
  tagessieger: TagessiegerRow[];
}

export interface ResultsResponse {
  results: Results;
  teams_without_wg: string[];
  runners_without_birth_date: string[];
  competition_day: string;
}

/** Round to 1/100 s, half up (same as `kido_core::timing::round_to_centis`). */
export function roundToCentis(ms: number): number {
  const m = Math.max(0, Math.round(ms));
  return (Math.floor(m / 10) + (m % 10 >= 5 ? 1 : 0)) * 10;
}

/** Result time at 1/100 s with German decimal comma: "47,24", "1:05,30". */
export function formatResultTime(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return "—";
  const centis = roundToCentis(ms) / 10;
  const minutes = Math.floor(centis / 6000);
  const seconds = Math.floor((centis % 6000) / 100);
  const hundredths = centis % 100;
  const cc = hundredths.toString().padStart(2, "0");
  if (minutes === 0) return `${seconds},${cc}`;
  return `${minutes}:${seconds.toString().padStart(2, "0")},${cc}`;
}

export function formatDecimal(value: number, digits: number): string {
  return value.toFixed(digits).replace(".", ",");
}

function teamName(snapshot: DisciplinePayload | null, teamId: string): string {
  return snapshot?.teams.find((t) => t.id === teamId)?.name ?? "?";
}

function isHandTime(snapshot: DisciplinePayload | null, runId: string): boolean {
  return snapshot?.runs.find((r) => r.id === runId)?.time_source === "handstopp";
}

/** One run cell: "47,24", "62,24 (+15 s)", "FV", "o. Zeit"; " H" = Handzeit. */
export function formatRunCell(
  run: RunResult,
  snapshot: DisciplinePayload | null,
): string {
  let text: string;
  if (!run.valid) {
    text = run.invalid_reason === "no_time" ? "o. Zeit" : "FV";
  } else {
    text = formatResultTime(run.total_ms);
    if (run.sz_count > 0) text += ` (+${run.sz_count * 15} s)`;
  }
  if (isHandTime(snapshot, run.run_id)) text += " H";
  if (run.provisional) text += "*";
  return text;
}

export interface WgTableRow {
  key: string;
  place: string;
  startNumber: number;
  teamName: string;
  /** Indexed by run position (Lauf 1, Lauf 2, …); "" if no such run. */
  runCells: string[];
  best: string;
  note: string;
  status: RankStatus;
  provisional: boolean;
}

export function maxRunsPerTeam(ranking: WgRanking): number {
  return ranking.rows.reduce((m, r) => Math.max(m, r.runs.length), 0);
}

export function buildWgRows(
  ranking: WgRanking,
  snapshot: DisciplinePayload | null,
): WgTableRow[] {
  const columns = maxRunsPerTeam(ranking);
  return ranking.rows.map((row) => {
    const runCells = Array.from({ length: columns }, (_, i) =>
      row.runs[i] ? formatRunCell(row.runs[i], snapshot) : "",
    );
    const notes: string[] = [];
    if (row.status === "all_invalid") notes.push("ungültig");
    if (row.status === "not_started") notes.push("nicht angetreten");
    if (row.best_run_id && isHandTime(snapshot, row.best_run_id)) notes.push("Handzeit");
    if (row.provisional) notes.push("vorläufig");
    return {
      key: row.team_id,
      place:
        row.status === "not_started" || row.place == null ? "n.a." : `${row.place}.`,
      startNumber: row.start_number,
      teamName: teamName(snapshot, row.team_id),
      runCells,
      best: row.status === "ranked" ? formatResultTime(row.best_total_ms) : "—",
      note: notes.join(", "),
      status: row.status,
      provisional: row.provisional,
    };
  });
}

export interface TagessiegerTableRow {
  key: string;
  place: string;
  startNumber: number;
  teamName: string;
  wg: string;
  time: string;
  avgAge: string;
  factor: string;
  gw: string;
  note: string;
  provisional: boolean;
}

export function buildTagessiegerRows(
  rows: TagessiegerRow[],
  snapshot: DisciplinePayload | null,
): TagessiegerTableRow[] {
  return rows.map((row) => {
    const notes: string[] = [];
    if (row.lineup_fallback) notes.push("ohne Besetzung (ganzer Kader)");
    if (isHandTime(snapshot, row.run_id)) notes.push("Handzeit");
    if (row.provisional) notes.push("vorläufig");
    return {
      key: row.team_id,
      place: `${row.place}.`,
      startNumber: row.start_number,
      teamName: teamName(snapshot, row.team_id),
      wg: wgLabelDE(row.wg),
      time: formatResultTime(row.total_ms),
      avgAge: formatDecimal(row.avg_age, 2),
      factor: formatDecimal(row.factor, 4),
      gw: formatResultTime(row.gw_ms),
      note: notes.join(", "),
      provisional: row.provisional,
    };
  });
}

/** Names of teams excluded for a missing WG (for the warning line). */
export function teamsWithoutWgNames(
  response: ResultsResponse,
  snapshot: DisciplinePayload | null,
): string[] {
  return response.teams_without_wg.map((id) => teamName(snapshot, id));
}

export function anyProvisional(results: Results): boolean {
  return (
    results.wg_rankings.some((w) => w.rows.some((r) => r.provisional)) ||
    results.tagessieger.some((r) => r.provisional)
  );
}
