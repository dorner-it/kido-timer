import { useEffect, useState } from "react";
import { modeLabelDE, t } from "../lib/i18n";
import type { DisciplineListItem, SyncMode } from "../lib/cloudTypes";

interface Props {
  open: boolean;
  disciplines: DisciplineListItem[];
  loading: boolean;
  error: string | null;
  selectedId: string | null;
  onRefresh: () => Promise<void>;
  onPick: (id: string) => Promise<unknown>;
  onClose: () => void;
}

export function CompetitionPicker({
  open,
  disciplines,
  loading,
  error,
  selectedId,
  onRefresh,
  onPick,
  onClose,
}: Props) {
  const [pickingId, setPickingId] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      void onRefresh();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  if (!open) return null;

  const pick = async (id: string) => {
    setPickingId(id);
    try {
      await onPick(id);
      onClose();
    } finally {
      setPickingId(null);
    }
  };

  // Group disciplines by event so the picker shows the day's structure.
  const groupedByEvent = new Map<
    string,
    { event_name: string; event_date: string; entries: DisciplineListItem[] }
  >();
  for (const d of disciplines) {
    const bucket = groupedByEvent.get(d.event_id) ?? {
      event_name: d.event_name,
      event_date: d.event_date,
      entries: [],
    };
    bucket.entries.push(d);
    groupedByEvent.set(d.event_id, bucket);
  }
  const eventGroups = Array.from(groupedByEvent.entries()).sort(
    (a, b) => (a[1].event_date > b[1].event_date ? -1 : 1),
  );

  return (
    <div className="fixed inset-0 z-50 grid place-items-center px-6">
      <div
        className="absolute inset-0 bg-ink-900/85 backdrop-blur-md"
        onClick={onClose}
      />
      <div className="relative z-10 flex w-full max-w-[640px] flex-col surface overflow-hidden">
        <header className="flex items-start justify-between border-b border-ink-50/[0.06] px-7 pt-6 pb-5">
          <div>
            <span className="label">{t.cloud.title}</span>
            <h2 className="mt-1 font-display text-[22px] font-medium tracking-tight text-ink-50">
              {t.cloud.pickCompetition}
            </h2>
          </div>
          <button
            type="button"
            onClick={onRefresh}
            disabled={loading}
            className="font-mono text-[10px] uppercase tracking-[0.2em] text-ink-200/80 hover:text-ink-50 transition disabled:opacity-50"
          >
            {loading ? t.cloud.loadingCompetitions : `↻ ${t.cloud.refresh}`}
          </button>
        </header>

        <div className="max-h-[60vh] overflow-y-auto px-7 py-5">
          {error && (
            <div className="mb-4 rounded-md border border-status-unknown/40 bg-status-unknown/5 px-3 py-2 font-mono text-[11px] text-status-unknown">
              {error}
            </div>
          )}
          {!loading && disciplines.length === 0 && (
            <p className="font-mono text-[12px] text-ink-200/80">
              {t.cloud.noCompetitions}
            </p>
          )}
          {eventGroups.map(([event_id, group]) => (
            <section key={event_id} className="mb-5 last:mb-0">
              <div className="mb-2 flex items-baseline justify-between">
                <h3 className="font-display text-[14px] font-medium text-ink-50 truncate">
                  {group.event_name}
                </h3>
                <p className="font-mono text-[11px] tracking-wider text-ink-200/70">
                  {formatDate(group.event_date)}
                </p>
              </div>
              <ul className="flex flex-col gap-2">
                {group.entries.map((d) => {
                  const isSelected = d.id === selectedId;
                  return (
                    <li key={d.id}>
                      <button
                        type="button"
                        onClick={() => pick(d.id)}
                        disabled={pickingId !== null}
                        className={[
                          "w-full rounded-xl border px-4 py-3 text-left transition disabled:opacity-50",
                          isSelected
                            ? "border-signal/40 bg-signal/10 ring-1 ring-signal/30"
                            : "border-ink-50/10 bg-ink-50/[0.02] hover:bg-ink-50/[0.06]",
                        ].join(" ")}
                      >
                        <div className="flex items-center justify-between gap-4">
                          <div className="min-w-0">
                            <p className="font-display text-[15px] font-medium text-ink-50 truncate">
                              {d.name}
                            </p>
                            <p className="mt-0.5 font-mono text-[11px] text-ink-200/80">
                              {modeLabelDE(d.mode)}
                            </p>
                          </div>
                          <div className="flex items-center gap-1.5 shrink-0">
                            {d.is_current && (
                              <Badge color="signal" label={t.cloud.activeBadge} />
                            )}
                            <SyncBadge mode={d.sync_mode} />
                          </div>
                        </div>
                      </button>
                    </li>
                  );
                })}
              </ul>
            </section>
          ))}
        </div>

        <footer className="flex items-center justify-end gap-2 border-t border-ink-50/[0.05] bg-ink-900/40 px-7 py-4">
          <button className="btn" onClick={onClose}>
            {t.cloud.cancel}
          </button>
        </footer>
      </div>
    </div>
  );
}

function Badge({
  color,
  label,
}: {
  color: "signal" | "neutral" | "warn";
  label: string;
}) {
  const cls =
    color === "signal"
      ? "bg-signal/15 text-signal-glow ring-signal/30"
      : color === "warn"
        ? "bg-status-unknown/15 text-status-unknown ring-status-unknown/30"
        : "bg-ink-50/[0.06] text-ink-100 ring-ink-50/10";
  return (
    <span
      className={[
        "rounded px-2 py-0.5 font-mono text-[10px] uppercase tracking-[0.2em] ring-1",
        cls,
      ].join(" ")}
    >
      {label}
    </span>
  );
}

function SyncBadge({ mode }: { mode: SyncMode }) {
  return mode === "live" ? (
    <Badge color="signal" label={t.cloud.liveBadge} />
  ) : (
    <Badge color="warn" label={t.cloud.offlineBadge} />
  );
}

function formatDate(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return iso;
  return `${m[3]}.${m[2]}.${m[1]}`;
}
