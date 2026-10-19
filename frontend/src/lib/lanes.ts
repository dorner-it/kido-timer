// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Lane rules per discipline (mirrors `kido_core::TimerMode`).
 * Gruppenstaffette: 1 or 2 lanes (default 2). Löschangriff: fixed 4.
 */

import type { JudgeMode, TeamEntry, TimerMode } from './api';

export const ALLOWED_LANE_COUNTS: Record<TimerMode, readonly number[]> = {
  gruppenstaffette: [1, 2],
  loeschangriff: [4],
};

export function allowedLaneCounts(mode: TimerMode): number[] {
  return [...ALLOWED_LANE_COUNTS[mode]];
}

/** Default for new disciplines (= maximum lanes of the mode). */
export function defaultLaneCount(mode: TimerMode): number {
  const counts = ALLOWED_LANE_COUNTS[mode];
  return counts[counts.length - 1] ?? 1;
}

export function maxLanes(mode: TimerMode): number {
  return defaultLaneCount(mode);
}

export function isValidLaneCount(mode: TimerMode, laneCount: unknown): boolean {
  return (
    typeof laneCount === 'number' &&
    Number.isInteger(laneCount) &&
    ALLOWED_LANE_COUNTS[mode].includes(laneCount)
  );
}

/** Organizer can only choose when the mode allows more than one value. */
export function isLaneCountEditable(mode: TimerMode): boolean {
  return ALLOWED_LANE_COUNTS[mode].length > 1;
}

/** Lane count of a discipline; legacy rows without `lane_count` get the
 *  mode default. Out-of-range values are clamped into the allowed set. */
export function effectiveLaneCount(d: { mode: TimerMode; lane_count?: number | null }): number {
  if (isValidLaneCount(d.mode, d.lane_count)) return d.lane_count as number;
  return defaultLaneCount(d.mode);
}

export function laneCountLabel(laneCount: number): string {
  return laneCount === 1 ? '1 Bahn' : `${laneCount} Bahnen`;
}

/** A lane picker is only shown when there is something to pick. */
export function showsLanePicker(laneCount: number): boolean {
  return laneCount > 1;
}

export interface LaneOption {
  value: number;
  label: string;
  /** `true` for a stored lane beyond the current lane count. */
  conflict: boolean;
}

/**
 * Options for a lane `<select>`: 1..laneCount, plus the current value
 * (flagged as conflict) if it lies outside — so a lane stored before
 * the lane count was reduced stays visible instead of silently blank.
 */
export function laneOptions(laneCount: number, current: number | null = null): LaneOption[] {
  const opts: LaneOption[] = Array.from({ length: Math.max(0, laneCount) }, (_, i) => ({
    value: i + 1,
    label: String(i + 1),
    conflict: false,
  }));
  if (current != null && (current < 1 || current > laneCount)) {
    opts.push({ value: current, label: `${current} (ungültig)`, conflict: true });
  }
  return opts;
}

/** Lane to send for a *new* entry: fixed 1 on single-lane disciplines. */
export function laneForNewEntry(laneCount: number, chosen: number | null): number | null {
  if (laneCount === 1) return 1;
  return chosen;
}

export function isLaneConflict(lane: number | null | undefined, laneCount: number): boolean {
  return lane != null && lane > laneCount;
}

/** Team ids whose stored lane exceeds the lane count (client-side mirror
 *  of the backend's `lane_conflicts`). */
export function laneConflicts(
  entries: readonly Pick<TeamEntry, 'team_id' | 'lane'>[],
  laneCount: number,
): string[] {
  return entries.filter((e) => isLaneConflict(e.lane, laneCount)).map((e) => e.team_id);
}

// ===== judge mode =====

export const JUDGE_MODES: readonly JudgeMode[] = ['per_station', 'per_station_and_lane'];

export const JUDGE_MODE_LABELS: Record<JudgeMode, string> = {
  per_station: 'Ein Richter je Station',
  per_station_and_lane: 'Ein Richter je Station und Bahn',
};

export function isJudgeMode(value: unknown): value is JudgeMode {
  return typeof value === 'string' && (JUDGE_MODES as readonly string[]).includes(value);
}
