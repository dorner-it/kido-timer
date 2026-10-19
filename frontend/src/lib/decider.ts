// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Pure helpers for the judge (Strafrichter) station app.
 */

import type { CatalogueEntry, JudgeMode, StationPenaltyInput } from './api';
import { PENALTY_KIND_SHORT } from './penalties';
import type { StationKind } from './stations';

export interface CurrentRunLike {
  run: { id: string; lane: number | null; run_number: number };
  team: { name: string };
}

/** Context of the judge for lane resolution. All optional so the
 *  plain single-run call `resolveSubmitTarget(runs)` keeps working. */
export interface JudgeContext {
  judgeMode?: JudgeMode | null;
  laneCount?: number | null;
  /** Judge's own lane (`per_station_and_lane` only). */
  myLane?: number | null;
  /** Run the judge picked (tap → "Bahn 1 · Team"). */
  selectedRunId?: string | null;
}

export type SubmitTarget<R extends CurrentRunLike = CurrentRunLike> =
  | { ok: true; run_id: string; run: R }
  | {
      ok: false;
      reason: 'no_run' | 'no_run_on_lane' | 'no_lane' | 'choose_run';
      message: string;
      /** Runs to choose from (`choose_run` only, ordered by lane). */
      options: R[];
    };

/** `true` when this judge only watches a single lane. */
export function isLaneBoundJudge(ctx: JudgeContext): boolean {
  return ctx.judgeMode === 'per_station_and_lane' && (ctx.laneCount ?? 1) > 1;
}

/**
 * Runs this judge may record errors for:
 * - `per_station_and_lane` with 2+ lanes → only the run on the judge's
 *   lane (runs without a lane are visible to every lane judge — legacy
 *   data, better visible than lost). No lane chosen → nothing.
 * - otherwise → all current runs.
 * Order: by lane, runs without lane last (stable).
 */
export function visibleRuns<R extends CurrentRunLike>(
  currentRuns: readonly R[],
  ctx: JudgeContext = {},
): R[] {
  const sorted = [...currentRuns].sort(
    (a, b) => (a.run.lane ?? Infinity) - (b.run.lane ?? Infinity),
  );
  if (!isLaneBoundJudge(ctx)) return sorted;
  if (ctx.myLane == null) return [];
  return sorted.filter((cr) => cr.run.lane === ctx.myLane || cr.run.lane == null);
}

/**
 * Which run a station submission goes to.
 *
 * - no current run → `no_run`
 * - lane-bound judge without lane → `no_lane`; nothing on their lane →
 *   `no_run_on_lane`
 * - exactly one visible run → that run (no extra tap)
 * - several visible runs (`per_station`, 2 lanes) → the selected run if
 *   it is one of them, else `choose_run` with the options
 */
export function resolveSubmitTarget<R extends CurrentRunLike>(
  currentRuns: readonly R[],
  ctx: JudgeContext = {},
): SubmitTarget<R> {
  if (currentRuns.length === 0) {
    return { ok: false, reason: 'no_run', message: 'Aktuell läuft kein Lauf.', options: [] };
  }
  if (isLaneBoundJudge(ctx) && ctx.myLane == null) {
    return {
      ok: false,
      reason: 'no_lane',
      message: 'Bitte zuerst deine Bahn wählen.',
      options: [],
    };
  }
  const runs = visibleRuns(currentRuns, ctx);
  if (runs.length === 0) {
    return {
      ok: false,
      reason: 'no_run_on_lane',
      message: `Auf Bahn ${ctx.myLane} läuft gerade kein Lauf.`,
      options: [],
    };
  }
  if (runs.length === 1) {
    return { ok: true, run_id: runs[0]!.run.id, run: runs[0]! };
  }
  const picked = ctx.selectedRunId
    ? runs.find((cr) => cr.run.id === ctx.selectedRunId)
    : undefined;
  if (picked) return { ok: true, run_id: picked.run.id, run: picked };
  return {
    ok: false,
    reason: 'choose_run',
    message: 'Bitte die Bahn wählen.',
    options: runs,
  };
}

/** Errors can be entered (possibly after choosing a lane). */
export function canEnterErrors(target: SubmitTarget): boolean {
  return target.ok || target.reason === 'choose_run';
}

/** `"Bahn 1 · FF Musterstadt"`; without lane just the team name. */
export function currentRunLabel(cr: CurrentRunLike): string {
  return cr.run.lane != null ? `Bahn ${cr.run.lane} · ${cr.team.name}` : cr.team.name;
}

/** 2027 submission: the catalogue code decides kind and delta server-side. */
export function catalogueSubmitBody(
  station: StationKind,
  entry: Pick<CatalogueEntry, 'code'>,
  runId: string | undefined,
  clientId: string,
): StationPenaltyInput {
  const body: StationPenaltyInput = { station, code: entry.code, client_id: clientId };
  if (runId) body.run_id = runId;
  return body;
}

/** Pre-2027 fallback (no catalogue available from the backend). */
export function legacySubmitBody(
  station: StationKind,
  opts: { deltaMs: number; reasonCode: string | null; dq?: boolean },
  runId: string | undefined,
  clientId: string,
): StationPenaltyInput {
  const body: StationPenaltyInput = {
    station,
    client_id: clientId,
    reason_code: opts.reasonCode,
  };
  if (runId) body.run_id = runId;
  if (opts.dq) body.is_disqualification = true;
  else body.delta_ms = opts.deltaMs;
  return body;
}

/** Label for the "Eigene Eingaben" list, e.g. `"SZ · 1 Knoten falsch"`. */
export function catalogueEntryLabel(entry: CatalogueEntry): string {
  return `${PENALTY_KIND_SHORT[entry.kind]} · ${entry.label_de}`;
}
