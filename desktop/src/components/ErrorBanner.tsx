import { t } from "../lib/i18n";

interface Props {
  message: string | null;
  onDismiss: () => void;
}

export function ErrorBanner({ message, onDismiss }: Props) {
  if (!message) return null;
  return (
    <div className="fixed bottom-6 left-1/2 z-40 -translate-x-1/2 max-w-[60ch] animate-[fade_0.18s_ease]">
      <div className="surface flex items-start gap-3 border-status-unknown/40 bg-status-unknown/10 px-4 py-3 ring-1 ring-status-unknown/30">
        <span className="mt-0.5 font-stencil text-[14px] text-status-unknown">!</span>
        <div className="flex-1">
          <p className="font-display text-[12px] font-medium text-ink-50">{t.errors.title}</p>
          <p className="font-mono text-[11px] text-ink-100/80 break-words">{message}</p>
        </div>
        <button
          type="button"
          className="font-mono text-[10px] uppercase tracking-[0.2em] text-ink-200 hover:text-ink-50"
          onClick={onDismiss}
        >
          {t.errors.dismiss}
        </button>
      </div>
    </div>
  );
}
