// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useMemo, useState } from 'react';

import {
  apiClient,
  type Penalty,
  type Run,
  type Runner,
  type Team,
  type TeamEntry,
} from '../../lib/api';
import { formatClock, LINEUP_MISSING_TEXT, lineupConfirmationText, lineupMissing } from '../../lib/lineup';
import {
  PENALTY_KIND_SHORT,
  type CatalogueIndex,
  penaltyDescription,
  penaltyKind,
} from '../../lib/penalties';
import {
  REVIEW_STATUS_LABELS,
  canReview,
  canUndoReview,
  groupByStation,
  needsFvConfirmation,
  penaltyLine,
  summarizeRun,
  summaryText,
  summaryTone,
  takeOverHeadline,
  takeOverPlan,
  type RunSummary,
} from '../../lib/run-review';
import { runStatusClasses } from '../../lib/runs';
import { RUN_STATUS_LABELS } from '../../lib/api';
import { wgLabel, WG_MISSING_LABEL } from '../../lib/wg';
import HandTimeDialog from './HandTimeDialog';
import LineupEditor from './LineupEditor';
import { ConfirmDialog } from './Modal';
import { RunTime } from './RunTime';

interface Props {
  eventId: string;
  disciplineId: string;
  run: Run;
  team: Team | undefined;
  entry: TeamEntry | undefined;
  roster: Runner[];
  penalties: Penalty[];
  catalogue: CatalogueIndex;
  /** "Bahn 1" when the run is current. */
  laneLabel?: string;
  highlight?: boolean;
  onPenaltyUpdated: (p: Penalty) => void;
  onRunUpdated: (r: Run) => void;
  /** After "Übernehmen": reload penalties. */
  onTakenOver: (runId: string) => Promise<void> | void;
  onError: (msg: string) => void;
}

export type ReviewAction = 'approve' | 'reject' | 'undo';

/** Calls the review endpoint for one penalty ("Bestätigen" / "Zurücknehmen" / "Rückgängig"). */
export function runReviewAction(eventId: string, penaltyId: string, action: ReviewAction): Promise<Penalty> {
  switch (action) {
    case 'approve':
      return apiClient.penalties.approve(eventId, penaltyId);
    case 'reject':
      return apiClient.penalties.reject(eventId, penaltyId);
    case 'undo':
      return apiClient.penalties.undo(eventId, penaltyId);
  }
}

export default function RunReviewCard({
  eventId,
  disciplineId,
  run,
  team,
  entry,
  roster,
  penalties,
  catalogue,
  laneLabel,
  highlight = false,
  onPenaltyUpdated,
  onRunUpdated,
  onTakenOver,
  onError,
}: Props) {
  const [busyId, setBusyId] = useState<string | null>(null);
  const [fvConfirm, setFvConfirm] = useState<Penalty | null>(null);
  const [takeOverOpen, setTakeOverOpen] = useState(false);
  const [handOpen, setHandOpen] = useState(false);
  const [lineupOpen, setLineupOpen] = useState(false);
  const [takenOverAt, setTakenOverAt] = useState<string | null>(null);

  const summary = useMemo(() => summarizeRun(penalties), [penalties]);
  const groups = useMemo(() => groupByStation(penalties, catalogue), [penalties, catalogue]);
  const title = `Lauf ${run.run_number} · ${team?.name ?? 'Unbekannte Mannschaft'}`;

  const review = async (p: Penalty, action: ReviewAction) => {
    setBusyId(p.id);
    try {
      const updated = await runReviewAction(eventId, p.id, action);
      onPenaltyUpdated(updated);
    } catch (e) {
      onError(e instanceof Error ? e.message : 'Prüfung fehlgeschlagen');
    } finally {
      setBusyId(null);
    }
  };

  const confirmClick = (p: Penalty) => {
    if (needsFvConfirmation(p)) setFvConfirm(p);
    else void review(p, 'approve');
  };

  return (
    <article
      className={[
        'surface flex flex-col gap-4 p-5',
        highlight ? 'ring-1 ring-signal/40' : '',
        !summary.valid ? 'border-status-unknown/40' : '',
      ].join(' ')}
    >
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex flex-col gap-1">
          <span className="label">
            {laneLabel ? `${laneLabel} · ` : ''}Lauf {run.run_number}
            {run.lane != null && !laneLabel ? ` · Bahn ${run.lane}` : ''}
          </span>
          <h3 className="font-display text-[18px] font-semibold text-ink-50">
            {team?.name ?? 'Unbekannte Mannschaft'}
          </h3>
          <span className="font-mono text-[11px] text-ink-200">
            {entry ? `Startnr. ${entry.start_number}` : 'nicht gemeldet'} ·{' '}
            {entry ? wgLabel(entry.wg, WG_MISSING_LABEL) : WG_MISSING_LABEL}
          </span>
        </div>
        <div className="flex flex-col items-end gap-2">
          <span className={`pill ${runStatusClasses(run.status)}`}>{RUN_STATUS_LABELS[run.status]}</span>
          <SummaryBadge summary={summary} />
        </div>
      </header>

      {/* Time */}
      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-[color:var(--hairline-color)] pt-3">
        <div className="flex flex-col gap-1">
          <span className="label">Zeit</span>
          <RunTime run={run} summary={summary} />
        </div>
        <button type="button" className="btn" onClick={() => setHandOpen(true)}>
          ⏱ Handzeit …
        </button>
      </div>

      {/* Lineup */}
      <div className="flex flex-col gap-2 border-t border-[color:var(--hairline-color)] pt-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex flex-col gap-1">
            <span className="label">Besetzung</span>
            <span className="text-[13px] text-ink-100">
              {(run.lineup ?? []).length > 0
                ? `${run.lineup.length} Läufer · ${lineupConfirmationText(run)}`
                : 'noch nicht bestätigt'}
            </span>
            {lineupMissing(run) && <LineupMissingBadge />}
          </div>
          <button type="button" className="btn" onClick={() => setLineupOpen((v) => !v)}>
            {lineupOpen ? 'Schließen' : 'Besetzung …'}
          </button>
        </div>
        {lineupOpen && (
          <LineupEditor
            eventId={eventId}
            disciplineId={disciplineId}
            run={run}
            roster={roster}
            onSaved={(r) => {
              onRunUpdated(r);
              setLineupOpen(false);
            }}
            onClose={() => setLineupOpen(false)}
          />
        )}
      </div>

      {/* Errors */}
      <div className="flex flex-col gap-3 border-t border-[color:var(--hairline-color)] pt-3">
        <span className="label">Fehler ({penalties.length})</span>
        {groups.length === 0 ? (
          <p className="text-[13px] text-ink-200">Keine Fehler gemeldet.</p>
        ) : (
          groups.map((g) => (
            <div key={g.station ?? 'none'} className="flex flex-col gap-1">
              <span className="font-mono text-[11px] uppercase tracking-[0.14em] text-ink-200">{g.label}</span>
              <ul className="flex flex-col gap-1">
                {g.items.map((p) => (
                  <PenaltyRow
                    key={p.id}
                    penalty={p}
                    catalogue={catalogue}
                    busy={busyId === p.id}
                    undoable={canUndoReview(p, run)}
                    onConfirm={() => confirmClick(p)}
                    onWithdraw={() => void review(p, 'reject')}
                    onUndo={() => void review(p, 'undo')}
                  />
                ))}
              </ul>
            </div>
          ))
        )}
      </div>

      {/* Übernehmen */}
      <footer className="flex flex-wrap items-center justify-between gap-2 border-t border-[color:var(--hairline-color)] pt-3">
        <span className="font-mono text-[11px] text-ink-200">
          {summary.pending > 0
            ? `${summary.pending} offen – zählen beim Übernehmen als bestätigt`
            : takenOverAt
              ? `Übernommen um ${takenOverAt}`
              : 'Keine offenen Fehler'}
        </span>
        <button type="button" className="btn btn-primary" onClick={() => setTakeOverOpen(true)}>
          Übernehmen{summary.pending > 0 ? ` (${summary.pending})` : ''}
        </button>
      </footer>

      {fvConfirm && (
        <ConfirmDialog
          title="Fehlversuch bestätigen?"
          tone="danger"
          confirmLabel="FV bestätigen – Lauf ungültig"
          busy={busyId === fvConfirm.id}
          onCancel={() => setFvConfirm(null)}
          onConfirm={async () => {
            await review(fvConfirm, 'approve');
            setFvConfirm(null);
          }}
        >
          <p className="font-semibold text-ink-50">{penaltyLine(fvConfirm, catalogue)}</p>
          <p>
            Mit der Bestätigung wird <strong>{title}</strong> ungültig (keine Zeit). Bitte vorher mit
            dem Mannschaftsführer klären.
          </p>
        </ConfirmDialog>
      )}

      {takeOverOpen && (
        <TakeOverDialog
          title={title}
          penalties={penalties}
          catalogue={catalogue}
          onCancel={() => setTakeOverOpen(false)}
          onConfirm={async () => {
            const res = await apiClient.penalties.bulkApprove(eventId, run.id);
            const acceptedAt = res.penalties_accepted_at ?? new Date().toISOString();
            // Freeze decisions made so far (undo rule) until the next refresh.
            onRunUpdated({ ...run, penalties_accepted_at: acceptedAt });
            setTakenOverAt(formatClock(acceptedAt));
            await onTakenOver(run.id);
          }}
          onError={onError}
        />
      )}

      {handOpen && (
        <HandTimeDialog
          eventId={eventId}
          disciplineId={disciplineId}
          run={run}
          title={title}
          onClose={() => setHandOpen(false)}
          onSaved={onRunUpdated}
        />
      )}
    </article>
  );
}

export function SummaryBadge({ summary }: { summary: RunSummary }) {
  const tone = summaryTone(summary);
  const cls =
    tone === 'invalid'
      ? 'bg-status-unknown/15 text-status-unknown'
      : tone === 'provisional'
        ? 'bg-status-armed/15 text-status-armed'
        : tone === 'penalty'
          ? 'bg-signal/10 text-signal-glow'
          : 'bg-status-confirmed/15 text-status-confirmed';
  return (
    <span className={`pill normal-case tracking-normal ${cls}`} title="Live-Zusammenfassung">
      {tone === 'provisional' && <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-current" />}
      {summaryText(summary)}
    </span>
  );
}

export function LineupMissingBadge() {
  return (
    <span className="pill normal-case tracking-normal bg-status-armed/15 text-status-armed" title={LINEUP_MISSING_TEXT}>
      ⚠ {LINEUP_MISSING_TEXT}
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
  catalogue: CatalogueIndex;
  busy: boolean;
  /** Decided and not frozen by "Übernehmen" → show "Rückgängig". */
  undoable?: boolean;
  onConfirm: () => void;
  onWithdraw: () => void;
  onUndo?: () => void;
}) {
  const kind = penaltyKind(p);
  const rejected = p.status === 'rejected';
  const fv = kind === 'fv';
  return (
    <li
      className={[
        'flex flex-wrap items-center gap-2 rounded-md border-l-4 px-3 py-2',
        fv && !rejected
          ? 'border-status-unknown bg-status-unknown/10'
          : p.status === 'pending'
            ? 'border-status-armed bg-status-armed/5'
            : 'border-transparent bg-[color:var(--soft-fill)]',
        rejected ? 'opacity-60' : '',
      ].join(' ')}
    >
      <span
        className={[
          'pill',
          fv ? 'bg-status-unknown text-white' : 'bg-status-armed/15 text-status-armed',
          rejected ? 'line-through' : '',
        ].join(' ')}
        title={fv ? 'Fehlversuch: Lauf ungültig' : 'Strafzeit: +15 s'}
      >
        {PENALTY_KIND_SHORT[kind]}
        <span className="font-mono text-[10px] opacity-80">{fv ? 'ungültig' : '+15 s'}</span>
      </span>
      <span
        className={[
          'min-w-0 flex-1 text-[13px]',
          rejected ? 'text-ink-300 line-through' : fv ? 'font-semibold text-status-unknown' : 'text-ink-50',
        ].join(' ')}
        title={p.code ?? p.reason_code ?? undefined}
      >
        {penaltyDescription(p, catalogue)}
      </span>
      <span className="font-mono text-[10px] text-ink-300" title="Erfasst um">
        {formatClock(p.created_at) ?? ''}
      </span>
      {canReview(p) ? (
        <span className="inline-flex gap-1">
          <button
            type="button"
            className={fv ? 'btn btn-danger' : 'btn btn-confirm'}
            disabled={busy}
            onClick={onConfirm}
          >
            Bestätigen
          </button>
          <button type="button" className="btn" disabled={busy} onClick={onWithdraw}>
            Zurücknehmen
          </button>
        </span>
      ) : (
        <span className="inline-flex items-center gap-1">
          <span
            className={[
              'pill',
              p.status === 'approved' ? 'bg-status-confirmed/15 text-status-confirmed' : 'bg-ink-50/5 text-ink-300',
            ].join(' ')}
            title={p.reviewed_at ? `geprüft ${formatClock(p.reviewed_at) ?? ''}` : undefined}
          >
            {REVIEW_STATUS_LABELS[p.status]}
          </span>
          {undoable && onUndo && (
            <button
              type="button"
              className="btn px-2 py-1 text-[10px]"
              disabled={busy}
              onClick={onUndo}
              title="Entscheidung zurücksetzen – der Fehler ist wieder offen"
            >
              Rückgängig
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
  onError,
}: {
  title: string;
  penalties: Penalty[];
  catalogue: CatalogueIndex;
  onCancel: () => void;
  onConfirm: () => Promise<void>;
  onError: (msg: string) => void;
}) {
  const plan = useMemo(() => takeOverPlan(penalties), [penalties]);
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);

  return (
    <ConfirmDialog
      title={`Lauf übernehmen – ${title}`}
      tone={plan.needsFvAck ? 'danger' : 'default'}
      confirmLabel="Übernehmen"
      busy={busy}
      confirmDisabled={plan.needsFvAck && !ack}
      onCancel={onCancel}
      onConfirm={async () => {
        setBusy(true);
        try {
          await onConfirm();
          onCancel();
        } catch (e) {
          onError(e instanceof Error ? e.message : 'Übernehmen fehlgeschlagen');
          setBusy(false);
        }
      }}
    >
      <p className="font-semibold text-ink-50">{takeOverHeadline(plan)}</p>
      {plan.pending.length > 0 && (
        <ul className="flex flex-col gap-1">
          {plan.pending.map((p) => (
            <li
              key={p.id}
              className={
                penaltyKind(p) === 'fv'
                  ? 'rounded border-l-4 border-status-unknown bg-status-unknown/10 px-2 py-1 font-semibold text-status-unknown'
                  : 'px-2 py-1'
              }
            >
              • {penaltyLine(p, catalogue)}
            </li>
          ))}
        </ul>
      )}
      <p>
        Ergebnis danach: <strong>{summaryText(plan.after)}</strong>
      </p>
      {plan.needsFvAck && (
        <label className="flex items-start gap-2 rounded-md bg-status-unknown/10 p-3 text-status-unknown">
          <input
            type="checkbox"
            className="mt-0.5 h-4 w-4"
            checked={ack}
            onChange={(e) => setAck(e.target.checked)}
          />
          <span>
            Ich bestätige den Fehlversuch (mit dem Mannschaftsführer geklärt) – der Lauf wird
            ungültig.
          </span>
        </label>
      )}
    </ConfirmDialog>
  );
}
