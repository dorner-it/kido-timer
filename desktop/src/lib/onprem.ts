/**
 * On-prem screen helpers (pure): current-run selection per lane and the
 * "no phone connected" hint.
 */
import type { DisciplinePayload, OnPremStatus, Run } from "./cloudTypes";
import { currentRunsByLane, laneCountFor } from "./lanes";

/** After this many seconds without a phone the screen shows the firewall hint. */
export const NO_PHONE_HINT_SECS = 60;

/** Server running for a while, but no phone has connected yet. */
export function showNoPhoneHint(status: OnPremStatus, graceSecs = NO_PHONE_HINT_SECS): boolean {
  return (
    status.running && (status.uptimeSecs ?? 0) >= graceSecs && status.clients.length === 0
  );
}

/** Runs that can be put on `lane`: same lane or no lane, by run number. */
export function laneCandidates(snapshot: DisciplinePayload, lane: number): Run[] {
  return snapshot.runs
    .filter((r) => r.discipline_id === snapshot.discipline.id || !r.discipline_id)
    .filter((r) => r.lane === lane || r.lane == null)
    .slice()
    .sort((a, b) => a.run_number - b.run_number);
}

/** Current run per lane `1..lane_count` (`null` = none). */
export function currentPicks(snapshot: DisciplinePayload | null): (string | null)[] {
  if (!snapshot) return [];
  const byLane = currentRunsByLane(snapshot);
  return Array.from({ length: laneCountFor(snapshot) }, (_, i) => byLane.get(i + 1)?.id ?? null);
}

/** Replace the pick of `lane` (1-based); a run picked on another lane moves. */
export function withPick(
  picks: readonly (string | null)[],
  lane: number,
  runId: string | null,
): (string | null)[] {
  return picks.map((p, i) => {
    if (i === lane - 1) return runId;
    return runId != null && p === runId ? null : p;
  });
}

/** Picks → `current_run_ids` (lane order, gaps dropped). */
export function picksToRunIds(picks: readonly (string | null)[]): string[] {
  return picks.filter((p): p is string => p != null);
}

/** Next run for `lane` after the current one: lowest run number that is
 *  not completed / cancelled and not current on another lane. */
export function nextRunForLane(
  snapshot: DisciplinePayload,
  picks: readonly (string | null)[],
  lane: number,
): Run | null {
  const current = picks[lane - 1];
  const currentRun = current ? snapshot.runs.find((r) => r.id === current) : null;
  const after = currentRun?.run_number ?? 0;
  const taken = new Set(picks.filter((p, i) => p != null && i !== lane - 1));
  return (
    laneCandidates(snapshot, lane).find(
      (r) =>
        r.run_number > after &&
        !taken.has(r.id) &&
        r.status !== "completed" &&
        r.status !== "cancelled",
    ) ?? null
  );
}

/** `"vor 12 s"` / `"vor 3 min"`. */
export function formatAgo(secs: number): string {
  if (secs < 60) return `vor ${secs} s`;
  return `vor ${Math.floor(secs / 60)} min`;
}
