import { useEffect, useState } from "react";
import { t } from "../lib/i18n";
import type { Theme } from "../lib/persistence";
import type { ConnectionStatus } from "../lib/types";
import type { CloudIdentity, DisciplinePayload, FailedPost } from "../lib/cloudTypes";
import { CloudSection } from "./CloudSection";

interface Props {
  open: boolean;
  onClose: () => void;
  status: ConnectionStatus;
  source: string | null;
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  onEditConnection: () => void;
  onDisconnect: () => void;
  onReset: () => void;
  onOpenProtocol: () => void;
  cloudIdentity: CloudIdentity | null;
  cloudSnapshot: DisciplinePayload | null;
  cloudLoading: boolean;
  cloudResultMessage: string | null;
  cloudFailedPosts: FailedPost[];
  onCloudPair: () => void;
  onCloudUnpair: () => Promise<void>;
  onCloudPickCompetition: () => void;
  onCloudDeselect: () => Promise<void>;
  onCloudOpenKido: () => Promise<void>;
  onCloudExportKido: () => Promise<string | null>;
  onCloudDismissResultMessage: () => void;
  onCloudRetryFailed: (runId: string) => Promise<void>;
  onCloudDismissFailed: (runId: string) => Promise<void>;
}

export function Drawer({
  open,
  onClose,
  status,
  source,
  theme,
  onThemeChange,
  onEditConnection,
  onDisconnect,
  onReset,
  onOpenProtocol,
  cloudIdentity,
  cloudSnapshot,
  cloudLoading,
  cloudResultMessage,
  cloudFailedPosts,
  onCloudPair,
  onCloudUnpair,
  onCloudPickCompetition,
  onCloudDeselect,
  onCloudOpenKido,
  onCloudExportKido,
  onCloudDismissResultMessage,
  onCloudRetryFailed,
  onCloudDismissFailed,
}: Props) {
  const isConnected = status === "connected";

  // Close drawer on Escape
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  return (
    <>
      {/* Backdrop */}
      <div
        aria-hidden
        onClick={onClose}
        className={[
          "fixed inset-0 z-30 bg-ink-900/60 backdrop-blur-sm transition-opacity duration-300",
          open ? "opacity-100" : "pointer-events-none opacity-0",
        ].join(" ")}
      />

      {/* Panel */}
      <aside
        aria-hidden={!open}
        className={[
          "fixed right-0 top-0 z-40 flex h-full w-[380px] flex-col bg-ink-800/95 shadow-2xl ring-1 ring-ink-50/[0.06] backdrop-blur",
          "transition-transform duration-300 ease-out",
          open ? "translate-x-0" : "translate-x-full",
        ].join(" ")}
      >
        <header className="flex items-center justify-between border-b border-ink-50/[0.05] px-6 py-4">
          <div>
            <span className="label">{t.setup.settings}</span>
            <h2 className="font-display text-[18px] font-medium text-ink-50">
              {t.drawer.title}
            </h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t.topbar.closeMenu}
            className="grid h-9 w-9 place-items-center rounded-lg border border-ink-50/10 bg-ink-50/[0.03] font-mono text-[13px] text-ink-100 transition hover:bg-ink-50/10"
          >
            ✕
          </button>
        </header>

        <div className="flex flex-1 flex-col gap-6 overflow-y-auto px-6 py-6 no-scrollbar">
          {/* Verbindung */}
          <Section title={t.drawer.sectionConnection}>
            <div className="rounded-xl border border-ink-50/[0.05] bg-ink-700/50 p-4">
              <span className="label">{t.drawer.source}</span>
              <p className="mt-1 font-mono tnum text-[13px] text-ink-50 break-words">
                {source ?? t.drawer.notConnected}
              </p>
            </div>
            <button className="btn" onClick={onEditConnection}>
              {t.drawer.edit}
            </button>
            <button
              className="btn btn-danger"
              disabled={!isConnected}
              onClick={onDisconnect}
            >
              {t.drawer.disconnect}
            </button>
          </Section>

          {/* Gerät */}
          <Section title={t.drawer.sectionDevice}>
            <ResetAction disabled={!isConnected} onReset={onReset} />
          </Section>

          {/* Online-Verbindung */}
          <Section title={t.drawer.sectionCloud}>
            <CloudSection
              identity={cloudIdentity}
              snapshot={cloudSnapshot}
              loading={cloudLoading}
              resultMessage={cloudResultMessage}
              failedPosts={cloudFailedPosts}
              onPair={onCloudPair}
              onUnpair={onCloudUnpair}
              onPickCompetition={onCloudPickCompetition}
              onDeselect={onCloudDeselect}
              onOpenKido={onCloudOpenKido}
              onExportKido={onCloudExportKido}
              onDismissResultMessage={onCloudDismissResultMessage}
              onRetryFailed={onCloudRetryFailed}
              onDismissFailed={onCloudDismissFailed}
            />
          </Section>

          {/* Ansicht */}
          <Section title={t.drawer.sectionView}>
            <button
              className="btn btn-primary justify-between"
              onClick={onOpenProtocol}
            >
              <span>📋 {t.drawer.openProtocol}</span>
              <span className="font-mono text-[10px] tracking-[0.2em]">→</span>
            </button>
          </Section>

          {/* Darstellung */}
          <Section title={t.drawer.sectionAppearance}>
            <ThemeToggle value={theme} onChange={onThemeChange} />
          </Section>
        </div>
      </aside>
    </>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="label">{title}</h3>
      {children}
    </section>
  );
}

function ThemeToggle({
  value,
  onChange,
}: {
  value: Theme;
  onChange: (t: Theme) => void;
}) {
  return (
    <div
      role="radiogroup"
      aria-label={t.drawer.themeLabel}
      className="grid grid-cols-2 gap-1 rounded-lg border border-ink-50/[0.08] bg-ink-50/[0.03] p-1"
    >
      <ThemeOption
        active={value === "light"}
        onClick={() => onChange("light")}
        icon={<SunIcon />}
        label={t.drawer.themeLight}
      />
      <ThemeOption
        active={value === "dark"}
        onClick={() => onChange("dark")}
        icon={<MoonIcon />}
        label={t.drawer.themeDark}
      />
    </div>
  );
}

function ThemeOption({
  active,
  onClick,
  icon,
  label,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={active}
      onClick={onClick}
      className={[
        "flex items-center justify-center gap-2 rounded-md px-3 py-2 font-mono text-[11px] uppercase tracking-[0.18em] transition",
        active
          ? "bg-signal/15 text-signal-glow ring-1 ring-signal/30"
          : "text-ink-200 hover:bg-ink-50/[0.06] hover:text-ink-50",
      ].join(" ")}
    >
      {icon}
      {label}
    </button>
  );
}

function SunIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden>
      <circle cx="12" cy="12" r="4" stroke="currentColor" strokeWidth="1.6" />
      <path
        d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
      />
    </svg>
  );
}

function MoonIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden>
      <path
        d="M21 13.5A9 9 0 0 1 10.5 3a7.5 7.5 0 1 0 10.5 10.5z"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function ResetAction({ disabled, onReset }: { disabled: boolean; onReset: () => void }) {
  const [confirming, setConfirming] = useState(false);

  if (!confirming) {
    return (
      <button
        type="button"
        disabled={disabled}
        onClick={() => setConfirming(true)}
        className="group rounded-xl border border-signal/30 bg-signal/10 px-4 py-3 text-left transition hover:bg-signal/20 disabled:opacity-40 disabled:cursor-not-allowed"
      >
        <div className="flex items-center justify-between">
          <span className="font-display text-[13px] font-medium tracking-wide text-signal-glow">
            ⏻ {t.drawer.reset}
          </span>
          <span className="font-mono text-[10px] uppercase tracking-[0.2em] text-signal-glow/70">
            RST\r
          </span>
        </div>
        <p className="mt-1 font-mono text-[10px] tracking-wider text-ink-200/70">
          {t.drawer.resetCommand}
        </p>
      </button>
    );
  }

  return (
    <div className="rounded-xl border border-status-unknown/40 bg-status-unknown/5 p-3">
      <p className="font-mono text-[12px] text-ink-100">{t.drawer.resetConfirm}</p>
      <div className="mt-3 grid grid-cols-2 gap-2">
        <button className="btn" onClick={() => setConfirming(false)}>
          {t.setup.cancel}
        </button>
        <button
          className="btn btn-danger"
          onClick={() => {
            onReset();
            setConfirming(false);
          }}
        >
          {t.drawer.resetConfirmAction}
        </button>
      </div>
    </div>
  );
}
