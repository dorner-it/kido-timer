import { useState } from "react";
import { t } from "../lib/i18n";
import type {
  CloudIdentity,
  DisciplinePayload,
  FailedPost,
} from "../lib/cloudTypes";
import { FailedPostsPanel } from "./FailedPostsPanel";

interface Props {
  identity: CloudIdentity | null;
  snapshot: DisciplinePayload | null;
  loading: boolean;
  resultMessage: string | null;
  failedPosts: FailedPost[];
  onPair: () => void;
  onUnpair: () => Promise<void>;
  onPickCompetition: () => void;
  onDeselect: () => Promise<void>;
  onOpenKido: () => Promise<void>;
  onExportKido: () => Promise<string | null>;
  onDismissResultMessage: () => void;
  onRetryFailed: (runId: string) => Promise<void>;
  onDismissFailed: (runId: string) => Promise<void>;
}

export function CloudSection({
  identity,
  snapshot,
  loading,
  resultMessage,
  failedPosts,
  onPair,
  onUnpair,
  onPickCompetition,
  onDeselect,
  onOpenKido,
  onExportKido,
  onDismissResultMessage,
  onRetryFailed,
  onDismissFailed,
}: Props) {
  const [confirmingUnpair, setConfirmingUnpair] = useState(false);
  const [exportInfo, setExportInfo] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  const onExport = async () => {
    setExportError(null);
    setExportInfo(null);
    try {
      const path = await onExportKido();
      if (path) setExportInfo(`${t.cloud.exportSaved}: ${shortenPath(path)}`);
    } catch (e) {
      setExportError(`${t.cloud.exportFailed}: ${e}`);
    }
  };

  if (!identity) {
    return (
      <div className="flex flex-col gap-2">
        <div className="rounded-xl border border-ink-50/[0.05] bg-ink-700/40 p-4">
          <p className="font-mono text-[12px] text-ink-100">{t.cloud.notPaired}</p>
        </div>
        <button className="btn btn-primary" onClick={onPair} disabled={loading}>
          {t.cloud.pair}
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="rounded-xl border border-ink-50/[0.05] bg-ink-700/40 p-4">
        <span className="label">{t.cloud.pairedAs}</span>
        <p className="mt-1 font-display text-[13px] text-ink-50">
          {identity.displayName || identity.email}
        </p>
        <p className="mt-0.5 font-mono text-[11px] text-ink-200/70">
          {identity.email}
        </p>
        <div className="mt-2 grid grid-cols-2 gap-2 font-mono text-[10px] tracking-wider text-ink-200/60">
          <div>
            <span className="block uppercase tracking-[0.18em] text-ink-200/50">
              {t.cloud.server}
            </span>
            <span className="block break-all text-ink-200/80">
              {hostname(identity.baseUrl)}
            </span>
          </div>
          <div>
            <span className="block uppercase tracking-[0.18em] text-ink-200/50">
              {t.cloud.keyId}
            </span>
            <span className="block text-ink-200/80">
              {identity.keyId.slice(0, 8)}…
            </span>
          </div>
        </div>
      </div>

      {snapshot ? (
        <div className="rounded-xl border border-signal/30 bg-signal/5 p-4">
          <span className="label">{t.banner.activeCompetition}</span>
          <p className="mt-1 font-display text-[13px] text-ink-50">
            {snapshot.event.name}
            <span className="mx-1.5 text-ink-200/60">·</span>
            <span className="text-ink-100">{snapshot.discipline.name}</span>
          </p>
          <button className="btn mt-3 w-full" onClick={onDeselect}>
            {t.cloud.deselect}
          </button>
        </div>
      ) : (
        <button
          className="btn btn-primary"
          onClick={onPickCompetition}
          disabled={loading}
        >
          {t.cloud.pickCompetition}
        </button>
      )}

      <div className="grid grid-cols-2 gap-2">
        <button className="btn" onClick={onOpenKido} disabled={loading}>
          📂 {t.cloud.openKido}
        </button>
        <button
          className="btn"
          onClick={onExport}
          disabled={loading || !snapshot}
          title={!snapshot ? t.cloud.needSnapshot : undefined}
        >
          💾 {t.cloud.exportKido}
        </button>
      </div>

      {exportInfo && (
        <p className="font-mono text-[10px] text-ink-200/80">{exportInfo}</p>
      )}
      {exportError && (
        <p className="font-mono text-[10px] text-status-unknown">{exportError}</p>
      )}

      <FailedPostsPanel
        failures={failedPosts}
        onRetry={onRetryFailed}
        onDismiss={onDismissFailed}
      />

      {resultMessage && (
        <button
          type="button"
          onClick={onDismissResultMessage}
          className="rounded-md border border-ink-50/[0.06] bg-ink-50/[0.03] px-3 py-2 text-left font-mono text-[10px] text-ink-100 hover:bg-ink-50/[0.06]"
        >
          {resultMessage}
        </button>
      )}

      {!confirmingUnpair ? (
        <button
          className="btn btn-danger"
          onClick={() => setConfirmingUnpair(true)}
        >
          {t.cloud.unpair}
        </button>
      ) : (
        <div className="rounded-xl border border-status-unknown/40 bg-status-unknown/5 p-3">
          <p className="font-mono text-[12px] text-ink-100">
            {t.cloud.unpairConfirm}
          </p>
          <div className="mt-3 grid grid-cols-2 gap-2">
            <button className="btn" onClick={() => setConfirmingUnpair(false)}>
              {t.cloud.cancel}
            </button>
            <button
              className="btn btn-danger"
              onClick={async () => {
                await onUnpair();
                setConfirmingUnpair(false);
              }}
            >
              {t.cloud.unpairAction}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function hostname(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

function shortenPath(p: string): string {
  if (p.length <= 48) return p;
  return `${p.slice(0, 12)}…${p.slice(-32)}`;
}
