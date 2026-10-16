import { t } from "../lib/i18n";
import type { KidoConflict } from "../lib/cloudTypes";

interface Props {
  conflict: KidoConflict | null;
  busy?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}

export function KidoConflictDialog({ conflict, busy, onCancel, onConfirm }: Props) {
  if (!conflict) return null;
  return (
    <div className="fixed inset-0 z-50 grid place-items-center px-6">
      <div
        className="absolute inset-0 bg-ink-900/85 backdrop-blur-md"
        onClick={() => !busy && onCancel()}
      />
      <div className="relative z-10 w-full max-w-[520px] surface overflow-hidden">
        <header className="border-b border-ink-50/[0.06] px-7 pt-6 pb-5">
          <span className="label">{t.cloud.title}</span>
          <h2 className="mt-1 font-display text-[22px] font-medium tracking-tight text-ink-50">
            {t.cloud.kidoConflictTitle}
          </h2>
        </header>

        <div className="flex flex-col gap-3 px-7 py-5">
          <div className="rounded-xl border border-ink-50/[0.05] bg-ink-700/40 p-3">
            <span className="label">{t.cloud.kidoConflictBodyPrefix}</span>
            <p className="mt-1 font-display text-[14px] text-ink-50">
              {conflict.currentDisciplineName}
            </p>
          </div>
          <div className="rounded-xl border border-signal/30 bg-signal/5 p-3">
            <span className="label">{t.cloud.kidoConflictBodyToPrefix}</span>
            <p className="mt-1 font-display text-[14px] text-ink-50">
              {conflict.newDisciplineName}
            </p>
          </div>
        </div>

        <footer className="flex items-center justify-end gap-2 border-t border-ink-50/[0.05] bg-ink-900/40 px-7 py-4">
          <button className="btn" onClick={onCancel} disabled={busy}>
            {t.cloud.kidoConflictKeep}
          </button>
          <button className="btn btn-danger" onClick={onConfirm} disabled={busy}>
            {t.cloud.kidoConflictReplace}
          </button>
        </footer>
      </div>
    </div>
  );
}
