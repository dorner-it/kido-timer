import type { ReactNode } from "react";

interface Props {
  title: string;
  kicker?: string;
  tone?: "default" | "danger";
  confirmLabel: string;
  cancelLabel?: string;
  busy?: boolean;
  confirmDisabled?: boolean;
  onCancel: () => void;
  onConfirm: () => void | Promise<void>;
  children?: ReactNode;
  /** Extra buttons left of "Abbrechen" (e.g. "Handzeit entfernen"). */
  extraActions?: ReactNode;
}

/** Modal dialog in the app's surface style. */
export function ConfirmDialog({
  title,
  kicker,
  tone = "default",
  confirmLabel,
  cancelLabel = "Abbrechen",
  busy = false,
  confirmDisabled = false,
  onCancel,
  onConfirm,
  children,
  extraActions,
}: Props) {
  return (
    <div className="fixed inset-0 z-50 grid place-items-center px-6" role="dialog" aria-modal="true">
      <div className="absolute inset-0 bg-ink-900/85 backdrop-blur-md" onClick={busy ? undefined : onCancel} />
      <div className="relative z-10 w-full max-w-[560px] surface overflow-hidden">
        <header className="border-b border-ink-50/[0.06] px-7 pt-6 pb-4">
          {kicker && <span className="label">{kicker}</span>}
          <h2 className="mt-1 font-display text-[20px] font-medium tracking-tight text-ink-50">{title}</h2>
        </header>
        <div className="flex max-h-[55vh] flex-col gap-3 overflow-y-auto px-7 py-4 text-[13px] text-ink-100">
          {children}
        </div>
        <footer className="flex items-center justify-end gap-2 border-t border-ink-50/[0.05] bg-ink-900/40 px-7 py-4">
          {extraActions}
          <button type="button" className="btn" disabled={busy} onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            type="button"
            className={tone === "danger" ? "btn btn-danger" : "btn btn-primary"}
            disabled={busy || confirmDisabled}
            onClick={() => void onConfirm()}
          >
            {confirmLabel}
          </button>
        </footer>
      </div>
    </div>
  );
}
