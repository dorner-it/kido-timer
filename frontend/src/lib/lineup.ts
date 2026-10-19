// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Lineup ("Besetzung") check by WR 1 at the Startlinie: which runners of
 * the roster (incl. substitutes) actually run. Pure helpers for the tick
 * list and the request body of
 * `PUT /api/decider/{owner_sub}/{token}/runs/{run_id}/lineup`.
 */

import type { StationKind } from './stations';

/**
 * Usual team size: the Gruppenstafette Wertungsbögen name the runners
 * WK 1 … WK 6. Only used for a soft hint — never a hard limit.
 */
export const EXPECTED_LINEUP_SIZE = 6;

export interface RosterRunnerLike {
  id: string;
  first_name: string;
  last_name: string;
  position: number;
}

export interface LineupRunLike {
  lineup?: readonly string[] | null;
  lineup_confirmed_at?: string | null;
  lineup_confirmed_by?: 'startlinie' | 'organizer' | null;
}

/** Roster by position (then name) — same order as the team sheet. */
export function sortRoster<R extends RosterRunnerLike>(roster: readonly R[]): R[] {
  return [...roster].sort(
    (a, b) =>
      a.position - b.position ||
      a.last_name.localeCompare(b.last_name, 'de') ||
      a.first_name.localeCompare(b.first_name, 'de'),
  );
}

export function runnerName(r: RosterRunnerLike): string {
  return `${r.first_name} ${r.last_name}`.trim();
}

/**
 * Initial ticks:
 * - lineup already stored → those runners (still on the roster),
 * - otherwise, when the roster is not bigger than the usual team size →
 *   everybody (nothing to choose),
 * - otherwise nothing (the judge ticks who runs; substitutes exist).
 */
export function initialSelection(
  run: LineupRunLike,
  roster: readonly RosterRunnerLike[],
  expected = EXPECTED_LINEUP_SIZE,
): Set<string> {
  const ids = new Set(roster.map((r) => r.id));
  const stored = (run.lineup ?? []).filter((id) => ids.has(id));
  if (stored.length > 0) return new Set(stored);
  if (roster.length > 0 && roster.length <= expected) return new Set(ids);
  return new Set();
}

export function toggleRunner(selection: ReadonlySet<string>, runnerId: string): Set<string> {
  const next = new Set(selection);
  if (next.has(runnerId)) next.delete(runnerId);
  else next.add(runnerId);
  return next;
}

/** Ticked ids in roster order; unknown ids and duplicates dropped. */
export function orderedSelection(
  selection: ReadonlySet<string>,
  roster: readonly RosterRunnerLike[],
): string[] {
  return sortRoster(roster)
    .map((r) => r.id)
    .filter((id) => selection.has(id));
}

export interface LineupRequestBody {
  runner_ids: string[];
  station: StationKind;
}

export function lineupRequestBody(
  selection: ReadonlySet<string>,
  roster: readonly RosterRunnerLike[],
): LineupRequestBody {
  return { runner_ids: orderedSelection(selection, roster), station: 'startlinie' };
}

export type LineupSizeHint =
  | { level: 'ok'; message: null }
  | { level: 'warn'; message: string };

/** Soft hint when the count differs from the usual team size. */
export function lineupSizeHint(count: number, expected = EXPECTED_LINEUP_SIZE): LineupSizeHint {
  if (count === expected) return { level: 'ok', message: null };
  if (count === 0) return { level: 'warn', message: 'Niemand angekreuzt.' };
  if (count < expected) {
    return {
      level: 'warn',
      message: `Nur ${count} von üblicherweise ${expected} Wettkämpfern angekreuzt.`,
    };
  }
  return {
    level: 'warn',
    message: `${count} Wettkämpfer angekreuzt – üblich sind ${expected}.`,
  };
}

/** `"5 / 7 angekreuzt"`. */
export function lineupCountLabel(count: number, rosterSize: number): string {
  return `${count} / ${rosterSize} angekreuzt`;
}

export function isLineupConfirmed(run: LineupRunLike): boolean {
  return Boolean(run.lineup_confirmed_at);
}

/** Selection differs from what is stored on the run (order-insensitive). */
export function isLineupDirty(
  selection: ReadonlySet<string>,
  run: LineupRunLike,
  roster: readonly RosterRunnerLike[],
): boolean {
  if (!isLineupConfirmed(run)) return true;
  const ids = new Set(roster.map((r) => r.id));
  const stored = new Set((run.lineup ?? []).filter((id) => ids.has(id)));
  if (stored.size !== selection.size) return true;
  for (const id of selection) if (!stored.has(id)) return true;
  return false;
}

function hhmm(iso: string): string | null {
  const t = Date.parse(iso);
  if (!Number.isFinite(t)) return null;
  const d = new Date(t);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/** `"Bestätigt 14:32 (Startlinie)"` / `"Noch nicht bestätigt"`. */
export function lineupStatusText(run: LineupRunLike): string {
  if (!run.lineup_confirmed_at) return 'Noch nicht bestätigt';
  const time = hhmm(run.lineup_confirmed_at);
  const by =
    run.lineup_confirmed_by === 'organizer'
      ? ' (Wettkampfleitung)'
      : run.lineup_confirmed_by === 'startlinie'
        ? ' (Startlinie)'
        : '';
  return `Bestätigt${time ? ` ${time}` : ''}${by}`;
}

/** The organizer changed the lineup since the judge opened the list:
 *  re-seed the ticks unless the judge has unsaved edits. */
export function lineupSignature(run: LineupRunLike): string {
  return `${run.lineup_confirmed_at ?? ''}|${[...(run.lineup ?? [])].sort().join(',')}`;
}
