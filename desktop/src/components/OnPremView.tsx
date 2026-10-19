import { useEffect, useMemo, useState } from "react";
import type { CatalogueItem, DisciplinePayload, OnPremStatus, UploadOutcome } from "../lib/cloudTypes";
import { t } from "../lib/i18n";
import { currentRunsOrdered } from "../lib/lanes";
import { currentPicks, laneCandidates, nextRunForLane, picksToRunIds, withPick } from "../lib/onprem";
import { runTitle } from "../lib/runReview";
import { uploadAvailability } from "../lib/uploadFlow";
import { OnPremServerPanel } from "./OnPremServerPanel";
import { RunReviewCard, type ReviewActions } from "./RunReviewCard";
import { UploadDialog } from "./UploadDialog";

interface Props {
  snapshot: DisciplinePayload;
  status: OnPremStatus;
  paired: boolean;
  catalogue: Map<string, CatalogueItem>;
  actions: ReviewActions;
  onBack: () => void;
  onRefreshStatus: () => Promise<void>;
  onRotateToken: () => Promise<void>;
  onSetCurrentRuns: (runIds: string[]) => Promise<boolean>;
  onUpload: () => Promise<UploadOutcome>;
  onGoLive: () => Promise<void>;
}

const STATUS_POLL_MS = 5000;

/**
 * On-prem organizer screen: judge access (QR), current runs per lane,
 * review per current run (+ any other run), upload to the web. Lists
 * refresh via `snapshotChanged` events when phones submit.
 */
export function OnPremView({
  snapshot,
  status,
  paired,
  catalogue,
  actions,
  onBack,
  onRefreshStatus,
  onRotateToken,
  onSetCurrentRuns,
  onUpload,
  onGoLive,
}: Props) {
  const [otherRunId, setOtherRunId] = useState<string>("");
  const [uploadOpen, setUploadOpen] = useState(false);

  // Uptime / phones seen are polled while the screen is open.
  useEffect(() => {
    void onRefreshStatus();
    const id = window.setInterval(() => void onRefreshStatus(), STATUS_POLL_MS);
    return () => window.clearInterval(id);
  }, [onRefreshStatus]);

  const picks = useMemo(() => currentPicks(snapshot), [snapshot]);
  const current = useMemo(() => currentRunsOrdered(snapshot), [snapshot]);
  const sortedRuns = useMemo(
    () => snapshot.runs.slice().sort((a, b) => a.run_number - b.run_number),
    [snapshot.runs],
  );
  const otherRun = sortedRuns.find((r) => r.id === otherRunId && !current.some((c) => c.id === r.id));
  const availability = uploadAvailability({ paired, snapshot });

  const pick = (lane: number, runId: string | null) =>
    void onSetCurrentRuns(picksToRunIds(withPick(picks, lane, runId)));

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto px-7 py-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-4">
          <button
            type="button"
            onClick={onBack}
            className="grid h-10 w-10 place-items-center rounded-xl border border-ink-50/10 bg-ink-50/[0.04] text-ink-50 transition hover:bg-ink-50/10"
            aria-label={t.onprem.back}
          >
            ←
          </button>
          <div>
            <span className="label">{t.onprem.kicker}</span>
            <h1 className="font-display text-[24px] font-medium tracking-tight text-ink-50">
              {t.onprem.title}
            </h1>
            <p className="font-mono text-[11px] text-ink-200/70">
              {snapshot.event.name} · {snapshot.discipline.name}
            </p>
          </div>
        </div>
        <div className="flex flex-col items-end gap-1">
          <button
            type="button"
            className="btn btn-primary"
            disabled={!availability.ok}
            title={availability.ok ? t.onprem.uploadHint : availability.reason}
            onClick={() => setUploadOpen(true)}
          >
            {t.onprem.upload}
          </button>
          {!availability.ok && <span className="max-w-[320px] text-right text-[11px] text-ink-200">{availability.reason}</span>}
          {status.lastUploadAt && (
            <span className="font-mono text-[10px] text-ink-200/70">
              {t.onprem.lastUpload(new Date(status.lastUploadAt).toLocaleString("de-DE"))}
            </span>
          )}
        </div>
      </div>

      <div className="grid min-h-0 grid-cols-1 gap-4 lg:grid-cols-[300px_1fr]">
        <OnPremServerPanel status={status} onRotate={onRotateToken} />

        <div className="flex min-w-0 flex-col gap-4">
          <section className="rounded-2xl border border-ink-50/[0.06] bg-ink-700/40 px-5 py-4">
            <span className="label">{t.onprem.currentRuns}</span>
            <p className="mt-1 text-[12px] text-ink-200">{t.onprem.currentRunsHint}</p>
            <div className={`mt-3 grid gap-3 ${picks.length > 1 ? "md:grid-cols-2" : ""}`}>
              {picks.map((runId, i) => {
                const lane = i + 1;
                const next = nextRunForLane(snapshot, picks, lane);
                return (
                  <div key={lane} className="flex items-center gap-2">
                    <span className="w-16 shrink-0 font-mono text-[11px] uppercase tracking-[0.18em] text-ink-200">
                      {t.onprem.lane(lane)}
                    </span>
                    <select
                      className="min-w-0 flex-1 rounded-lg border border-ink-50/10 bg-ink-50/[0.03] px-2 py-1.5 text-[13px] text-ink-50"
                      value={runId ?? ""}
                      onChange={(e) => pick(lane, e.target.value || null)}
                    >
                      <option value="">{t.onprem.noRun}</option>
                      {laneCandidates(snapshot, lane).map((r) => (
                        <option key={r.id} value={r.id}>
                          {runTitle(snapshot, r)}
                          {r.status === "completed" ? " ✓" : ""}
                        </option>
                      ))}
                    </select>
                    <button
                      type="button"
                      className="btn"
                      disabled={!next}
                      title={t.onprem.nextTitle}
                      onClick={() => next && pick(lane, next.id)}
                    >
                      {t.onprem.next}
                    </button>
                  </div>
                );
              })}
            </div>
          </section>

          <section className="flex flex-col gap-3">
            <span className="label">{t.onprem.review}</span>
            {current.length === 0 ? (
              <p className="text-[13px] text-ink-200">{t.onprem.noCurrent}</p>
            ) : (
              <div className={`grid gap-4 ${current.length > 1 ? "xl:grid-cols-2" : ""}`}>
                {current.map((run, i) => (
                  <RunReviewCard
                    key={run.id}
                    snapshot={snapshot}
                    run={run}
                    laneLabel={t.onprem.lane(run.lane ?? i + 1)}
                    catalogue={catalogue}
                    actions={actions}
                  />
                ))}
              </div>
            )}
          </section>

          <section className="flex flex-col gap-3">
            <label className="flex items-center gap-3">
              <span className="label">{t.onprem.otherRun}</span>
              <select
                className="min-w-0 flex-1 rounded-lg border border-ink-50/10 bg-ink-50/[0.03] px-2 py-1.5 text-[13px] text-ink-50"
                value={otherRunId}
                onChange={(e) => setOtherRunId(e.target.value)}
              >
                <option value="">{t.onprem.otherRunPlaceholder}</option>
                {sortedRuns
                  .filter((r) => !current.some((c) => c.id === r.id))
                  .map((r) => (
                    <option key={r.id} value={r.id}>
                      {runTitle(snapshot, r)}
                    </option>
                  ))}
              </select>
            </label>
            {otherRun && (
              <RunReviewCard snapshot={snapshot} run={otherRun} catalogue={catalogue} actions={actions} />
            )}
          </section>
        </div>
      </div>

      <UploadDialog
        open={uploadOpen}
        runsToSend={status.uploadRuns}
        penaltiesToSend={status.uploadPenalties}
        onClose={() => setUploadOpen(false)}
        upload={onUpload}
        goLive={onGoLive}
      />
    </div>
  );
}
