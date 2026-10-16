import { useEffect, useState } from "react";
import { t } from "../lib/i18n";

interface Props {
  open: boolean;
  initialBaseUrl: string;
  onCancel: () => void;
  onSubmit: (baseUrl: string, apiKey: string) => Promise<void>;
}

export function CloudPairingModal({
  open,
  initialBaseUrl,
  onCancel,
  onSubmit,
}: Props) {
  const [baseUrl, setBaseUrl] = useState(initialBaseUrl);
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setBaseUrl(initialBaseUrl);
      setApiKey("");
      setError(null);
    }
  }, [open, initialBaseUrl]);

  if (!open) return null;

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await onSubmit(baseUrl.trim(), apiKey.trim());
      // close handled by parent on success
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 grid place-items-center px-6">
      <div
        className="absolute inset-0 bg-ink-900/85 backdrop-blur-md"
        onClick={() => !busy && onCancel()}
      />
      <div className="relative z-10 w-full max-w-[560px] surface overflow-hidden">
        <header className="border-b border-ink-50/[0.06] px-7 pt-6 pb-5">
          <span className="label">{t.cloud.subtitle}</span>
          <h2 className="mt-1 font-display text-[24px] font-medium tracking-tight text-ink-50">
            {t.cloud.pairTitle}
          </h2>
          <p className="mt-2 max-w-[52ch] font-mono text-[12px] leading-relaxed text-ink-200/80">
            {t.cloud.pairBody}
          </p>
        </header>

        <div className="flex flex-col gap-5 px-7 py-6">
          <div className="field">
            <label className="label">{t.cloud.baseUrlLabel}</label>
            <input
              type="url"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
              spellCheck={false}
              className="input font-mono text-[12px]"
              disabled={busy}
            />
          </div>
          <div className="field">
            <label className="label">{t.cloud.apiKeyLabel}</label>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              spellCheck={false}
              autoFocus
              placeholder={t.cloud.apiKeyPlaceholder}
              className="input font-mono text-[12px]"
              disabled={busy}
            />
          </div>
          {error && (
            <div className="rounded-md border border-status-unknown/40 bg-status-unknown/5 px-3 py-2 font-mono text-[11px] text-status-unknown">
              {error}
            </div>
          )}
        </div>

        <footer className="flex items-center justify-end gap-2 border-t border-ink-50/[0.05] bg-ink-900/40 px-7 py-4">
          <button className="btn" onClick={onCancel} disabled={busy}>
            {t.cloud.cancel}
          </button>
          <button
            className="btn btn-primary"
            onClick={submit}
            disabled={busy || !apiKey || !baseUrl}
          >
            {busy ? t.cloud.pairing : t.cloud.pairAction}
          </button>
        </footer>
      </div>
    </div>
  );
}
