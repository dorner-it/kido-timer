import { useCallback, useEffect, useState } from "react";
import { computeResults } from "../lib/cloudApi";
import type { DisciplinePayload, SyncMode } from "../lib/cloudTypes";
import { modeLabelDE, t, wgDescriptionDE, wgLabelDE } from "../lib/i18n";
import {
  anyProvisional,
  buildTagessiegerRows,
  buildWgRows,
  maxRunsPerTeam,
  teamsWithoutWgNames,
  type ResultsResponse,
} from "../lib/results";
import { syncModeLabel } from "../lib/syncMode";

interface Props {
  snapshot: DisciplinePayload | null;
  effectiveMode: SyncMode | null;
  onBack: () => void;
}

const APP_VERSION = "0.6.0";

/**
 * WG rankings + Tagessieger, computed locally with `kido_core::scoring`
 * (works on-prem). Print layout: A4, one WG per page, Tagessieger on its
 * own page, header/footer on every section (see `@media print` in
 * styles.css).
 */
export function ResultsView({ snapshot, effectiveMode, onBack }: Props) {
  const [data, setData] = useState<ResultsResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!snapshot) {
      setData(null);
      return;
    }
    try {
      setData(await computeResults());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, [snapshot]);

  // Recompute whenever the snapshot changes (new times, decided errors).
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const printedAt = new Date().toLocaleString("de-DE");
  const provisional = data ? anyProvisional(data.results) : false;
  const missingWg = data && snapshot ? teamsWithoutWgNames(data, snapshot) : [];

  const header = snapshot && (
    <PrintHeader snapshot={snapshot} provisional={provisional} />
  );
  const footer = (
    <p className="print-only mt-4 border-t border-black/30 pt-1 text-[8pt]">
      {t.results.legend} · {t.results.printedAt}: {printedAt} · KiDo-Timer {APP_VERSION}
      {effectiveMode ? ` · ${syncModeLabel(effectiveMode)}` : ""}
    </p>
  );

  return (
    <div className="results-root flex h-full flex-col px-7 py-6">
      <div className="no-print flex items-center justify-between gap-6">
        <div className="flex items-center gap-4">
          <button
            type="button"
            onClick={onBack}
            className="grid h-10 w-10 place-items-center rounded-xl border border-ink-50/10 bg-ink-50/[0.04] text-ink-50 transition hover:bg-ink-50/10"
            aria-label={t.results.back}
          >
            ←
          </button>
          <div>
            <span className="label">{t.results.kicker}</span>
            <h1 className="font-display text-[26px] font-medium tracking-tight text-ink-50">
              {t.results.title}
            </h1>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <button className="btn" onClick={() => void refresh()} disabled={!snapshot}>
            ↻ {t.results.refresh}
          </button>
          <button
            className="btn btn-primary"
            onClick={() => window.print()}
            disabled={!data}
          >
            🖨 {t.results.print}
          </button>
        </div>
      </div>

      <div className="results-scroll mt-5 min-h-0 flex-1 overflow-auto">
        {!snapshot && <Notice>{t.results.noDiscipline}</Notice>}
        {error && (
          <div className="mb-4 rounded-md border border-status-unknown/40 bg-status-unknown/5 px-3 py-2 font-mono text-[11px] text-status-unknown">
            {error}
          </div>
        )}
        {snapshot && data && (
          <>
            <div className="no-print mb-4 flex flex-col gap-1 font-mono text-[11px] text-ink-200/80">
              <span>{t.results.legend}</span>
              {provisional && <span className="text-status-armed">{t.results.provisional}</span>}
              {missingWg.length > 0 && (
                <span className="text-status-unknown">
                  {t.results.missingWg(missingWg.join(", "))}
                </span>
              )}
            </div>

            {data.results.wg_rankings.length === 0 && <Notice>{t.results.empty}</Notice>}

            {data.results.wg_rankings.map((ranking) => {
              const rows = buildWgRows(ranking, snapshot);
              const runColumns = maxRunsPerTeam(ranking);
              return (
                <section key={ranking.wg} className="print-page results-section mb-8">
                  {header}
                  <h2 className="font-display text-[18px] font-medium text-ink-50">
                    {wgLabelDE(ranking.wg)}
                    <span className="ml-2 font-mono text-[12px] text-ink-200/70">
                      {wgDescriptionDE(ranking.wg)}
                    </span>
                  </h2>
                  <table className="results-table mt-2 w-full border-separate border-spacing-0">
                    <thead>
                      <tr>
                        <Th className="w-16">{t.results.colPlace}</Th>
                        <Th className="w-20">{t.results.colStart}</Th>
                        <Th>{t.results.colTeam}</Th>
                        {Array.from({ length: runColumns }, (_, i) => (
                          <Th key={i}>{t.results.colRun(i + 1)}</Th>
                        ))}
                        <Th>{t.results.colBest}</Th>
                        <Th>{t.results.colNote}</Th>
                      </tr>
                    </thead>
                    <tbody>
                      {rows.map((row) => (
                        <tr key={row.key}>
                          <Td strong>{row.place}</Td>
                          <Td>{row.startNumber}</Td>
                          <Td>{row.teamName}</Td>
                          {row.runCells.map((cell, i) => (
                            <Td key={i} mono>
                              {cell}
                            </Td>
                          ))}
                          <Td mono strong>
                            {row.best}
                          </Td>
                          <Td>{row.note}</Td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {footer}
                </section>
              );
            })}

            <section className="print-page results-section mb-8">
              {header}
              <h2 className="font-display text-[18px] font-medium text-ink-50">
                {t.results.tagessieger}
              </h2>
              <p className="font-mono text-[11px] text-ink-200/70">{t.results.tagessiegerHint}</p>
              {data.results.tagessieger.length === 0 ? (
                <Notice>{t.results.noTagessieger}</Notice>
              ) : (
                <table className="results-table mt-2 w-full border-separate border-spacing-0">
                  <thead>
                    <tr>
                      <Th className="w-16">{t.results.colPlace}</Th>
                      <Th className="w-20">{t.results.colStart}</Th>
                      <Th>{t.results.colTeam}</Th>
                      <Th>{t.results.colWg}</Th>
                      <Th>{t.results.colTime}</Th>
                      <Th>{t.results.colAvgAge}</Th>
                      <Th>{t.results.colFactor}</Th>
                      <Th>{t.results.colGw}</Th>
                      <Th>{t.results.colNote}</Th>
                    </tr>
                  </thead>
                  <tbody>
                    {buildTagessiegerRows(data.results.tagessieger, snapshot).map((row) => (
                      <tr key={row.key}>
                        <Td strong>{row.place}</Td>
                        <Td>{row.startNumber}</Td>
                        <Td>{row.teamName}</Td>
                        <Td>{row.wg}</Td>
                        <Td mono>{row.time}</Td>
                        <Td mono>{row.avgAge}</Td>
                        <Td mono>{row.factor}</Td>
                        <Td mono strong>
                          {row.gw}
                        </Td>
                        <Td>{row.note}</Td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              {footer}
            </section>
          </>
        )}
      </div>
    </div>
  );
}

function PrintHeader({
  snapshot,
  provisional,
}: {
  snapshot: DisciplinePayload;
  provisional: boolean;
}) {
  return (
    <div className="print-only mb-3 border-b border-black/40 pb-1">
      <p className="text-[13pt] font-bold">
        {snapshot.event.name} · {snapshot.discipline.name}
      </p>
      <p className="text-[9pt]">
        {formatDate(snapshot.event.date)}
        {snapshot.event.location ? ` · ${snapshot.event.location}` : ""} ·{" "}
        {modeLabelDE(snapshot.discipline.mode, snapshot.discipline.lane_count)}
        {provisional ? ` · ${t.results.provisional}` : ""}
      </p>
    </div>
  );
}

function Notice({ children }: { children: React.ReactNode }) {
  return <p className="py-6 font-mono text-[12px] text-ink-200/70">{children}</p>;
}

function Th({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return (
    <th
      className={[
        "border-b border-ink-50/[0.10] px-3 py-2 text-left label !text-ink-200/80",
        className,
      ].join(" ")}
    >
      {children}
    </th>
  );
}

function Td({
  children,
  mono,
  strong,
}: {
  children: React.ReactNode;
  mono?: boolean;
  strong?: boolean;
}) {
  return (
    <td
      className={[
        "border-b border-ink-50/[0.05] px-3 py-1.5 align-middle text-[13px] text-ink-100",
        mono ? "font-mono tnum" : "font-display",
        strong ? "font-semibold text-ink-50" : "",
      ].join(" ")}
    >
      {children}
    </td>
  );
}

function formatDate(iso: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return iso;
  return `${m[3]}.${m[2]}.${m[1]}`;
}
