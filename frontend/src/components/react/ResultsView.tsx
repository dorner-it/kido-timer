// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useCallback, useEffect, useMemo, useState } from 'react';

import { ApiError, TIMER_MODE_LABELS, apiClient } from '../../lib/api';
import { startPolling } from '../../lib/poller';
import {
  HAND_TIME_MARKER,
  PROVISIONAL_MARKER,
  TAGESSIEGER_LABEL,
  buildTagessiegerRows,
  buildWgTableRows,
  isProvisional,
  resolveTab,
  resultsTabs,
  runColumnCount,
  sortWgRankings,
  type DisciplineResults,
  type ResultsTab,
  type RunCell,
  type TagessiegerTableRow,
  type WgRanking,
  type WgTableRow,
} from '../../lib/results';
import { formatDateNumericDE, formatTimestampDE } from '../../lib/time-format';
import '../../styles/results-print.css';

/** Auto-refresh interval while the page is visible. */
const REFRESH_MS = 5000;

interface Props {
  eventId: string;
  disciplineId: string;
}

export default function ResultsView({ eventId, disciplineId }: Props) {
  const [data, setData] = useState<DisciplineResults | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<ResultsTab | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const res = await apiClient.results.get(eventId, disciplineId);
      setData(res);
      setError(null);
    } catch (e) {
      setError(
        e instanceof ApiError
          ? e.status === 404
            ? 'Wettkampf nicht gefunden.'
            : e.message
          : 'Ergebnisse konnten nicht geladen werden.',
      );
    } finally {
      setLoading(false);
    }
  }, [eventId, disciplineId]);

  useEffect(() => startPolling(load, { intervalMs: REFRESH_MS }), [load]);

  useEffect(() => {
    if (data) setTab((current) => resolveTab(data, current));
  }, [data]);

  if (!data) {
    return (
      <main className="mx-auto w-full max-w-6xl px-7 py-10">
        {error ? (
          <p className="surface px-5 py-3 text-[13px] text-status-unknown">{error}</p>
        ) : (
          <p className="label">Lade Ergebnisse …</p>
        )}
      </main>
    );
  }

  return (
    <>
      <ResultsScreen
        data={data}
        eventId={eventId}
        tab={tab ?? resolveTab(data, null)}
        onTab={setTab}
        error={error}
        loading={loading}
        onReload={() => void load()}
      />
      <ResultsPrint data={data} />
    </>
  );
}

// ===== screen =====

function ResultsScreen({
  data,
  eventId,
  tab,
  onTab,
  error,
  loading,
  onReload,
}: {
  data: DisciplineResults;
  eventId: string;
  tab: ResultsTab;
  onTab: (t: ResultsTab) => void;
  error: string | null;
  loading: boolean;
  onReload: () => void;
}) {
  const tabs = useMemo(() => resultsTabs(data), [data]);
  const rankings = useMemo(() => sortWgRankings(data.wg_rankings), [data]);
  const provisional = isProvisional(data);
  const selected = rankings.find((w) => w.wg === tab);

  return (
    <main className="results-screen mx-auto w-full max-w-6xl px-7 py-10 print:hidden">
      <header className="mb-6 flex flex-wrap items-end justify-between gap-6">
        <div className="flex flex-col gap-2">
          <a href={`/events/${eventId}`} className="label hover:text-ink-50">
            ← {data.event.name}
          </a>
          <h1 className="font-stencil text-[40px] font-bold leading-none tracking-tight text-ink-50">
            Ergebnisse · {data.discipline.name}
          </h1>
          <p className="font-mono text-[12px] text-ink-200">
            {TIMER_MODE_LABELS[data.discipline.mode]} · {formatDateNumericDE(data.event.date)}
            {data.event.location ? ` · ${data.event.location}` : ''}
          </p>
        </div>
        <div className="flex flex-col items-end gap-2">
          <div className="flex items-center gap-2">
            <button type="button" className="btn" onClick={onReload} disabled={loading}>
              {loading ? 'Lädt …' : '↻ Aktualisieren'}
            </button>
            <button type="button" className="btn btn-primary" onClick={() => window.print()}>
              Drucken
            </button>
          </div>
          <span className="font-mono text-[11px] text-ink-300">
            Stand: {formatTimestampDE(data.generated_at)} · automatische Aktualisierung alle 5 s
          </span>
        </div>
      </header>

      {provisional && (
        <div
          role="status"
          className="mb-5 rounded-xl border border-status-armed/40 bg-status-armed/10 px-5 py-3 text-[13px] text-status-armed"
        >
          <strong className="font-semibold">Vorläufig:</strong> Es sind noch nicht alle Fehler
          bestätigt. Mit {PROVISIONAL_MARKER} markierte Ergebnisse können sich noch ändern.
        </div>
      )}
      {error && (
        <p className="mb-5 rounded-xl border border-status-unknown/40 bg-status-unknown/10 px-5 py-3 text-[13px] text-status-unknown">
          Aktualisierung fehlgeschlagen: {error}
        </p>
      )}
      {data.missing_wg.length > 0 && (
        <p className="mb-5 rounded-xl border border-status-unknown/30 bg-status-unknown/5 px-5 py-3 text-[13px] text-ink-100">
          <strong className="font-semibold">WG fehlt – nicht gewertet:</strong>{' '}
          {data.missing_wg
            .map((t) => `${t.team_name}${t.start_number ? ` (Nr. ${t.start_number})` : ''}`)
            .join(', ')}
          . Bitte in den Meldungen eine Wertungsgruppe setzen.
        </p>
      )}

      <nav className="mb-6 flex flex-wrap gap-2 border-b border-[color:var(--hairline-color)]">
        {tabs.map((t) => (
          <button
            key={t.key}
            type="button"
            onClick={() => onTab(t.key)}
            className={[
              'border-b-2 px-4 py-2 font-mono text-[11px] uppercase tracking-[0.18em] transition',
              t.key === tab
                ? 'border-signal text-ink-50'
                : 'border-transparent text-ink-200 hover:text-ink-100',
            ].join(' ')}
          >
            {t.label} ({t.count})
          </button>
        ))}
      </nav>

      {tab === 'tagessieger' ? (
        <section className="surface overflow-x-auto p-4">
          <TagessiegerTable rows={buildTagessiegerRows(data.tagessieger)} />
        </section>
      ) : selected ? (
        <section className="surface overflow-x-auto p-4">
          <h2 className="mb-3 font-display text-[18px] font-semibold text-ink-50">
            {selected.wg_label} · {selected.wg_label_de}
          </h2>
          <WgTable ranking={selected} />
        </section>
      ) : (
        <p className="surface px-5 py-4 text-[13px] text-ink-200">
          Noch keine Meldungen mit Wertungsgruppe.
        </p>
      )}

      <Legend />
    </main>
  );
}

function Legend() {
  return (
    <p className="results-legend mt-4 font-mono text-[11px] text-ink-300">
      SZ = Strafzeit (+15 s je Fehler) · FV = Fehlversuch (Lauf ungültig) · {HAND_TIME_MARKER} =
      Handzeit · {PROVISIONAL_MARKER} = vorläufig · n.a. = nicht angetreten · fett = gewertete
      Bestzeit
    </p>
  );
}

// ===== tables (shared by screen and print) =====

function WgTable({ ranking }: { ranking: WgRanking }) {
  const columns = runColumnCount(ranking.rows);
  const rows = buildWgTableRows(ranking.rows, columns);
  if (rows.length === 0) {
    return <p className="text-[13px] text-ink-200">Keine Mannschaften.</p>;
  }
  return (
    <table className="results-table w-full border-collapse text-left text-[13px]">
      <thead>
        <tr className="label">
          <th className="w-14 py-2 pr-2">Platz</th>
          <th className="w-16 py-2 pr-2">Startnr.</th>
          <th className="py-2 pr-2">Mannschaft</th>
          {Array.from({ length: columns }, (_, i) => (
            <th key={i} className="py-2 pr-2 text-right">
              Lauf {i + 1}
            </th>
          ))}
          <th className="py-2 text-right">Bestzeit</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <WgRow key={row.key} row={row} />
        ))}
      </tbody>
    </table>
  );
}

function WgRow({ row }: { row: WgTableRow }) {
  return (
    <tr
      className={[
        'border-t border-[color:var(--hairline-color)]',
        row.groupStart ? 'results-group-start' : '',
        row.status === 'not_started' ? 'text-ink-300' : '',
      ].join(' ')}
    >
      <td className="py-2 pr-2 font-mono tnum font-semibold">{row.place}</td>
      <td className="py-2 pr-2 font-mono tnum">{row.startNumber}</td>
      <td className="py-2 pr-2">
        {row.teamName}
        {row.provisional && <ProvisionalMark />}
      </td>
      {row.runs.map((cell, i) => (
        <td key={i} className="py-2 pr-2 text-right">
          <RunCellView cell={cell} />
        </td>
      ))}
      <td className="py-2 text-right font-mono tnum font-semibold">{row.best}</td>
    </tr>
  );
}

function RunCellView({ cell }: { cell: RunCell }) {
  return (
    <span title={cell.title || undefined} className="inline-flex items-baseline justify-end gap-1">
      <span
        className={[
          'font-mono tnum',
          cell.best ? 'font-bold' : '',
          cell.invalid ? 'results-invalid text-status-unknown' : '',
        ].join(' ')}
      >
        {cell.text}
      </span>
      {cell.sz && (
        <span className="results-sz font-mono text-[10px] text-status-captured">{cell.sz}</span>
      )}
      {cell.hand && (
        <span
          className="results-hand rounded border border-current px-0.5 font-mono text-[9px] leading-none text-ink-200"
          aria-label="Handzeit"
        >
          {HAND_TIME_MARKER}
        </span>
      )}
      {cell.provisional && <ProvisionalMark />}
    </span>
  );
}

function ProvisionalMark() {
  return (
    <span className="results-provisional ml-0.5 text-status-armed" title="Vorläufig" aria-label="vorläufig">
      {PROVISIONAL_MARKER}
    </span>
  );
}

function TagessiegerTable({ rows }: { rows: TagessiegerTableRow[] }) {
  if (rows.length === 0) {
    return <p className="text-[13px] text-ink-200">Noch keine gültigen Läufe.</p>;
  }
  return (
    <table className="results-table w-full border-collapse text-left text-[13px]">
      <thead>
        <tr className="label">
          <th className="w-14 py-2 pr-2">Platz</th>
          <th className="py-2 pr-2">Mannschaft</th>
          <th className="py-2 pr-2">WG</th>
          <th className="py-2 pr-2 text-right">⌀ Alter</th>
          <th className="py-2 pr-2 text-right">Faktor</th>
          <th className="py-2 pr-2 text-right">Bestzeit</th>
          <th className="py-2 text-right">GW-Zeit</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={r.key} className="border-t border-[color:var(--hairline-color)]" title={r.title}>
            <td className="py-2 pr-2 font-mono tnum font-semibold">{r.place}</td>
            <td className="py-2 pr-2">
              {r.teamName}
              {r.startNumber !== '—' && (
                <span className="ml-1 font-mono text-[11px] text-ink-300">(Nr. {r.startNumber})</span>
              )}
              {r.provisional && <ProvisionalMark />}
              {r.fallback && (
                <span className="results-fallback ml-2 rounded border border-status-armed/50 px-1 font-mono text-[10px] text-status-armed">
                  {r.fallback}
                </span>
              )}
            </td>
            <td className="py-2 pr-2 font-mono">{r.wg}</td>
            <td className="py-2 pr-2 text-right font-mono tnum">{r.avgAge}</td>
            <td className="py-2 pr-2 text-right font-mono tnum">{r.factor}</td>
            <td className="py-2 pr-2 text-right font-mono tnum">{r.best}</td>
            <td className="py-2 text-right font-mono tnum font-bold">{r.gw}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ===== print =====

/**
 * Print-only tree (`hidden print:block`): one A4 page per WG, the
 * Tagessieger on its own page. Each page is a layout table whose
 * `thead`/`tfoot` (event header / "Stand") repeat when a WG spans
 * several pages. Styles: `styles/results-print.css`.
 */
function ResultsPrint({ data }: { data: DisciplineResults }) {
  const rankings = sortWgRankings(data.wg_rankings);
  const provisional = isProvisional(data);
  const stand = formatTimestampDE(data.generated_at);
  return (
    <div className="results-print hidden print:block" aria-hidden="true">
      {rankings.map((w) => (
        <PrintPage
          key={w.wg}
          data={data}
          title={`${w.wg_label} · ${w.wg_label_de}`}
          provisional={provisional}
          stand={stand}
        >
          <WgTable ranking={w} />
        </PrintPage>
      ))}
      <PrintPage data={data} title={TAGESSIEGER_LABEL} provisional={provisional} stand={stand}>
        <TagessiegerTable rows={buildTagessiegerRows(data.tagessieger)} />
        <p className="results-print-note">
          GW-Zeit = Bestzeit × Faktor; Faktor = 1 − 2 · (18 − ⌀ Alter) / 100. Alter in vollendeten
          Jahren am Wettkampftag, gemittelt über die Besetzung des gewerteten Laufs.
        </p>
      </PrintPage>
    </div>
  );
}

function PrintPage({
  data,
  title,
  provisional,
  stand,
  children,
}: {
  data: DisciplineResults;
  title: string;
  provisional: boolean;
  stand: string;
  children: React.ReactNode;
}) {
  return (
    <table className="results-print-page">
      <thead>
        <tr>
          <td>
            <div className="results-print-header">
              <div className="results-print-event">
                <strong>{data.event.name}</strong>
                <span>
                  {formatDateNumericDE(data.event.date)}
                  {data.event.location ? ` · ${data.event.location}` : ''}
                </span>
              </div>
              <div className="results-print-discipline">
                {data.discipline.name}
                {provisional && <span className="results-print-provisional">Vorläufig</span>}
              </div>
              <h2 className="results-print-title">{title}</h2>
            </div>
          </td>
        </tr>
      </thead>
      <tfoot>
        <tr>
          <td>
            <div className="results-print-footer">
              <span>Stand: {stand}</span>
              <span>
                SZ = +15 s · FV = Fehlversuch · {HAND_TIME_MARKER} = Handzeit ·{' '}
                {PROVISIONAL_MARKER} = vorläufig · n.a. = nicht angetreten
              </span>
            </div>
          </td>
        </tr>
      </tfoot>
      <tbody>
        <tr>
          <td>{children}</td>
        </tr>
      </tbody>
    </table>
  );
}
