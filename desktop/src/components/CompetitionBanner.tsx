import { useMemo } from "react";
import { modeLabelDE, t, wgDescriptionDE, wgLabelDE } from "../lib/i18n";
import type {
  CatalogueItem,
  DisciplinePayload,
  Run,
  SyncMode,
} from "../lib/cloudTypes";
import { currentRunsOrdered } from "../lib/lanes";
import { penaltiesForRun } from "../lib/penalties";
import { formatResultTime } from "../lib/results";
import { RunPenalties } from "./RunPenalties";
import { SyncModeToggle } from "./SyncModeToggle";

interface Props {
  snapshot: DisciplinePayload | null;
  effectiveMode: SyncMode | null;
  catalogue: Map<string, CatalogueItem>;
  onSyncModeChange: (mode: SyncMode | null) => void;
  onClose: () => void;
}

export function CompetitionBanner({
  snapshot,
  effectiveMode,
  catalogue,
  onSyncModeChange,
  onClose,
}: Props) {
  const currentRuns = useMemo(() => currentRunsOrdered(snapshot), [snapshot]);
  if (!snapshot || !effectiveMode) return null;
  const event = snapshot.event;
  const discipline = snapshot.discipline;
  const live = effectiveMode === "live";

  return (
    <div className="mx-7 mt-4 rounded-2xl border border-ink-50/[0.05] bg-ink-700/40 px-5 py-4">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <span className="label">{t.banner.activeCompetition}</span>
          <h3 className="mt-1 font-display text-[18px] font-medium tracking-tight text-ink-50 truncate">
            {event.name}
            <span className="mx-2 font-mono text-[12px] text-ink-200/70">·</span>
            <span className="font-display text-[16px] text-ink-100">
              {discipline.name}
            </span>
          </h3>
          <p className="mt-0.5 font-mono text-[11px] text-ink-200/70">
            {formatDate(event.date)}
            {event.location ? ` · ${event.location}` : ""}
            {` · ${modeLabelDE(discipline.mode, discipline.lane_count)}`}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <SyncModeToggle
            webDefault={discipline.sync_mode}
            effective={effectiveMode}
            onChange={onSyncModeChange}
          />
          <button
            type="button"
            onClick={onClose}
            aria-label={t.banner.closeSnapshot}
            className="grid h-7 w-7 place-items-center rounded-md border border-ink-50/10 bg-ink-50/[0.03] font-mono text-[11px] text-ink-200 hover:bg-ink-50/10"
          >
            ✕
          </button>
        </div>
      </div>

      <div className="mt-3">
        <span className="label">{t.banner.currentRuns}</span>
        {currentRuns.length === 0 ? (
          <p className="mt-1 font-mono text-[12px] text-ink-200/60">{t.banner.none}</p>
        ) : (
          <div
            className={[
              "mt-1 grid grid-cols-1 gap-3",
              currentRuns.length > 1 ? "md:grid-cols-2" : "",
            ].join(" ")}
          >
            {currentRuns.map((run) => (
              <CurrentRunTile
                key={run.id}
                snapshot={snapshot}
                run={run}
                catalogue={catalogue}
              />
            ))}
          </div>
        )}
      </div>

      <p className="mt-3 font-mono text-[10px] uppercase tracking-[0.2em] text-ink-200/60">
        {live ? t.banner.syncLive : t.banner.syncOffline}
      </p>
    </div>
  );
}

function CurrentRunTile({
  snapshot,
  run,
  catalogue,
}: {
  snapshot: DisciplinePayload;
  run: Run;
  catalogue: Map<string, CatalogueItem>;
}) {
  const team = snapshot.teams.find((tm) => tm.id === run.team_id) ?? null;
  const entry =
    snapshot.team_entries.find(
      (e) => e.team_id === run.team_id && e.discipline_id === snapshot.discipline.id,
    ) ?? null;
  const penalties = penaltiesForRun(snapshot, run.id);
  return (
    <div className="rounded-xl border border-ink-50/[0.05] bg-ink-50/[0.02] px-3 py-2">
      <div className="flex items-baseline justify-between gap-2">
        <p className="min-w-0 truncate font-display text-[14px] text-ink-50">
          {run.lane != null && (
            <span className="mr-2 font-mono text-[11px] tracking-wider text-ink-200/70">
              {t.banner.lane} {run.lane}
            </span>
          )}
          {entry && (
            <span className="mr-2 font-mono text-[11px] tracking-wider text-ink-200/70">
              #{entry.start_number}
            </span>
          )}
          {team?.name ?? "?"}
          {entry?.wg && (
            <span
              title={wgDescriptionDE(entry.wg)}
              className="ml-2 rounded px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-[0.18em] bg-signal/15 text-signal-glow ring-1 ring-signal/30"
            >
              {wgLabelDE(entry.wg)}
            </span>
          )}
        </p>
        <span className="shrink-0 font-mono text-[10px] uppercase tracking-[0.2em] text-ink-200/60">
          {t.banner.runOf} {run.run_number} · {statusLabel(run.status)}
        </span>
      </div>
      {run.original_time_ms != null && (
        <p className="mt-1 font-mono text-[12px] tnum text-ink-100">
          {formatResultTime(run.original_time_ms)} s
          {run.time_source === "handstopp" && (
            <span className="ml-2 text-[10px] uppercase tracking-[0.18em] text-status-armed">
              {t.banner.handTime}
            </span>
          )}
        </p>
      )}
      <RunPenalties penalties={penalties} catalogue={catalogue} />
    </div>
  );
}

function statusLabel(s: string): string {
  switch (s) {
    case "pending":
      return "Wartend";
    case "active":
      return "Aktiv";
    case "completed":
      return "Beendet";
    case "cancelled":
      return "Abgebrochen";
    default:
      return s;
  }
}

function formatDate(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return iso;
  return `${m[3]}.${m[2]}.${m[1]}`;
}
