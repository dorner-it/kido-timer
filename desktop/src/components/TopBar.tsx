import { useEffect, useState } from "react";
import { deviceModeDE, stateFlagDE, t } from "../lib/i18n";
import type { ConnectionStatus, FrameDto } from "../lib/types";

interface Props {
  status: ConnectionStatus;
  source: string | null;
  frame: FrameDto | null;
  fps: number;
  frameCount: number;
  onMenu: () => void;
  onReset: () => void;
  onConfirmRun: () => void;
  pendingConfirmCount: number;
  resetDisabled: boolean;
  confirmDisabled: boolean;
}

const STATUS_DOT: Record<ConnectionStatus, { color: string; label: string }> = {
  idle: { color: "bg-ink-300", label: t.conn.statusIdle },
  connecting: { color: "bg-status-armed animate-pulse", label: t.conn.statusConnecting },
  connected: { color: "bg-status-confirmed", label: t.conn.statusConnected },
  error: { color: "bg-status-unknown animate-pulse", label: t.conn.statusError },
};

function useNowClock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}

export function TopBar({
  status,
  source,
  frame,
  fps,
  frameCount,
  onMenu,
  onReset,
  onConfirmRun,
  pendingConfirmCount,
  resetDisabled,
  confirmDisabled,
}: Props) {
  const dot = STATUS_DOT[status];
  const now = useNowClock();
  const clock = now.toLocaleTimeString("de-DE", { hour12: false });

  return (
    <header className="flex items-center justify-between gap-6 px-7 py-4 border-b border-ink-50/[0.06]">
      <div className="flex items-center gap-4">
        <BrandMark />
        <div className="hidden md:flex flex-col leading-tight">
          <span className="font-display text-[15px] font-medium tracking-wide text-ink-50">
            {t.app.brand}
          </span>
          <span className="label">{t.app.subtitle}</span>
        </div>
      </div>

      <div className="flex flex-1 items-center justify-center gap-3">
        <div className="flex items-center gap-2 rounded-full border border-ink-50/[0.06] bg-ink-800/60 px-3 py-1.5 backdrop-blur">
          <span className={`h-2 w-2 rounded-full ${dot.color}`} />
          <span className="font-mono text-[10px] uppercase tracking-[0.22em] text-ink-100">
            {dot.label}
          </span>
          <span className="text-ink-300">·</span>
          <span className="font-mono text-[10px] tracking-wider text-ink-200/90 truncate max-w-[24ch]">
            {source ?? t.conn.noSource}
          </span>
        </div>
        <ResetButton onReset={onReset} disabled={resetDisabled} />
        <ConfirmRunButton
          onConfirm={onConfirmRun}
          disabled={confirmDisabled}
          count={pendingConfirmCount}
        />
      </div>

      <div className="flex items-center gap-5 text-right">
        <Stat label={t.topbar.mode} value={frame ? deviceModeDE(frame.deviceMode) : "—"} />
        <Stat label={t.topbar.lane} value={frame ? `B${frame.lane}` : "—"} />
        <Stat label={t.topbar.state} value={frame ? stateFlagDE(frame.stateFlag) : "—"} />
        <Stat label={t.topbar.fps} value={fps.toFixed(0)} />
        <Stat label={t.topbar.frames} value={frameCount.toLocaleString("de-DE")} />
        <div className="flex flex-col items-end leading-tight">
          <span className="font-mono tnum text-[18px] tracking-wider text-ink-50">{clock}</span>
          <span className="label">{t.topbar.localTime}</span>
        </div>
        <button
          type="button"
          onClick={onMenu}
          aria-label={t.topbar.menu}
          className="grid h-10 w-10 place-items-center rounded-xl border border-ink-50/10 bg-ink-50/[0.04] text-ink-50 transition hover:bg-ink-50/10"
        >
          <svg width="16" height="14" viewBox="0 0 16 14" fill="none">
            <path
              d="M1 1h14M1 7h14M1 13h14"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          </svg>
        </button>
      </div>
    </header>
  );
}

function ResetButton({
  onReset,
  disabled,
}: {
  onReset: () => void;
  disabled: boolean;
}) {
  const [confirming, setConfirming] = useState(false);

  useEffect(() => {
    if (!confirming) return;
    const id = window.setTimeout(() => setConfirming(false), 4000);
    return () => window.clearTimeout(id);
  }, [confirming]);

  if (!confirming) {
    return (
      <button
        type="button"
        disabled={disabled}
        onClick={() => setConfirming(true)}
        title={t.topbar.resetTitle}
        className="rounded-xl border border-status-unknown/40 bg-status-unknown/10 px-4 py-2 font-mono text-[11px] uppercase tracking-[0.22em] text-status-unknown transition hover:bg-status-unknown/20 disabled:opacity-30 disabled:cursor-not-allowed"
      >
        ⏻ {t.topbar.reset}
      </button>
    );
  }

  return (
    <div className="flex items-center gap-1 rounded-xl border border-status-unknown/40 bg-status-unknown/10 px-2 py-1">
      <button
        type="button"
        onClick={() => setConfirming(false)}
        className="rounded-md px-2 py-1 font-mono text-[10px] uppercase tracking-[0.2em] text-ink-100 hover:bg-ink-50/10"
      >
        {t.topbar.resetCancel}
      </button>
      <button
        type="button"
        onClick={() => {
          onReset();
          setConfirming(false);
        }}
        className="rounded-md bg-status-unknown/30 px-3 py-1 font-mono text-[10px] uppercase tracking-[0.2em] text-status-unknown hover:bg-status-unknown/50"
      >
        {t.topbar.resetConfirmAction}
      </button>
    </div>
  );
}

function ConfirmRunButton({
  onConfirm,
  disabled,
  count,
}: {
  onConfirm: () => void;
  disabled: boolean;
  count: number;
}) {
  const title = disabled
    ? t.topbar.confirmRunNone
    : t.topbar.confirmRunCount(count);
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onConfirm}
      title={title}
      className="flex items-center gap-2 rounded-xl border border-status-confirmed/40 bg-status-confirmed/10 px-4 py-2 font-mono text-[11px] uppercase tracking-[0.22em] text-status-confirmed transition hover:bg-status-confirmed/20 disabled:opacity-30 disabled:cursor-not-allowed"
    >
      <span>✓ {t.topbar.confirmRun}</span>
      {count > 0 && (
        <span className="rounded-full bg-status-confirmed/30 px-1.5 py-0.5 font-mono text-[10px] text-status-confirmed">
          {count}
        </span>
      )}
    </button>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="hidden lg:flex flex-col items-end leading-tight">
      <span className="font-mono text-[13px] uppercase tracking-wider text-ink-50">{value}</span>
      <span className="label">{label}</span>
    </div>
  );
}

function BrandMark() {
  return (
    <div className="relative grid h-10 w-10 place-items-center rounded-xl bg-gradient-to-br from-signal to-status-captured shadow-[0_8px_24px_-8px_rgba(255,77,46,0.7)]">
      <span className="font-stencil text-[18px] font-bold leading-none text-ink-900">KiDo</span>
      <span className="absolute -bottom-0.5 -right-0.5 h-2 w-2 rounded-full bg-status-confirmed ring-2 ring-ink-900" />
    </div>
  );
}
