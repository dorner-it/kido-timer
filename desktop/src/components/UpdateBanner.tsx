import { t } from "../lib/i18n";
import type { UpdateState } from "../lib/useUpdater";

interface Props {
  state: UpdateState;
  onInstall: () => void;
  onDismiss: () => void;
}

/**
 * Non-blocking banner pinned to the top-right corner. Stays out of the way
 * of the lane view; operator decides when to install.
 */
export function UpdateBanner({ state, onInstall, onDismiss }: Props) {
  if (state.kind === "idle" || state.kind === "checking" || state.kind === "dismissed") {
    return null;
  }

  return (
    <div className="fixed right-6 top-20 z-30 max-w-[360px] animate-[fade_0.2s_ease]">
      <div className="surface flex items-start gap-3 border border-signal/30 bg-signal/[0.06] px-4 py-3 ring-1 ring-signal/20">
        <UpdateIcon />
        <div className="flex-1">
          {renderBody(state, onInstall, onDismiss)}
        </div>
      </div>
    </div>
  );
}

function renderBody(state: UpdateState, onInstall: () => void, onDismiss: () => void) {
  switch (state.kind) {
    case "available":
      return (
        <>
          <p className="font-display text-[13px] font-medium text-ink-50">
            {t.updates.available}
          </p>
          <p className="mt-0.5 font-mono text-[11px] text-ink-200/90">
            {t.updates.versionPrefix} {state.version}
          </p>
          <div className="mt-3 flex gap-2">
            <button className="btn btn-primary !py-1.5 !px-3" onClick={onInstall}>
              {t.updates.installNow}
            </button>
            <button className="btn !py-1.5 !px-3" onClick={onDismiss}>
              {t.updates.later}
            </button>
          </div>
        </>
      );

    case "downloading": {
      const { downloaded, total } = state.progress;
      const pct = total ? Math.min(100, Math.round((downloaded / total) * 100)) : null;
      return (
        <>
          <p className="font-display text-[13px] font-medium text-ink-50">
            {t.updates.downloading}
          </p>
          <p className="mt-0.5 font-mono tnum text-[11px] text-ink-200/90">
            {t.updates.versionPrefix} {state.version}
            {pct !== null && ` · ${pct}%`}
            {pct === null && downloaded > 0 && ` · ${formatBytes(downloaded)}`}
          </p>
          <div className="mt-2 h-1 w-full overflow-hidden rounded-full bg-ink-50/[0.08]">
            <div
              className="h-full bg-signal transition-[width] duration-200"
              style={{ width: pct !== null ? `${pct}%` : "30%" }}
            />
          </div>
        </>
      );
    }

    case "installing":
      return (
        <p className="font-mono text-[12px] text-ink-100">{t.updates.installing}</p>
      );

    case "failed":
      return (
        <>
          <p className="font-display text-[13px] font-medium text-status-unknown">
            {t.updates.failed}
          </p>
          <p className="mt-0.5 font-mono text-[11px] text-ink-200/90 break-words">
            {state.message}
          </p>
          <div className="mt-2 flex gap-2">
            <button className="btn !py-1.5 !px-3" onClick={onDismiss}>
              {t.errors.dismiss}
            </button>
          </div>
        </>
      );

    default:
      return null;
  }
}

function UpdateIcon() {
  return (
    <span className="grid h-7 w-7 flex-shrink-0 place-items-center rounded-md bg-signal/15 text-signal-glow">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden>
        <path
          d="M12 4v10m0 0l-4-4m4 4l4-4M5 20h14"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  );
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
