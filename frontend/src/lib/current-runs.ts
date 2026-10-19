// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * "Laufende Läufe": which run(s) of the active discipline are on the
 * track right now. One run per lane, at most `lane_count` runs
 * (`Event.current_run_ids`, ordered by lane). With 2 lanes two runs are
 * current at the same time.
 */

import type { Run, SetCurrentInput, TeamEntry } from './api';

type RunLite = Pick<Run, 'id' | 'team_id' | 'lane' | 'run_number' | 'status' | 'discipline_id'>;
type EntryLite = Pick<TeamEntry, 'team_id' | 'lane'>;

/**
 * Lane a run belongs to: the run's own lane, else the team entry's lane.
 * Single-lane disciplines always use lane 1. `null` = not assigned.
 */
export function effectiveLane(
  run: Pick<Run, 'lane' | 'team_id'>,
  entries: readonly EntryLite[],
  laneCount: number,
): number | null {
  if (laneCount <= 1) return 1;
  if (run.lane != null) return run.lane;
  return entries.find((e) => e.team_id === run.team_id)?.lane ?? null;
}

/** Selection slots (index = lane − 1) from `current_run_ids`. */
export function selectionFromCurrent(
  currentRunIds: readonly string[],
  runs: readonly RunLite[],
  entries: readonly EntryLite[],
  laneCount: number,
): (string | null)[] {
  const slots: (string | null)[] = Array.from({ length: Math.max(1, laneCount) }, () => null);
  const byId = new Map(runs.map((r) => [r.id, r]));
  const unplaced: string[] = [];
  for (const id of currentRunIds) {
    const run = byId.get(id);
    const lane = run ? effectiveLane(run, entries, laneCount) : null;
    if (lane != null && lane >= 1 && lane <= slots.length && slots[lane - 1] == null) {
      slots[lane - 1] = id;
    } else {
      unplaced.push(id);
    }
  }
  // Runs without a (free) lane fill the remaining slots in stored order.
  for (const id of unplaced) {
    const free = slots.indexOf(null);
    if (free === -1) break;
    slots[free] = id;
  }
  return slots;
}

/**
 * Runs offered for one lane: not cancelled, lane matches (or the run has
 * no lane yet), sorted by run number.
 */
export function candidatesForLane<T extends RunLite>(
  lane: number,
  runs: readonly T[],
  entries: readonly EntryLite[],
  laneCount: number,
): T[] {
  return runs
    .filter((r) => r.status !== 'cancelled')
    .filter((r) => {
      const l = effectiveLane(r, entries, laneCount);
      return l == null || l === lane;
    })
    .sort((a, b) => a.run_number - b.run_number);
}

/** Validation of a lane selection; returns German error messages. */
export function validateSelection(
  selection: readonly (string | null)[],
  runs: readonly RunLite[],
  entries: readonly EntryLite[],
  laneCount: number,
  disciplineId: string,
): string[] {
  const errors: string[] = [];
  const chosen = selection.filter((id): id is string => id != null);
  if (chosen.length > laneCount) {
    errors.push(`Höchstens ${laneCount} laufende${laneCount === 1 ? 'r' : ''} Lauf/Läufe möglich.`);
  }
  if (selection.length > laneCount && selection.slice(laneCount).some((id) => id != null)) {
    errors.push(`Es gibt nur ${laneCount === 1 ? '1 Bahn' : `${laneCount} Bahnen`}.`);
  }
  const seen = new Set<string>();
  const byId = new Map(runs.map((r) => [r.id, r]));
  selection.forEach((id, idx) => {
    if (id == null) return;
    const lane = idx + 1;
    if (seen.has(id)) {
      errors.push('Ein Lauf kann nicht auf zwei Bahnen gleichzeitig laufen.');
      return;
    }
    seen.add(id);
    const run = byId.get(id);
    if (!run || run.discipline_id !== disciplineId) {
      errors.push(`Bahn ${lane}: Lauf gehört nicht zu diesem Wettkampf.`);
      return;
    }
    if (run.status === 'cancelled') {
      errors.push(`Bahn ${lane}: Lauf ${run.run_number} ist abgebrochen.`);
    }
    const l = effectiveLane(run, entries, laneCount);
    if (l != null && l !== lane) {
      errors.push(`Bahn ${lane}: Lauf ${run.run_number} ist Bahn ${l} zugeordnet.`);
    }
  });
  return errors;
}

/** Body for `POST /api/events/{id}/current` — run ids ordered by lane. */
export function setCurrentBody(
  disciplineId: string,
  selection: readonly (string | null)[],
): SetCurrentInput {
  return {
    discipline_id: disciplineId,
    run_ids: selection.filter((id): id is string => id != null),
  };
}

/** Whether the selection differs from what is stored (ignores order of empty slots). */
export function selectionChanged(
  selection: readonly (string | null)[],
  currentRunIds: readonly string[],
): boolean {
  const a = selection.filter((id): id is string => id != null);
  if (a.length !== currentRunIds.length) return true;
  return a.some((id, i) => id !== currentRunIds[i]);
}

/**
 * "Nächste Läufe": per lane the first pending run (lowest run number)
 * that comes after the run currently on that lane; empty lane → first
 * pending run of the lane. Lanes with no further run become empty.
 */
export function suggestNext(
  selection: readonly (string | null)[],
  runs: readonly RunLite[],
  entries: readonly EntryLite[],
  laneCount: number,
): (string | null)[] {
  const byId = new Map(runs.map((r) => [r.id, r]));
  // Runs currently on the track are never suggested again.
  const taken = new Set<string>(selection.filter((id): id is string => id != null));
  const out: (string | null)[] = [];
  for (let lane = 1; lane <= laneCount; lane++) {
    const cur = selection[lane - 1] ? byId.get(selection[lane - 1]!) : undefined;
    const after = cur?.run_number ?? 0;
    const next = candidatesForLane(lane, runs, entries, laneCount).find(
      (r) => r.status === 'pending' && r.run_number > after && !taken.has(r.id) && r.id !== cur?.id,
    );
    if (next) taken.add(next.id);
    out.push(next?.id ?? null);
  }
  return out;
}
