import { t } from "../lib/i18n";
import type { SyncMode } from "../lib/cloudTypes";
import { isOverridden, syncModeLabel } from "../lib/syncMode";

interface Props {
  webDefault: SyncMode;
  effective: SyncMode;
  onChange: (mode: SyncMode | null) => void;
}

/** Live / On-prem switch for the loaded discipline (desktop override). */
export function SyncModeToggle({ webDefault, effective, onChange }: Props) {
  const overridden = isOverridden(webDefault, effective);
  return (
    <div className="flex items-center gap-2">
      <div
        role="radiogroup"
        aria-label={t.syncMode.label}
        className="grid grid-cols-2 gap-1 rounded-lg border border-ink-50/[0.08] bg-ink-50/[0.03] p-0.5"
      >
        {(["live", "offline"] as const).map((mode) => {
          const active = effective === mode;
          return (
            <button
              key={mode}
              type="button"
              role="radio"
              aria-checked={active}
              title={mode === "live" ? t.syncMode.liveHint : t.syncMode.onPremHint}
              onClick={() => onChange(mode)}
              className={[
                "rounded-md px-2.5 py-1 font-mono text-[10px] uppercase tracking-[0.18em] transition",
                active
                  ? mode === "live"
                    ? "bg-signal/15 text-signal-glow ring-1 ring-signal/30"
                    : "bg-status-armed/15 text-status-armed ring-1 ring-status-armed/30"
                  : "text-ink-200 hover:bg-ink-50/[0.06]",
              ].join(" ")}
            >
              {syncModeLabel(mode)}
            </button>
          );
        })}
      </div>
      {overridden && (
        <button
          type="button"
          onClick={() => onChange(null)}
          title={`${t.cloud.overriddenHint(syncModeLabel(webDefault))} — ${t.syncMode.resetToDefault}`}
          className="rounded px-1.5 py-0.5 font-mono text-[9px] uppercase tracking-[0.18em] bg-ink-50/[0.06] text-ink-100 ring-1 ring-ink-50/10 hover:bg-ink-50/10"
        >
          {t.cloud.overriddenBadge} ↺
        </button>
      )}
    </div>
  );
}
