import { useMemo, useState } from "react";
import { defaultCsvFilename, entriesToCsv } from "../lib/csv";
import { t } from "../lib/i18n";
import { saveTextFile } from "../lib/tauri";
import { formatCorrection, formatTime } from "../lib/format";
import { totalTimeMs, type RunEntry } from "../lib/types";

interface Props {
  entries: RunEntry[];
  onBack: () => void;
  onClear: () => void;
}

type SortKey = "newest" | "oldest" | "fastest" | "slowest" | "lane";

export function ProtocolView({ entries, onBack, onClear }: Props) {
  const [sortKey, setSortKey] = useState<SortKey>("newest");
  const [confirmingClear, setConfirmingClear] = useState(false);
  const [exportFlash, setExportFlash] = useState<
    { kind: "ok"; path: string } | { kind: "err"; message: string } | null
  >(null);

  const sorted = useMemo(() => sortEntries(entries, sortKey), [entries, sortKey]);

  const handleExport = async () => {
    try {
      const filename = defaultCsvFilename();
      const csv = entriesToCsv(entries);
      const path = await saveTextFile(filename, csv, [
        { name: "CSV", extensions: ["csv"] },
      ]);
      if (path) {
        setExportFlash({ kind: "ok", path });
        window.setTimeout(() => setExportFlash(null), 4500);
      }
    } catch (err) {
      setExportFlash({ kind: "err", message: String(err) });
      window.setTimeout(() => setExportFlash(null), 6000);
    }
  };

  return (
    <div className="flex h-full flex-col px-7 py-6">
      {/* Header */}
      <div className="flex items-center justify-between gap-6">
        <div className="flex items-center gap-4">
          <button
            type="button"
            onClick={onBack}
            className="grid h-10 w-10 place-items-center rounded-xl border border-ink-50/10 bg-ink-50/[0.04] text-ink-50 transition hover:bg-ink-50/10"
            aria-label={t.drawer.backToLanes}
          >
            ←
          </button>
          <div>
            <span className="label">{t.protocol.subtitle}</span>
            <h1 className="font-display text-[26px] font-medium tracking-tight text-ink-50">
              {t.protocol.title}
            </h1>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <SortPicker value={sortKey} onChange={setSortKey} />
          <button
            className="btn"
            disabled={entries.length === 0}
            onClick={() => setConfirmingClear(true)}
          >
            {t.protocol.clearAll}
          </button>
          <button
            className="btn btn-primary"
            disabled={entries.length === 0}
            onClick={handleExport}
          >
            ⬇ {t.protocol.exportCsv}
          </button>
        </div>
      </div>

      {/* KPI strip */}
      <div className="mt-4 grid grid-cols-4 gap-3 max-w-3xl">
        <Kpi label={t.protocol.kpiCount} value={entries.length.toString()} />
        <Kpi label={t.protocol.kpiFastest} value={fastest(entries)} />
        <Kpi label={t.protocol.kpiSlowest} value={slowest(entries)} />
        <Kpi label={t.protocol.kpiCorrected} value={correctedCount(entries).toString()} />
      </div>

      {/* Table */}
      <div className="mt-6 surface flex min-h-0 flex-1 flex-col overflow-hidden">
        {entries.length === 0 ? (
          <div className="grid flex-1 place-items-center px-6 py-20 text-center">
            <div className="flex flex-col items-center gap-4">
              <div className="grid h-14 w-14 place-items-center rounded-full border border-ink-50/10 bg-ink-50/[0.02]">
                <span className="font-stencil text-[22px] text-ink-300/80">∅</span>
              </div>
              <p className="font-mono text-[11px] uppercase tracking-[0.2em] text-ink-200/60">
                {t.protocol.empty}
              </p>
            </div>
          </div>
        ) : (
          <div className="no-scrollbar relative flex-1 overflow-auto">
            <table className="w-full border-separate border-spacing-0">
              <thead className="sticky top-0 z-10 bg-ink-800/95 backdrop-blur">
                <tr className="text-left">
                  <Th className="w-20">#</Th>
                  <Th className="w-28">{t.protocol.columnLane}</Th>
                  <Th>{t.protocol.columnOriginal}</Th>
                  <Th>{t.protocol.columnPenalty}</Th>
                  <Th>{t.protocol.columnTotal}</Th>
                  <Th className="w-32">{t.protocol.columnTimestamp}</Th>
                  <Th className="w-32">{t.protocol.columnDate}</Th>
                </tr>
              </thead>
              <tbody>
                {sorted.map((entry) => {
                  const total = totalTimeMs(entry);
                  const hasPenalty = entry.correctionMs !== 0;
                  return (
                    <tr key={entry.id} className="group">
                      <Td className="text-ink-300/80 font-mono tnum w-20">
                        #{entries.length - originalIndex(entries, entry)}
                      </Td>
                      <Td>
                        <span className="font-stencil text-[15px] text-ink-50">
                          {t.protocol.columnLane} {entry.channel}
                        </span>
                      </Td>
                      <Td>
                        <span className="font-mono tnum text-[16px] text-ink-100">
                          {formatTime(entry.originalTimeMs, "confirmed")}
                        </span>
                      </Td>
                      <Td>
                        <span
                          className={[
                            "font-mono tnum text-[14px]",
                            hasPenalty ? "text-status-armed" : "text-ink-300/60",
                          ].join(" ")}
                        >
                          {formatCorrection(entry.correctionMs)}
                        </span>
                      </Td>
                      <Td>
                        <span
                          className={[
                            "font-mono tnum text-[20px] tracking-tight",
                            hasPenalty ? "text-status-armed" : "text-status-confirmed",
                          ].join(" ")}
                        >
                          {formatTime(total, "confirmed")}
                        </span>
                      </Td>
                      <Td className="font-mono tnum text-[12px] text-ink-100">
                        {new Date(entry.timestamp).toLocaleTimeString("de-DE", {
                          hour12: false,
                        })}
                      </Td>
                      <Td className="font-mono tnum text-[12px] text-ink-200/80">
                        {new Date(entry.timestamp).toLocaleDateString("de-DE")}
                      </Td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {confirmingClear && (
        <div className="fixed inset-0 z-40 grid place-items-center px-6">
          <div
            className="absolute inset-0 bg-ink-900/85 backdrop-blur"
            onClick={() => setConfirmingClear(false)}
          />
          <div className="surface relative z-10 w-full max-w-md p-6">
            <h3 className="font-display text-[18px] font-medium text-ink-50">
              {t.protocol.clearAll}
            </h3>
            <p className="mt-2 font-mono text-[12px] text-ink-200/80">
              {t.protocol.clearConfirm}
            </p>
            <div className="mt-5 grid grid-cols-2 gap-2">
              <button className="btn" onClick={() => setConfirmingClear(false)}>
                {t.setup.cancel}
              </button>
              <button
                className="btn btn-danger"
                onClick={() => {
                  onClear();
                  setConfirmingClear(false);
                }}
              >
                {t.protocol.clearConfirmAction}
              </button>
            </div>
          </div>
        </div>
      )}

      {exportFlash && (
        <div className="pointer-events-none fixed bottom-6 left-1/2 z-30 -translate-x-1/2">
          <div
            className={[
              "surface px-4 py-3 ring-1",
              exportFlash.kind === "ok"
                ? "ring-status-confirmed/40 bg-status-confirmed/10"
                : "ring-status-unknown/40 bg-status-unknown/10",
            ].join(" ")}
          >
            <p className="font-mono text-[12px] text-ink-50 break-all">
              {exportFlash.kind === "ok"
                ? `${t.protocol.saved}${exportFlash.path}`
                : `${t.protocol.exportFailed}: ${exportFlash.message}`}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}

function Th({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return (
    <th
      className={[
        "border-b border-ink-50/[0.06] px-5 py-3 text-left",
        "label !text-ink-200/70",
        className,
      ].join(" ")}
    >
      {children}
    </th>
  );
}

function Td({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return (
    <td
      className={[
        "border-b border-ink-50/[0.04] px-5 py-3 align-middle",
        "group-hover:bg-ink-50/[0.02] transition-colors",
        className,
      ].join(" ")}
    >
      {children}
    </td>
  );
}

function SortPicker({ value, onChange }: { value: SortKey; onChange: (k: SortKey) => void }) {
  const options: Array<{ key: SortKey; label: string }> = [
    { key: "newest", label: "Neueste zuerst" },
    { key: "oldest", label: "Älteste zuerst" },
    { key: "fastest", label: "Schnellste zuerst" },
    { key: "slowest", label: "Langsamste zuerst" },
    { key: "lane", label: "Nach Bahn" },
  ];
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as SortKey)}
      className="input appearance-none !py-1.5 !text-[12px]"
    >
      {options.map((o) => (
        <option key={o.key} value={o.key}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

function Kpi({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-ink-50/[0.06] bg-ink-700/40 px-4 py-3">
      <span className="label">{label}</span>
      <p className="mt-1 font-mono tnum text-[18px] text-ink-50">{value}</p>
    </div>
  );
}

function sortEntries(entries: RunEntry[], key: SortKey): RunEntry[] {
  const copy = [...entries];
  switch (key) {
    case "newest":
      return copy.sort((a, b) => b.timestamp - a.timestamp);
    case "oldest":
      return copy.sort((a, b) => a.timestamp - b.timestamp);
    case "fastest":
      return copy.sort((a, b) => totalTimeMs(a) - totalTimeMs(b));
    case "slowest":
      return copy.sort((a, b) => totalTimeMs(b) - totalTimeMs(a));
    case "lane":
      return copy.sort((a, b) => a.channel - b.channel || a.timestamp - b.timestamp);
  }
}

function originalIndex(entries: RunEntry[], entry: RunEntry): number {
  return entries.findIndex((e) => e.id === entry.id);
}

function fastest(entries: RunEntry[]): string {
  if (entries.length === 0) return "—";
  const min = entries.reduce((a, b) => (totalTimeMs(a) < totalTimeMs(b) ? a : b));
  return formatTime(totalTimeMs(min), "confirmed");
}

function slowest(entries: RunEntry[]): string {
  if (entries.length === 0) return "—";
  const max = entries.reduce((a, b) => (totalTimeMs(a) > totalTimeMs(b) ? a : b));
  return formatTime(totalTimeMs(max), "confirmed");
}

function correctedCount(entries: RunEntry[]): number {
  return entries.filter((e) => e.correctionMs !== 0).length;
}
