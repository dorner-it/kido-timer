import { useMemo, useState } from "react";
import type { CatalogueItem, DisciplinePayload, Penalty, Run } from "../lib/cloudTypes";
import { handTimeRawText, isHandTime } from "../lib/handTime";
import { t } from "../lib/i18n";
import { penaltiesForRun, penaltyLabel } from "../lib/penalties";
import { formatResultTime } from "../lib/results";
import {
  canReview,
  canUndoOnDesktop,
  groupByStation,
  needsFvConfirmation,
  PENALTY_KIND_SHORT,
  penaltyLine,
  resultTimeMs,
  REVIEW_STATUS_LABELS,
  reviewSummary,
  reviewSummaryText,
  runTitle,
  summaryTone,
  takeOverFooter,
  takeOverHeadline,
  takeOverPlan,
  type ReviewSummary,
} from "../lib/runReview";
import { ConfirmDialog } from "./ConfirmDialog";
import { HandTimeDialog } from "./HandTimeDialog";
import { LineupEditor } from "./LineupEditor";

export interface ReviewActions {
  reviewPenalty: (penaltyId: string, approve: boolean) => Promise<boolean>;
  /** „Rückgängig“: decided → pending (until „Übernehmen“). */
  undoReview: (penaltyId: string) => Promise<boolean>;
  takeOver: (runId: string) => Promise<boolean>;
  setLineup: (runId: string, runnerIds: string[]) => Promise<boolean>;
  setHandTimes: (runId: string, t1: number, t2: number) => Promise<boolean>;
  clearHandTimes: (runId: string) => Promise<boolean>;
}

interface Props {
  snapshot: DisciplinePayload;
  run: Run;
  /** Lane label when the run is current ("Bahn 1"). */
  laneLabel?: string | null;
  catalogue: Map<string, CatalogueItem>;
  actions: ReviewActions;
  /** Penalties whose decision comes from the web snapshot — no „Rückgängig“. */
  webDecided?: ReadonlySet<string>;
}

const NO_IDS: ReadonlySet<string> = new Set();

/**
 * Organizer review of one run (on-prem, desktop) — same workflow and
 * wording as the web's "Läufe & Prüfung": errors by station with
 * Bestätigen / Zurücknehmen, rejected crossed out, FV with confirmation,
 * "Übernehmen", live summary, hand times and lineup correction.
 */
export function RunReviewCard({
  snapshot,
  run,
  laneLabel,
  catalogue,
  actions,
  webDecided = NO_IDS,
}: Props) {
  const penalties = useMemo(() => penaltiesForRun(snapshot, run.id), [snapshot, run.id]);
  const summary = useMemo(() => reviewSummary(penalties), [penalties]);
  const groups = useMemo(() => groupByStation(penalties, catalogue), [penalties, catalogue]);
  const title = runTitle(snapshot, run);
  const roster = useMemo(
    () => snapshot.runners.filter((r) => r.team_id === run.team_id),
    [snapshot.runners, run.team_id],
  );
  const [busyId, setBusyId] = useState<string | null>(null);
  const [fvConfirm, setFvConfirm] = useState<Penalty | null>(null);
  const [takeOverOpen, setTakeOverOpen] = useState(false);
  const [handOpen, setHandOpen] = useState(false);
  const [lineupOpen, setLineupOpen] = useState(false);

  const review = async (p: Penalty, approve: boolean) => {
    setBusyId(p.id);
    await actions.reviewPenalty(p.id, approve);
    setBusyId(null);
  };

  const undo = async (p: Penalty) => {
    setBusyId(p.id);
    await actions.undoReview(p.id);
    setBusyId(null);
  };

  const confirmClick = (p: Penalty) => {
    if (needsFvConfirmation(p)) setFvConfirm(p);
    else void review(p, true);
  };

  const result = resultTimeMs(run, summary);
  const lineupNames = run.lineup
    .map((id) => roster.find((r) => r.id === id))
    .filter((r) => r != null)
    .map((r) => `${r.first_name} ${r.last_name}`);

  return (
    <article className="flex flex-col gap-3 rounded-2xl border border-ink-50/[0.06] bg-ink-50/[0.02] px-5 py-4">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          {laneLabel && <span className="label">{laneLabel}</span>}
          <h3 className="font-display text-[17px] font-medium tracking-tight text-ink-50">{title}</h3>
        </div>
        <SummaryBadge summary={summary} />
      </header>

      <div className="grid grid-cols-2 gap-3 md:grid-cols-3">
        <div>
          <span className="label">{t.review.time}</span>
          <p className="font-mono tnum text-[16px] text-ink-50">
            {run.original_time_ms != null ? `${formatResultTime(run.original_time_ms)} s` : t.review.noTime}
            {isHandTime(run) && (
              <span className="ml-2 font-mono text-[10px] uppercase tracking-[0.18em] text-status-armed">
                {t.review.handTime}
              </span>
            )}
          </p>
          {handTimeRawText(run) && (
            <p className="font-mono text-[11px] text-ink-200/70">{handTimeRawText(run)}</p>
          )}
        </div>
        <div>
          <span className="label">{t.review.result}</span>
          <p className="font-mono tnum text-[16px] text-ink-50">
            {result != null ? `${formatResultTime(result)} s` : summary.valid ? "—" : "FV"}
          </p>
        </div>
        <div className="col-span-2 flex items-end gap-2 md:col-span-1 md:justify-end">
          <button type="button" className="btn" onClick={() => setHandOpen(true)}>
            {t.review.handTimeButton}
          </button>
          <button type="button" className="btn" onClick={() => setLineupOpen(true)}>
            {t.review.lineupButton}
          </button>
        </div>
      </div>

      <div>
        <span className="label">{t.review.lineup}</span>
        {run.lineup.length === 0 ? (
          <p className="font-mono text-[12px] text-status-armed">⚠ {t.review.lineupMissing}</p>
        ) : (
          <p className="text-[13px] text-ink-100">
            {lineupNames.join(", ")}
            {run.lineup_confirmed_by && (
              <span className="ml-2 font-mono text-[10px] uppercase tracking-[0.18em] text-ink-200/60">
                {t.review.lineupBy(run.lineup_confirmed_by)}
              </span>
            )}
          </p>
        )}
      </div>

      <div className="flex flex-col gap-3 border-t border-ink-50/[0.06] pt-3">
        <span className="label">{t.review.errors(penalties.length)}</span>
        {groups.length === 0 ? (
          <p className="text-[13px] text-ink-200">{t.review.none}</p>
        ) : (
          groups.map((g) => (
            <div key={g.station ?? "none"} className="flex flex-col gap-1">
              <span className="font-mono text-[11px] uppercase tracking-[0.14em] text-ink-200">{g.label}</span>
              <ul className="flex flex-col gap-1">
                {g.items.map((p) => (
                  <PenaltyRow
                    key={p.id}
                    penalty={p}
                    catalogue={catalogue}
                    busy={busyId === p.id}
                    undoable={canUndoOnDesktop(p, run, webDecided)}
                    onConfirm={() => confirmClick(p)}
                    onWithdraw={() => void review(p, false)}
                    onUndo={() => void undo(p)}
                  />
                ))}
              </ul>
            </div>
          ))
        )}
      </div>

      <footer className="flex flex-wrap items-center justify-between gap-2 border-t border-ink-50/[0.06] pt-3">
        <span className="font-mono text-[11px] text-ink-200">{takeOverFooter(summary)}</span>
        <button type="button" className="btn btn-primary" onClick={() => setTakeOverOpen(true)}>
          {t.review.takeOver}
          {summary.pending > 0 ? ` (${summary.pending})` : ""}
        </button>
      </footer>

      {fvConfirm && (
        <ConfirmDialog
          title={t.review.fvTitle}
          tone="danger"
          confirmLabel={t.review.fvConfirm}
          cancelLabel={t.review.cancel}
          busy={busyId === fvConfirm.id}
          onCancel={() => setFvConfirm(null)}
          onConfirm={async () => {
            await review(fvConfirm, true);
            setFvConfirm(null);
          }}
        >
          <p className="font-semibold text-ink-50">{penaltyLine(fvConfirm, catalogue)}</p>
          <p>{t.review.fvBody(title)}</p>
        </ConfirmDialog>
      )}

      {takeOverOpen && (
        <TakeOverDialog
          title={title}
          penalties={penalties}
          catalogue={catalogue}
          onCancel={() => setTakeOverOpen(false)}
          onConfirm={() => actions.takeOver(run.id)}
        />
      )}

      {handOpen && (
        <HandTimeDialog
          run={run}
          title={title}
          onClose={() => setHandOpen(false)}
          onSave={(a, b) => actions.setHandTimes(run.id, a, b)}
          onClear={() => actions.clearHandTimes(run.id)}
        />
      )}

      {lineupOpen && (
        <LineupEditor
          run={run}
          title={title}
          roster={roster}
          onClose={() => setLineupOpen(false)}
          onSave={(ids) => actions.setLineup(run.id, ids)}
        />
      )}
    </article>
  );
}

export function SummaryBadge({ summary }: { summary: ReviewSummary }) {
  const tone = summaryTone(summary);
  const cls =
    tone === "invalid"
      ? "bg-status-unknown/15 text-status-unknown ring-status-unknown/30"
      : tone === "provisional"
        ? "bg-status-armed/15 text-status-armed ring-status-armed/30"
        : tone === "penalty"
          ? "bg-signal/10 text-signal-glow ring-signal/30"
          : "bg-status-confirmed/15 text-status-confirmed ring-status-confirmed/30";
  return (
    <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 font-mono text-[11px] ring-1 ${cls}`}>
      {tone === "provisional" && <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-current" />}
      {reviewSummaryText(summary)}
    </span>
  );
}

export function PenaltyRow({
  penalty: p,
  catalogue,
  busy,
  undoable = false,
  onConfirm,
  onWithdraw,
  onUndo,
}: {
  penalty: Penalty;
  catalogue: Map<string, CatalogueItem>;
  busy: boolean;
  /** Decided and not frozen by „Übernehmen“ → show „Rückgängig“. */
  undoable?: boolean;
  onConfirm: () => void;
  onWithdraw: () => void;
  onUndo?: () => void;
}) {
  const rejected = p.status === "rejected";
  const fv = p.kind === "fv";
  return (
    <li
      className={[
        "flex flex-wrap items-center gap-2 rounded-md border-l-4 px-3 py-2",
        fv && !rejected
          ? "border-status-unknown bg-status-unknown/10"
          : p.status === "pending"
            ? "border-status-armed bg-status-armed/5"
            : "border-transparent bg-ink-50/[0.03]",
        rejected ? "opacity-60" : "",
      ].join(" ")}
    >
      <span
        className={[
          "rounded px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-[0.14em]",
          fv ? "bg-status-unknown text-white" : "bg-status-armed/15 text-status-armed",
          rejected ? "line-through" : "",
        ].join(" ")}
        title={fv ? t.review.fv : t.review.sz}
      >
        {PENALTY_KIND_SHORT[p.kind]} {fv ? "ungültig" : "+15 s"}
      </span>
      <span
        className={[
          "min-w-0 flex-1 text-[13px]",
          rejected ? "text-ink-300 line-through" : fv ? "font-semibold text-status-unknown" : "text-ink-50",
        ].join(" ")}
        title={p.code ?? p.reason_code ?? undefined}
      >
        {penaltyLabel(p, catalogue)}
      </span>
      <span className="font-mono text-[10px] text-ink-300" title={t.review.recordedAt}>
        {formatClock(p.created_at)}
      </span>
      {canReview(p) ? (
        <span className="inline-flex gap-1">
          <button
            type="button"
            className={fv ? "btn btn-danger" : "btn btn-primary"}
            disabled={busy}
            onClick={onConfirm}
          >
            {t.review.confirm}
          </button>
          <button type="button" className="btn" disabled={busy} onClick={onWithdraw}>
            {t.review.withdraw}
          </button>
        </span>
      ) : (
        <span className="inline-flex items-center gap-1">
          <span
            className={[
              "rounded px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-[0.14em]",
              p.status === "approved" ? "bg-status-confirmed/15 text-status-confirmed" : "bg-ink-50/5 text-ink-300",
            ].join(" ")}
            title={p.reviewed_by ?? undefined}
          >
            {REVIEW_STATUS_LABELS[p.status]}
          </span>
          {undoable && onUndo && (
            <button
              type="button"
              className="btn px-2 py-1 text-[10px]"
              disabled={busy}
              onClick={onUndo}
              title={t.review.undoHint}
            >
              {t.review.undo}
            </button>
          )}
        </span>
      )}
    </li>
  );
}

function TakeOverDialog({
  title,
  penalties,
  catalogue,
  onCancel,
  onConfirm,
}: {
  title: string;
  penalties: Penalty[];
  catalogue: Map<string, CatalogueItem>;
  onCancel: () => void;
  onConfirm: () => Promise<boolean>;
}) {
  const plan = useMemo(() => takeOverPlan(penalties), [penalties]);
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  return (
    <ConfirmDialog
      title={t.review.takeOverTitle(title)}
      tone={plan.needsFvAck ? "danger" : "default"}
      confirmLabel={t.review.takeOver}
      cancelLabel={t.review.cancel}
      busy={busy}
      confirmDisabled={plan.needsFvAck && !ack}
      onCancel={onCancel}
      onConfirm={async () => {
        setBusy(true);
        const ok = await onConfirm();
        setBusy(false);
        if (ok) onCancel();
      }}
    >
      <p className="font-semibold text-ink-50">{takeOverHeadline(plan)}</p>
      {plan.pending.length > 0 && (
        <ul className="flex flex-col gap-1">
          {plan.pending.map((p) => (
            <li
              key={p.id}
              className={
                p.kind === "fv"
                  ? "rounded border-l-4 border-status-unknown bg-status-unknown/10 px-2 py-1 font-semibold text-status-unknown"
                  : "px-2 py-1"
              }
            >
              • {penaltyLine(p, catalogue)}
            </li>
          ))}
        </ul>
      )}
      <p>
        {t.review.takeOverAfter} <strong>{reviewSummaryText(plan.after)}</strong>
      </p>
      {plan.needsFvAck && (
        <label className="flex items-start gap-2 rounded-md bg-status-unknown/10 p-3 text-status-unknown">
          <input type="checkbox" className="mt-0.5 h-4 w-4" checked={ack} onChange={(e) => setAck(e.target.checked)} />
          <span>{t.review.takeOverFvAck}</span>
        </label>
      )}
    </ConfirmDialog>
  );
}

function formatClock(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}
