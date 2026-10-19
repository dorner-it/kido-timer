import { useEffect, useMemo, useState } from "react";
import { handTimeMean } from "../lib/cloudApi";
import type { Run } from "../lib/cloudTypes";
import { formatTimeInput, isHandTime, validateHandTimes } from "../lib/handTime";
import { t } from "../lib/i18n";
import { ConfirmDialog } from "./ConfirmDialog";

interface Props {
  run: Run;
  title: string;
  onClose: () => void;
  onSave: (t1: number, t2: number) => Promise<boolean>;
  onClear: () => Promise<boolean>;
}

/**
 * Hand-time entry (light barrier failed): two stopwatch readings → mean
 * via `kido_core::timing::hand_time_mean`, stored as "Handzeit".
 */
export function HandTimeDialog({ run, title, onClose, onSave, onClear }: Props) {
  const [t1, setT1] = useState(formatTimeInput(run.hand_time_1_ms));
  const [t2, setT2] = useState(formatTimeInput(run.hand_time_2_ms));
  const [touched, setTouched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [mean, setMean] = useState<number | null>(null);
  const v = useMemo(() => validateHandTimes(t1, t2), [t1, t2]);
  const hand = isHandTime(run);

  useEffect(() => {
    let alive = true;
    if (!v.ok) {
      setMean(null);
      return;
    }
    void handTimeMean(v.t1, v.t2).then((m) => {
      if (alive) setMean(m);
    });
    return () => {
      alive = false;
    };
  }, [v]);

  const save = async () => {
    setTouched(true);
    if (!v.ok) return;
    setBusy(true);
    const ok = await onSave(v.t1, v.t2);
    setBusy(false);
    if (ok) onClose();
  };

  const clear = async () => {
    setBusy(true);
    const ok = await onClear();
    setBusy(false);
    if (ok) onClose();
  };

  const fieldError = (field: "t1" | "t2", value: string) =>
    !v.ok && (touched || value.trim() !== "") ? v.errors[field] : undefined;

  return (
    <ConfirmDialog
      title={t.handTime.title(title)}
      confirmLabel={busy ? t.handTime.saving : t.handTime.save}
      cancelLabel={t.handTime.cancel}
      busy={busy}
      confirmDisabled={!v.ok}
      onCancel={onClose}
      onConfirm={save}
      extraActions={
        hand ? (
          <button type="button" className="btn btn-danger mr-auto" disabled={busy} onClick={() => void clear()}>
            {t.handTime.clear}
          </button>
        ) : undefined
      }
    >
      <p className="text-[12px] text-ink-200">{t.handTime.intro}</p>
      {!hand && run.original_time_ms != null && (
        <p className="font-mono text-[12px] text-ink-200">
          {t.handTime.lightBarrier(formatTimeInput(run.original_time_ms))}
        </p>
      )}
      <form
        className="grid grid-cols-2 gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        {(["t1", "t2"] as const).map((field) => {
          const value = field === "t1" ? t1 : t2;
          const err = fieldError(field, value);
          return (
            <label key={field} className="flex flex-col gap-1">
              <span className="label">{field === "t1" ? t.handTime.watch1 : t.handTime.watch2}</span>
              <input
                className="rounded-lg border border-ink-50/10 bg-ink-50/[0.03] px-3 py-2 font-mono tnum text-[16px] text-ink-50"
                inputMode="decimal"
                placeholder={field === "t1" ? "47,23" : "47,25"}
                value={value}
                autoFocus={field === "t1"}
                onChange={(e) => (field === "t1" ? setT1 : setT2)(e.target.value)}
              />
              {err && <span className="text-[12px] text-status-unknown">{err}</span>}
            </label>
          );
        })}
        <button type="submit" className="hidden" aria-hidden="true" tabIndex={-1} />
      </form>
      <div className="flex items-baseline justify-between rounded-lg border border-ink-50/[0.06] bg-ink-50/[0.03] px-4 py-3">
        <span className="label">{t.handTime.mean}</span>
        <span className="font-stencil text-[28px] font-bold tnum text-ink-50">
          {mean != null ? formatTimeInput(mean) : "—"}
        </span>
      </div>
      {v.ok && v.warning && <p className="text-[12px] text-status-armed">⚠ {v.warning}</p>}
      {hand && <p className="text-[11px] text-ink-300">{t.handTime.clearHint}</p>}
    </ConfirmDialog>
  );
}
