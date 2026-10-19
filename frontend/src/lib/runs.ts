// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Run (Lauf) management helpers for the organizer: numbering, creating
 * runs from the entries, labels.
 */

import type { Run, RunStatus, Team, TeamEntry } from './api';
import { wgLabel, WG_MISSING_LABEL } from './wg';

export interface CreateRunInput {
  team_id: string;
  runner_id: string | null;
  run_number: number;
  lane: number | null;
}

/** Run numbers are sequential per discipline. */
export function nextRunNumber(runs: readonly Pick<Run, 'run_number'>[]): number {
  return runs.reduce((max, r) => Math.max(max, r.run_number), 0) + 1;
}

/** Lane for a new run: fixed 1 on single-lane disciplines, else the entry's lane. */
export function laneForNewRun(laneCount: number, entryLane: number | null | undefined): number | null {
  if (laneCount <= 1) return 1;
  if (entryLane != null && entryLane >= 1 && entryLane <= laneCount) return entryLane;
  return null;
}

/** Body for creating a single run for one team. */
export function createRunBody(
  teamId: string,
  runs: readonly Pick<Run, 'run_number'>[],
  laneCount: number,
  entryLane: number | null | undefined,
): CreateRunInput {
  return {
    team_id: teamId,
    runner_id: null,
    run_number: nextRunNumber(runs),
    lane: laneForNewRun(laneCount, entryLane),
  };
}

/**
 * "Durchgang anlegen": one new run per entry, in start-number order,
 * numbered after the existing runs.
 */
export function roundBodies(
  entries: readonly Pick<TeamEntry, 'team_id' | 'start_number' | 'lane'>[],
  runs: readonly Pick<Run, 'run_number'>[],
  laneCount: number,
): CreateRunInput[] {
  let n = nextRunNumber(runs);
  return [...entries]
    .sort((a, b) => a.start_number - b.start_number)
    .map((e) => ({
      team_id: e.team_id,
      runner_id: null,
      run_number: n++,
      lane: laneForNewRun(laneCount, e.lane),
    }));
}

export function sortRuns<T extends Pick<Run, 'run_number' | 'lane'>>(runs: readonly T[]): T[] {
  return [...runs].sort((a, b) => a.run_number - b.run_number || (a.lane ?? 99) - (b.lane ?? 99));
}

export function entryForRun<T extends Pick<TeamEntry, 'team_id'>>(
  run: Pick<Run, 'team_id'>,
  entries: readonly T[],
): T | undefined {
  return entries.find((e) => e.team_id === run.team_id);
}

/** `"Lauf 3 · #12 Musterwehr · WG2"`. */
export function runLabel(
  run: Pick<Run, 'run_number' | 'team_id'>,
  teams: readonly Pick<Team, 'id' | 'name'>[],
  entries: readonly Pick<TeamEntry, 'team_id' | 'start_number' | 'wg'>[],
): string {
  const team = teams.find((t) => t.id === run.team_id);
  const entry = entries.find((e) => e.team_id === run.team_id);
  const parts = [`Lauf ${run.run_number}`];
  const name = team?.name ?? 'Unbekannte Mannschaft';
  parts.push(entry ? `#${entry.start_number} ${name}` : name);
  parts.push(entry ? wgLabel(entry.wg, WG_MISSING_LABEL) : WG_MISSING_LABEL);
  return parts.join(' · ');
}

export const RUN_STATUSES: readonly RunStatus[] = ['pending', 'active', 'completed', 'cancelled'];

export function runStatusClasses(status: RunStatus): string {
  switch (status) {
    case 'active':
      return 'bg-status-running/15 text-status-running';
    case 'completed':
      return 'bg-status-confirmed/15 text-status-confirmed';
    case 'cancelled':
      return 'bg-status-unknown/15 text-status-unknown';
    default:
      return 'bg-status-inactive/15 text-status-inactive';
  }
}
