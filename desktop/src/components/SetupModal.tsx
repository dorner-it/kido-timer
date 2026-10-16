import { useEffect, useState } from "react";
import { listSerialPorts, pickHexDumpFile } from "../lib/tauri";
import type { AppSettings } from "../lib/persistence";
import type { CloudIdentity } from "../lib/cloudTypes";
import { t } from "../lib/i18n";

interface Props {
  open: boolean;
  initial: AppSettings;
  cloudIdentity: CloudIdentity | null;
  onCancel?: () => void;
  onConfirm: (next: AppSettings, mode: "serial" | "demo" | "cloud") => void;
  onPair: (baseUrl: string, apiKey: string) => Promise<void>;
  /** When true, the cancel button is hidden (first-launch / required setup). */
  required?: boolean;
}

const COMMON_BAUDS = [9600, 19200, 38400, 57600, 115200];

type Tab = "serial" | "demo" | "cloud";

export function SetupModal({
  open,
  initial,
  cloudIdentity,
  onCancel,
  onConfirm,
  onPair,
  required,
}: Props) {
  const [tab, setTab] = useState<Tab>(
    initial.demoPath && !initial.port ? "demo" : "serial",
  );
  const [ports, setPorts] = useState<string[]>([]);
  const [scanning, setScanning] = useState(false);
  const [port, setPort] = useState<string | null>(initial.port);
  const [baud, setBaud] = useState<number>(initial.baud);
  const [autoConnect, setAutoConnect] = useState<boolean>(initial.autoConnect);
  const [demoPath, setDemoPath] = useState<string | null>(initial.demoPath);
  const [demoSpeed, setDemoSpeed] = useState<number>(initial.demoSpeed);

  const [cloudBaseUrl, setCloudBaseUrl] = useState<string>(initial.cloudBaseUrl);
  const [cloudApiKey, setCloudApiKey] = useState("");
  const [cloudBusy, setCloudBusy] = useState(false);
  const [cloudError, setCloudError] = useState<string | null>(null);

  // Refresh port list whenever the modal opens
  const refresh = async () => {
    setScanning(true);
    try {
      const list = await listSerialPorts();
      setPorts(list);
      if (!port && list.length > 0) setPort(list[0]);
    } finally {
      setScanning(false);
    }
  };

  useEffect(() => {
    if (open) {
      refresh();
      setCloudBaseUrl(initial.cloudBaseUrl);
      setCloudApiKey("");
      setCloudError(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (!open) return null;

  const handleConfirm = () => {
    const next: AppSettings = {
      ...initial,
      port: tab === "serial" ? port : initial.port,
      baud,
      autoConnect,
      demoPath: tab === "demo" ? demoPath : initial.demoPath,
      demoSpeed,
      cloudBaseUrl: tab === "cloud" ? cloudBaseUrl : initial.cloudBaseUrl,
      hasCompletedSetup: true,
    };
    onConfirm(next, tab);
  };

  const handlePair = async () => {
    setCloudBusy(true);
    setCloudError(null);
    try {
      await onPair(cloudBaseUrl.trim(), cloudApiKey.trim());
      handleConfirm();
    } catch (e) {
      setCloudError(String(e));
    } finally {
      setCloudBusy(false);
    }
  };

  const canConfirmSerial = tab === "serial" && !!port;
  const canConfirmDemo = tab === "demo" && !!demoPath;
  const canConfirmCloud =
    tab === "cloud" && !!cloudBaseUrl && (!!cloudIdentity || !!cloudApiKey);
  const canConfirm = canConfirmSerial || canConfirmDemo || canConfirmCloud;

  return (
    <div className="fixed inset-0 z-50 grid place-items-center px-6">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-ink-900/85 backdrop-blur-md"
        onClick={() => !required && !cloudBusy && onCancel?.()}
      />
      {/* Dialog */}
      <div className="relative z-10 w-full max-w-[640px] surface overflow-hidden">
        <header className="border-b border-ink-50/[0.06] px-7 pt-6 pb-5">
          <span className="label">{t.setup.settings}</span>
          <h2 className="mt-1 font-display text-[26px] font-medium tracking-tight text-ink-50">
            {t.setup.welcomeTitle}
          </h2>
          <p className="mt-2 max-w-[52ch] font-mono text-[12px] leading-relaxed text-ink-200/80">
            {t.setup.welcomeBody}
          </p>
        </header>

        {/* Tabs */}
        <nav className="flex items-center gap-1 border-b border-ink-50/[0.05] bg-ink-900/40 px-3 py-2">
          <TabButton active={tab === "serial"} onClick={() => setTab("serial")}>
            ⚡ {t.setup.tabSerial}
          </TabButton>
          <TabButton active={tab === "demo"} onClick={() => setTab("demo")}>
            ▶ {t.setup.tabDemo}
          </TabButton>
          <TabButton active={tab === "cloud"} onClick={() => setTab("cloud")}>
            ☁ {t.setup.tabCloud}
          </TabButton>
        </nav>

        <div className="px-7 py-6">
          {tab === "serial" ? (
            <div className="flex flex-col gap-5">
              <div className="grid grid-cols-[1fr,140px] gap-3">
                <div className="field">
                  <div className="flex items-center justify-between">
                    <label className="label">{t.setup.serialPort}</label>
                    <button
                      type="button"
                      onClick={refresh}
                      disabled={scanning}
                      className="font-mono text-[10px] uppercase tracking-[0.2em] text-ink-200/80 hover:text-ink-50 transition disabled:opacity-50"
                    >
                      {scanning ? t.setup.scanning : `↻ ${t.setup.rescan}`}
                    </button>
                  </div>
                  <select
                    value={port ?? ""}
                    onChange={(e) => setPort(e.target.value || null)}
                    className="input appearance-none"
                  >
                    {ports.length === 0 && (
                      <option value="">{t.setup.noPorts}</option>
                    )}
                    {ports.map((p) => (
                      <option key={p} value={p}>
                        {p}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="field">
                  <label className="label">{t.setup.baudRate}</label>
                  <select
                    value={baud}
                    onChange={(e) => setBaud(Number(e.target.value))}
                    className="input appearance-none"
                  >
                    {COMMON_BAUDS.map((b) => (
                      <option key={b} value={b}>
                        {b}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
            </div>
          ) : tab === "demo" ? (
            <div className="flex flex-col gap-5">
              <div className="field">
                <label className="label">{t.setup.pickFile}</label>
                <button
                  type="button"
                  onClick={async () => {
                    const path = await pickHexDumpFile();
                    if (path) setDemoPath(path);
                  }}
                  className="rounded-md border border-dashed border-ink-50/10 bg-ink-50/[0.02] px-4 py-3 text-left font-mono text-[12px] text-ink-100 transition hover:bg-ink-50/[0.05]"
                >
                  {demoPath ? truncatePath(demoPath) : `📂 ${t.setup.pickFile}`}
                </button>
              </div>

              <div className="field">
                <label className="label">{t.setup.speed}</label>
                <div className="flex items-center gap-3">
                  <input
                    type="range"
                    min={0}
                    max={10}
                    step={0.5}
                    value={demoSpeed}
                    onChange={(e) => setDemoSpeed(Number(e.target.value))}
                    className="flex-1 accent-signal"
                  />
                  <span className="font-mono tnum w-16 text-right text-[12px] text-ink-100">
                    {demoSpeed === 0 ? t.setup.speedMax : `${demoSpeed.toFixed(1)}×`}
                  </span>
                </div>
              </div>
            </div>
          ) : (
            <CloudTab
              identity={cloudIdentity}
              baseUrl={cloudBaseUrl}
              setBaseUrl={setCloudBaseUrl}
              apiKey={cloudApiKey}
              setApiKey={setCloudApiKey}
              busy={cloudBusy}
              error={cloudError}
            />
          )}

          {tab !== "cloud" && (
            <label className="mt-5 flex items-center gap-3 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={autoConnect}
                onChange={(e) => setAutoConnect(e.target.checked)}
                className="h-4 w-4 accent-signal"
              />
              <span className="font-mono text-[12px] text-ink-100">
                {t.setup.rememberLabel}
              </span>
            </label>
          )}
        </div>

        <footer className="flex items-center justify-end gap-2 border-t border-ink-50/[0.05] bg-ink-900/40 px-7 py-4">
          {!required && (
            <button className="btn" onClick={onCancel} disabled={cloudBusy}>
              {t.setup.cancel}
            </button>
          )}
          {tab === "cloud" ? (
            <button
              className="btn btn-primary"
              disabled={!canConfirmCloud || cloudBusy}
              onClick={cloudIdentity && !cloudApiKey ? handleConfirm : handlePair}
            >
              {cloudBusy
                ? t.cloud.pairing
                : cloudIdentity && !cloudApiKey
                  ? t.setup.cloudFinish
                  : t.cloud.pairAction}
            </button>
          ) : (
            <button
              className="btn btn-primary"
              disabled={!canConfirm}
              onClick={handleConfirm}
            >
              {tab === "demo" ? t.setup.startDemo : t.setup.saveAndConnect}
            </button>
          )}
        </footer>
      </div>
    </div>
  );
}

function CloudTab({
  identity,
  baseUrl,
  setBaseUrl,
  apiKey,
  setApiKey,
  busy,
  error,
}: {
  identity: CloudIdentity | null;
  baseUrl: string;
  setBaseUrl: (v: string) => void;
  apiKey: string;
  setApiKey: (v: string) => void;
  busy: boolean;
  error: string | null;
}) {
  return (
    <div className="flex flex-col gap-5">
      {identity && (
        <div className="rounded-xl border border-signal/30 bg-signal/5 p-4">
          <span className="label">{t.cloud.pairedAs}</span>
          <p className="mt-1 font-display text-[13px] text-ink-50">
            {identity.displayName || identity.email}
          </p>
          <p className="mt-0.5 font-mono text-[11px] text-ink-200/70">
            {identity.email}
          </p>
          <p className="mt-2 font-mono text-[10px] text-ink-200/60">
            {t.setup.cloudPaired}
          </p>
        </div>
      )}

      <p className="font-mono text-[11px] leading-relaxed text-ink-200/80">
        {t.cloud.pairBody}
      </p>

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
        <label className="label">
          {identity ? t.cloud.rePair : t.cloud.apiKeyLabel}
        </label>
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          spellCheck={false}
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
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "rounded-md px-4 py-2 font-mono text-[11px] uppercase tracking-[0.18em] transition",
        active
          ? "bg-ink-50/10 text-ink-50"
          : "text-ink-200/70 hover:bg-ink-50/5 hover:text-ink-100",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

function truncatePath(p: string): string {
  if (p.length <= 56) return p;
  return `${p.slice(0, 18)}…${p.slice(-32)}`;
}
