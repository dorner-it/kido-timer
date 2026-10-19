// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Lineup per run (who actually runs). Confirmed by the Startlinie judge
 * per run; the organizer can correct it. The Tagessieger uses the
 * lineup's average age — without a lineup it falls back to the whole
 * roster, hence the warning on completed runs.
 */

import type { Run, Runner } from './api';

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
