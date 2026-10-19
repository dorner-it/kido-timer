import { useState } from "react";
import { t } from "../lib/i18n";
import type { DisciplinePayload, DivergenceState, RunStatus } from "../lib/cloudTypes";
import { formatResultTime } from "../lib/results";

interface Props {
  divergence: DivergenceState;
  snapshot: DisciplinePayload | null;
  onUpload: () => Promise<void>;
  onDiscard: () => Promise<void>;
}

/**
 * Shown after switching back to live when local changes differ from the
 * web. Nothing is overwritten silently: the operator uploads everything
 * in one on-prem upload (A3: run times, errors incl. status, lineups,
 * hand times) or discards the local changes.
 */
export function DivergenceDialog({ divergence, snapshot, onUpload, onDiscard }: Props) {
  const [busy, setBusy] = useState(false);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const { divergences, unsynced } = divergence;
  const extras = t.divergence.extras(unsynced.penalties, unsynced.lineups, unsynced.handTimes);
  if (divergences.length === 0 && !extras) return null;

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
    } finally {
      setBusy(false);
      setConfirmDiscard(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 grid place-items-center px-6">
      <div className="absolute inset-0 bg-ink-900/85 backdrop-blur-md" />
      <div className="relative z-10 w-full max-w-[620px] surface overflow-hidden">
        <header className="border-b border-ink-50/[0.06] px-7 pt-6 pb-5">
          <span className="label">{t.cloud.title}</span>
          <h2 className="mt-1 font-display text-[22px] font-medium tracking-tight text-ink-50">
            {t.divergence.title}
          </h2>
          <p className="mt-2 font-mono text-[12px] text-ink-200/80">{t.divergence.body}</p>
        </header>

        <div className="max-h-[45vh] overflow-y-auto px-7 py-4">
          {divergences.length > 0 && (
          <table className="w-full font-mono text-[12px]">
            <thead>
              <tr className="text-left text-ink-200/70">
                <th className="py-1 pr-3 font-normal">{t.divergence.run}</th>
                <th className="py-1 pr-3 font-normal">{t.divergence.local}</th>
                <th className="py-1 font-normal">{t.divergence.web}</th>
              </tr>
            </thead>
            <tbody>
              {divergences.map((d) => (
                <tr key={d.runId} className="border-t border-ink-50/[0.05] text-ink-100">
                  <td className="py-1.5 pr-3">
                    {d.runNumber} · {teamName(snapshot, d.teamId)}
                  </td>
                  <td className="py-1.5 pr-3">{describe(d.localStatus, d.localTimeMs)}</td>
                  <td className="py-1.5 text-ink-200/80">
                    {d.remoteStatus
                      ? describe(d.remoteStatus, d.remoteTimeMs)
                      : t.divergence.missing}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          )}
          {extras && <p className="mt-3 font-mono text-[12px] text-ink-100">{extras}</p>}
          <p className="mt-3 font-mono text-[10px] text-ink-200/60">{t.divergence.uploadNote}</p>
        </div>

        <footer className="flex flex-col gap-2 border-t border-ink-50/[0.05] bg-ink-900/40 px-7 py-4">
          {confirmDiscard ? (
            <div className="rounded-xl border border-status-unknown/40 bg-status-unknown/5 p-3">
              <p className="font-mono text-[12px] text-ink-100">{t.divergence.discardConfirm}</p>
              <div className="mt-3 grid grid-cols-2 gap-2">
                <button className="btn" disabled={busy} onClick={() => setConfirmDiscard(false)}>
                  {t.cloud.cancel}
                </button>
                <button className="btn btn-danger" disabled={busy} onClick={() => run(onDiscard)}>
                  {t.divergence.discardConfirmAction}
                </button>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-end gap-2">
              <button className="btn btn-danger" disabled={busy} onClick={() => setConfirmDiscard(true)}>
                {t.divergence.discard}
              </button>
              <button className="btn btn-primary" disabled={busy} onClick={() => run(onUpload)}>
                {busy ? t.divergence.busy : t.divergence.upload}
              </button>
            </div>
          )}
        </footer>
      </div>
    </div>
  );
}

function teamName(snapshot: DisciplinePayload | null, teamId: string | null): string {
  if (!snapshot || !teamId) return "?";
  return snapshot.teams.find((tm) => tm.id === teamId)?.name ?? "?";
}

function describe(status: RunStatus, timeMs: number | null): string {
  const label =
    status === "completed"
      ? "beendet"
      : status === "active"
        ? "aktiv"
        : status === "cancelled"
          ? "abgebrochen"
          : "wartend";
  return timeMs != null ? `${label} · ${formatResultTime(timeMs)} s` : label;
}
