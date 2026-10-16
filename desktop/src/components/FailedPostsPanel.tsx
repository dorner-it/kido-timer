import { t } from "../lib/i18n";
import type { FailedPost } from "../lib/cloudTypes";

interface Props {
  failures: FailedPost[];
  onRetry: (runId: string) => Promise<void>;
  onDismiss: (runId: string) => Promise<void>;
}

export function FailedPostsPanel({ failures, onRetry, onDismiss }: Props) {
  if (failures.length === 0) return null;
  return (
    <div className="rounded-xl border border-status-unknown/40 bg-status-unknown/5 p-3">
      <span className="label">
        {t.cloud.failedHeader} ({failures.length})
      </span>
      <ul className="mt-2 flex flex-col gap-2">
        {failures.map((f) => (
          <li
            key={f.runId}
            className="rounded-lg border border-status-unknown/30 bg-ink-700/40 px-3 py-2"
          >
            <div className="flex items-baseline justify-between gap-2">
              <span className="font-display text-[12px] text-ink-50">
                {t.cloud.failedRunPrefix} {f.runNumber} · {f.status}
                {f.originalTimeMs != null && (
                  <span className="ml-2 font-mono text-[10px] text-ink-200/70">
                    {(f.originalTimeMs / 1000).toFixed(3)}s
                  </span>
                )}
              </span>
              <span className="font-mono text-[9px] tracking-wider text-ink-200/50">
                {shortTime(f.failedAt)}
              </span>
            </div>
            <p className="mt-1 font-mono text-[10px] text-status-unknown break-words">
              {f.error}
            </p>
            <div className="mt-2 grid grid-cols-2 gap-1.5">
              <button
                type="button"
                onClick={() => onRetry(f.runId)}
                className="rounded-md border border-signal/30 bg-signal/10 px-2 py-1 font-mono text-[10px] uppercase tracking-[0.18em] text-signal-glow transition hover:bg-signal/20"
              >
                ↻ {t.cloud.failedRetry}
              </button>
              <button
                type="button"
                onClick={() => onDismiss(f.runId)}
                className="rounded-md border border-ink-50/10 bg-ink-50/[0.03] px-2 py-1 font-mono text-[10px] uppercase tracking-[0.18em] text-ink-200 transition hover:bg-ink-50/[0.08]"
              >
                {t.cloud.failedDismiss}
              </button>
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}

function shortTime(iso: string): string {
  // RFC3339 → HH:MM:SS local. If parse fails, return raw.
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleTimeString("de-DE", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
