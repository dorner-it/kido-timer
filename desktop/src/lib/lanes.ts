/**
 * Which run is timed on which desktop lane — UI mirror of
 * `src-tauri/src/lanes.rs` (the Rust side resolves authoritatively when a
 * time is recorded).
 *
 * Hardware: 4 channels; the Gruppenstafette uses one channel per lane,
 * so lane N = channel N. `discipline.lane_count` decides how many lanes
 * are shown (no discipline loaded → all 4 channels).
 *
 * Order per lane: web `current_run_ids` (by `run.lane`; current runs
 * without lane fill free lanes; an already `completed` current run is
 * skipped so a new start never overwrites a result — on-prem the current
 * runs are frozen) → operator override → active run on lane.
 */
import type { DisciplinePayload, Run } from "./cloudTypes";

/** Number of hardware channels on the timer. */
export const HARDWARE_CHANNELS = 4;

/** Lanes to render: the discipline's `lane_count`, else all channels. */
export function laneCountFor(snapshot: DisciplinePayload | null): number {
  if (!snapshot) return HARDWARE_CHANNELS;
  const n = snapshot.discipline.lane_count;
  if (!Number.isFinite(n) || n < 1) return 1;
  return Math.min(n, HARDWARE_CHANNELS);
}

/** Current runs mapped to lanes `1..lane_count`. */
export function currentRunsByLane(
  snapshot: DisciplinePayload | null,
): Map<number, Run> {
  const byLane = new Map<number, Run>();
  if (!snapshot) return byLane;
  const laneCount = Math.max(1, snapshot.discipline.lane_count);
  const current = snapshot.event.current_run_ids
    .map((id) => snapshot.runs.find((r) => r.id === id))
    .filter((r): r is Run => !!r);
  for (const run of current) {
    if (run.lane != null && run.lane >= 1 && run.lane <= laneCount) {
      if (!byLane.has(run.lane)) byLane.set(run.lane, run);
    }
  }
  for (const run of current.filter((r) => r.lane == null)) {
    for (let lane = 1; lane <= laneCount; lane++) {
      if (!byLane.has(lane)) {
        byLane.set(lane, run);
        break;
      }
    }
  }
  return byLane;
}

export type LaneSource = "current" | "override" | "active";

export interface LaneResolution {
  run: Run;
  source: LaneSource;
}

/** Resolve the run for `lane` and where the assignment comes from. */
export function resolveLane(
  snapshot: DisciplinePayload | null,
  lane: number,
  overrides: Record<number, string> = {},
): LaneResolution | null {
  if (!snapshot) return null;
  const current = currentRunsByLane(snapshot).get(lane);
  if (current && current.status !== "completed") {
    return { run: current, source: "current" };
  }
  const overrideId = overrides[lane];
  if (overrideId) {
    const run = snapshot.runs.find((r) => r.id === overrideId);
    if (run) return { run, source: "override" };
  }
  const active = snapshot.runs.find(
    (r) => r.status === "active" && r.lane === lane,
  );
  return active ? { run: active, source: "active" } : null;
}

export function resolveRunForLane(
  snapshot: DisciplinePayload | null,
  lane: number,
  overrides: Record<number, string> = {},
): Run | null {
  return resolveLane(snapshot, lane, overrides)?.run ?? null;
}

/** All current runs, ordered by lane (for the banner). */
export function currentRunsOrdered(snapshot: DisciplinePayload | null): Run[] {
  return Array.from(currentRunsByLane(snapshot).entries())
    .sort((a, b) => a[0] - b[0])
    .map(([, run]) => run);
}

/** Tailwind grid layout for 1, 2 or 4 lane cards. */
export function laneGridClass(laneCount: number): string {
  if (laneCount <= 1) return "grid-cols-1 grid-rows-1";
  if (laneCount === 2) return "grid-cols-2 grid-rows-1";
  return "grid-cols-2 grid-rows-2";
}
