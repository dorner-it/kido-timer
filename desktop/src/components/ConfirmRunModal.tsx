import { useEffect } from "react";
import { t } from "../lib/i18n";
import { formatCorrection, formatTime } from "../lib/format";
import type { RunEntry } from "../lib/types";

export type ConfirmRunStep = "confirm" | "askReset";

interface Props {
  open: boolean;
  step: ConfirmRunStep;
  entries: Map<number, RunEntry>;
  onCancel: () => void;
  onConfirm: () => void;
  onSkipReset: () => void;
  onAcceptReset: () => void;
}

/**
 * Two-step confirmation modal for the central "Lauf übernehmen" action.
 *
 * Step 1 — `confirm`: lists the pending lanes + times so the operator can
 *   double-check before posting. Cancel keeps the queue, Confirm posts and
 *   advances to step 2.
 *
 * Step 2 — `askReset`: after the times went out, offer to send the
 *   hardware reset so the device is ready for the next heat. Either choice
 *   closes the modal.
 */
export function ConfirmRunModal({
  open,
  step,
  entries,
  onCancel,
  onConfirm,
  onSkipReset,
  onAcceptReset,
}: Props) {
  // Escape closes whichever step we're in.
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (step === "confirm") onCancel();
      else onSkipReset();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, step, onCancel, onSkipReset]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 grid place-items-center px-6">
      <div
        className="absolute inset-0 bg-ink-900/85 backdrop-blur-md"
        onClick={() => (step === "confirm" ? onCancel() : onSkipReset())}
      />
      <div className="relative z-10 w-full max-w-[560px] surface overflow-hidden">
        {step === "confirm" ? (
          <ConfirmStep
            entries={entries}
            onCancel={onCancel}
            onConfirm={onConfirm}
          />
        ) : (
          <ResetStep
            onSkip={onSkipReset}
            onAccept={onAcceptReset}
          />
        )}
      </div>
    </div>
  );
}

function ConfirmStep({
  entries,
  onCancel,
  onConfirm,
}: {
  entries: Map<number, RunEntry>;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const sorted = Array.from(entries.entries()).sort((a, b) => a[0] - b[0]);

  return (
    <>
      <header className="border-b border-ink-50/[0.06] px-7 pt-6 pb-5">
        <span className="label">{t.confirmModal.title}</span>
        <h2 className="mt-1 font-display text-[22px] font-medium tracking-tight text-ink-50">
          {t.confirmModal.subtitle}
        </h2>
      </header>

      <div className="max-h-[50vh] overflow-y-auto px-7 py-5">
        {sorted.length === 0 ? (
          <p className="font-mono text-[12px] text-ink-200/80">
            {t.confirmModal.noEntries}
          </p>
        ) : (
          <ul className="flex flex-col gap-2">
            {sorted.map(([lane, entry]) => {
              const total = entry.originalTimeMs + entry.correctionMs;
              return (
                <li
                  key={lane}
                  className="flex items-center justify-between gap-4 rounded-xl border border-ink-50/[0.05] bg-ink-50/[0.02] px-4 py-3"
                >
                  <span className="font-stencil text-[20px] font-bold tracking-tight text-ink-50">
                    <span className="text-ink-300/80">
                      {t.confirmModal.laneShort}
                    </span>
                    <span className="ml-1">{lane}</span>
                  </span>
                  <div className="flex flex-col items-end leading-tight">
                    <span className="font-mono tnum text-[18px] tracking-wider text-ink-50">
                      {formatTime(total, "confirmed")}
                    </span>
                    {entry.correctionMs !== 0 && (
                      <span className="font-mono text-[10px] tracking-wider text-status-armed">
                        {t.lanes.original}{" "}
                        {formatTime(entry.originalTimeMs, "confirmed")} ·{" "}
                        {formatCorrection(entry.correctionMs)}
                      </span>
                    )}
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      <footer className="flex items-center justify-end gap-2 border-t border-ink-50/[0.05] bg-ink-900/40 px-7 py-4">
        <button className="btn" onClick={onCancel}>
          {t.confirmModal.cancel}
        </button>
        <button
          className="btn btn-primary"
          onClick={onConfirm}
          disabled={sorted.length === 0}
        >
          {t.confirmModal.confirm}
        </button>
      </footer>
    </>
  );
}

function ResetStep({
  onSkip,
  onAccept,
}: {
  onSkip: () => void;
  onAccept: () => void;
}) {
  return (
    <>
      <header className="border-b border-ink-50/[0.06] px-7 pt-6 pb-5">
        <span className="label">{t.confirmModal.title}</span>
        <h2 className="mt-1 font-display text-[22px] font-medium tracking-tight text-ink-50">
          {t.confirmModal.resetTitle}
        </h2>
      </header>

      <div className="px-7 py-5">
        <p className="font-mono text-[13px] leading-relaxed text-ink-100">
          {t.confirmModal.resetBody}
        </p>
      </div>

      <footer className="flex items-center justify-end gap-2 border-t border-ink-50/[0.05] bg-ink-900/40 px-7 py-4">
        <button className="btn" onClick={onSkip}>
          {t.confirmModal.resetSkip}
        </button>
        <button className="btn btn-danger" onClick={onAccept}>
          {t.confirmModal.resetAccept}
        </button>
      </footer>
    </>
  );
}
