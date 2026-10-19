// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Lineup ("Besetzung") per run: which runners of the roster (incl.
 * substitutes) actually run. Confirmed by WR 1 at the Startlinie (judge
 * app, `PUT /api/decider/{owner_sub}/{token}/runs/{run_id}/lineup`); the
 * organizer can correct it. The Tagessieger uses the lineup's average
 * age — without a lineup it falls back to the whole roster, hence the
 * warning on completed runs.
 */

import type { Run, Runner } from './api';
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

// --- Organizer review (RunsPanel / LineupEditor) ---

export const LINEUP_MISSING_TEXT = 'Besetzung fehlt — Tagessieger nutzt dann den ganzen Kader';

/** `"10:42"` in German local time; `null` on an invalid timestamp. */
export function formatClock(iso: string | null | undefined): string | null {
  if (!iso) return null;
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return null;
  return d.toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
}

/**
 * Who confirmed the lineup: `"Startlinie, 10:42"` / `"Organisator, 10:42"`;
 * `"nicht bestätigt"` when nobody did.
 */
export function lineupConfirmationText(
  run: Pick<Run, 'lineup_confirmed_by' | 'lineup_confirmed_at'>,
  clock: (iso: string | null | undefined) => string | null = formatClock,
): string {
  if (!run.lineup_confirmed_by) return 'nicht bestätigt';
  const who = run.lineup_confirmed_by === 'startlinie' ? 'Startlinie' : 'Organisator';
  const at = clock(run.lineup_confirmed_at);
  return at ? `${who}, ${at}` : who;
}

/** Completed run without lineup → warning badge. */
export function lineupMissing(run: Pick<Run, 'status' | 'lineup'>): boolean {
  return run.status === 'completed' && (run.lineup ?? []).length === 0;
}

/** Team roster in roster order (position, then name). */
export function rosterForTeam<T extends Pick<Runner, 'team_id' | 'position' | 'last_name' | 'first_name'>>(
  runners: readonly T[],
  teamId: string,
): T[] {
  return runners
    .filter((r) => r.team_id === teamId)
    .sort(
      (a, b) =>
        a.position - b.position ||
        a.last_name.localeCompare(b.last_name, 'de') ||
        a.first_name.localeCompare(b.first_name, 'de'),
    );
}

/**
 * All rosters of an event, keyed by team id, each in roster order
 * (position, then name). Positions are not unique — a runner created at
 * an occupied position sits next to the other one, ordered by name.
 */
export function rostersByTeam<T extends RosterRunnerLike & Pick<Runner, 'team_id'>>(
  runners: readonly T[],
): Map<string, T[]> {
  const m = new Map<string, T[]>();
  for (const r of runners) {
    const list = m.get(r.team_id) ?? [];
    list.push(r);
    m.set(r.team_id, list);
  }
  for (const [team, list] of m) m.set(team, sortRoster(list));
  return m;
}

/** Tick / untick one runner. */
export function toggleLineup(ids: readonly string[], runnerId: string): string[] {
  return ids.includes(runnerId) ? ids.filter((id) => id !== runnerId) : [...ids, runnerId];
}

/**
 * Lineup to send: deduplicated, only runners of the roster, in roster
 * order (the backend rejects foreign ids and duplicates).
 */
export function lineupBody(ids: readonly string[], roster: readonly Pick<Runner, 'id'>[]): { runner_ids: string[] } {
  const chosen = new Set(ids);
  return { runner_ids: roster.filter((r) => chosen.has(r.id)).map((r) => r.id) };
}

/** Lineup ids that are no longer in the roster (runner deleted). */
export function unknownLineupIds(ids: readonly string[], roster: readonly Pick<Runner, 'id'>[]): string[] {
  const known = new Set(roster.map((r) => r.id));
  return ids.filter((id) => !known.has(id));
}

/** Set equality: has the organizer changed anything? */
export function lineupChanged(a: readonly string[], b: readonly string[]): boolean {
  const sa = new Set(a);
  const sb = new Set(b);
  if (sa.size !== sb.size) return true;
  for (const id of sa) if (!sb.has(id)) return true;
  return false;
}
