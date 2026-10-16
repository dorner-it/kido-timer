import { useMemo, useState } from "react";
import { stationLabelDE, t, wgDescriptionDE, wgLabelDE } from "../lib/i18n";
import type {
  DisciplinePayload,
  Penalty,
  Run,
  TeamEntry,
  Wertungsgruppe,
} from "../lib/cloudTypes";
import { pickRunForLane } from "../lib/useCloud";

interface Props {
  snapshot: DisciplinePayload;
  laneOverrides: Record<number, string>;
  onSetOverride: (lane: number, runId: string | null) => void;
}

const LANE_COUNT = 4;

/**
 * Lane-assignment panel for the desktop operator. Auto-binding only fits
 * the simple modes (single lane / two-lane parallel); for relay-style modes
 * the operator picks which run is on each physical lane.
 *
 * Schema v3 additions:
 * - Optional WG filter (only shown if any team_entry carries a `wg`).
 * - WG badge per lane (next to team/runner hint).
 * - Approved-penalty count + DQ marker per lane (lit from
 *   `snapshot.approved_penalties` matched by `run_id`).
 */
export function LaneAssignmentPanel({
  snapshot,
  laneOverrides,
  onSetOverride,
}: Props) {
  const [wgFilter, setWgFilter] = useState<Wertungsgruppe | "">("");

  const wgOptions = useMemo(() => collectWgs(snapshot.team_entries), [
    snapshot.team_entries,
  ]);

  const eligibleRuns = useMemo(() => {
    const wgByTeam = new Map<string, Wertungsgruppe | null>();
    for (const e of snapshot.team_entries) {
      if (e.discipline_id !== snapshot.discipline.id) continue;
      wgByTeam.set(e.team_id, e.wg ?? null);
    }
    return snapshot.runs
      .filter((r) => r.status === "active" || r.status === "pending")
      .filter((r) => {
        if (!wgFilter) return true;
        return wgByTeam.get(r.team_id) === wgFilter;
      })
      .sort((a, b) => a.run_number - b.run_number);
  }, [snapshot.runs, snapshot.team_entries, snapshot.discipline.id, wgFilter]);

  // Hide the lane-picker for the simple modes — auto-binding by lane works
  // there. WG filter still gets rendered if there are WGs to show.
  const mode = snapshot.discipline.mode;
  const isSimple = mode === "single_lane" || mode === "two_lane_parallel";
  if (isSimple && wgOptions.length === 0) {
    return null;
  }
  if (!isSimple && eligibleRuns.length === 0 && !wgFilter) return null;

  return (
    <section className="mx-7 mt-3 rounded-2xl border border-ink-50/[0.05] bg-ink-700/40 px-5 py-4">
      <header className="mb-3 flex items-end justify-between gap-3">
        <div>
          <span className="label">{t.laneAssign.title}</span>
          <p className="mt-0.5 font-mono text-[11px] text-ink-200/70">
            {t.laneAssign.subtitle}
          </p>
        </div>
        {wgOptions.length > 0 && (
          <label className="flex items-center gap-2 font-mono text-[11px] text-ink-200/80">
            <span className="uppercase tracking-[0.18em]">
              {t.laneAssign.wgFilter}
            </span>
            <select
              value={wgFilter}
              onChange={(e) =>
                setWgFilter((e.target.value as Wertungsgruppe) || "")
              }
              className="input text-[12px]"
            >
              <option value="">{t.laneAssign.wgFilterAll}</option>
              {wgOptions.map((wg) => (
                <option key={wg} value={wg}>
                  {wgLabelDE(wg)} — {wgDescriptionDE(wg)}
                </option>
              ))}
            </select>
          </label>
        )}
      </header>

      {!isSimple && (
        <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
          {Array.from({ length: LANE_COUNT }, (_, i) => i + 1).map((lane) => {
            const overrideId = laneOverrides[lane] ?? null;
            const auto = pickRunForLane(snapshot, lane, {});
            const selected = overrideId
              ? eligibleRuns.find((r) => r.id === overrideId) ?? null
              : auto;
            return (
              <div
                key={lane}
                className="flex items-center gap-3 rounded-xl border border-ink-50/[0.05] bg-ink-50/[0.02] px-3 py-2"
              >
                <span className="w-12 shrink-0 font-mono text-[11px] uppercase tracking-[0.2em] text-ink-200/70">
                  {t.laneAssign.laneShort} {lane}
                </span>
                <select
                  value={overrideId ?? ""}
                  onChange={(e) =>
                    onSetOverride(lane, e.target.value || null)
                  }
                  className="input flex-1 text-[12px]"
                >
                  <option value="">{t.laneAssign.auto}</option>
                  {eligibleRuns.map((r) => (
                    <option key={r.id} value={r.id}>
                      {labelForRun(snapshot, r)}
                    </option>
                  ))}
                </select>
                <RunHint
                  snapshot={snapshot}
                  run={selected}
                  isAuto={!overrideId}
                />
              </div>
            );
          })}
        </div>
      )}

      {snapshot.approved_penalties.length > 0 && (
        <p className="mt-3 font-mono text-[10px] uppercase tracking-[0.18em] text-ink-200/50">
          {t.laneAssign.penaltyHint}
        </p>
      )}
    </section>
  );
}

function collectWgs(entries: TeamEntry[]): Wertungsgruppe[] {
  const seen = new Set<Wertungsgruppe>();
  for (const e of entries) {
    if (e.wg) seen.add(e.wg);
  }
  return Array.from(seen).sort();
}

function RunHint({
  snapshot,
  run,
  isAuto,
}: {
  snapshot: DisciplinePayload;
  run: Run | null;
  isAuto: boolean;
}) {
  if (!run) {
    return (
      <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-ink-200/50">
        {t.laneAssign.none}
      </span>
    );
  }
  const team = snapshot.teams.find((tm) => tm.id === run.team_id);
  const runner = run.runner_id
    ? snapshot.runners.find((r) => r.id === run.runner_id)
    : null;
  const entry = snapshot.team_entries.find(
    (e) => e.team_id === run.team_id && e.discipline_id === run.discipline_id,
  );
  const penalties = penaltiesForRun(snapshot.approved_penalties, run.id);
  const dq = penalties.some((p) => p.is_disqualification);
  const counted = penalties.filter((p) => !p.is_disqualification).length;

  return (
    <div className="flex shrink-0 items-center gap-1.5 truncate max-w-[55%]">
      <span className="font-mono text-[10px] tracking-wider text-ink-200/70 truncate">
        {isAuto ? "auto · " : ""}
        {team?.name ?? "?"}
        {runner ? ` · ${runner.first_name} ${runner.last_name}` : ""}
      </span>
      {entry?.wg && (
        <span
          title={wgDescriptionDE(entry.wg)}
          className="shrink-0 rounded px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-[0.18em] bg-signal/15 text-signal-glow ring-1 ring-signal/30"
        >
          {wgLabelDE(entry.wg)}
        </span>
      )}
      {dq && (
        <span className="shrink-0 rounded px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-[0.18em] bg-status-unknown/20 text-status-unknown ring-1 ring-status-unknown/40">
          {t.laneAssign.dq}
        </span>
      )}
      {counted > 0 && !dq && (
        <span
          title={penalties
            .map((p) => (p.station ? stationLabelDE(p.station) : "?"))
            .join(", ")}
          className="shrink-0 rounded px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-[0.18em] bg-status-armed/15 text-status-armed ring-1 ring-status-armed/30"
        >
          {t.laneAssign.penaltyCount(counted)}
        </span>
      )}
    </div>
  );
}

function penaltiesForRun(all: Penalty[], runId: string): Penalty[] {
  return all.filter((p) => p.run_id === runId);
}

function labelForRun(snapshot: DisciplinePayload, r: Run): string {
  const team = snapshot.teams.find((tm) => tm.id === r.team_id);
  const runner = r.runner_id
    ? snapshot.runners.find((x) => x.id === r.runner_id)
    : null;
  const entry = snapshot.team_entries.find(
    (e) => e.team_id === r.team_id && e.discipline_id === r.discipline_id,
  );
  const parts = [
    `${t.laneAssign.runLabel} ${r.run_number}`,
    team ? team.name : "?",
  ];
  if (entry?.wg) parts.push(wgLabelDE(entry.wg));
  if (runner) parts.push(`${runner.first_name} ${runner.last_name}`);
  if (r.lane != null) parts.push(`${t.laneAssign.laneShort} ${r.lane}`);
  parts.push(
    r.status === "active" ? t.laneAssign.statusActive : t.laneAssign.statusPending,
  );
  return parts.join(" · ");
}
