// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  RUN_STATUS_LABELS,
  apiClient,
  type CatalogueEntry,
  type EventDetail as EventDetailData,
  type Penalty,
  type Run,
  type RunStatus,
} from '../../lib/api';
import { effectiveLane } from '../../lib/current-runs';
import { effectiveLaneCount } from '../../lib/lanes';
import { lineupMissing, rosterForTeam } from '../../lib/lineup';
import { indexCatalogue } from '../../lib/penalties';
import { createPoller, pollIntervalMs, type Poller } from '../../lib/polling';
import { markRunApproved, penaltiesByRun, replacePenalty, summarizeRun } from '../../lib/run-review';
import { RUN_STATUSES, createRunBody, entryForRun, roundBodies, sortRuns } from '../../lib/runs';
import { wgLabel, WG_MISSING_LABEL } from '../../lib/wg';
import CurrentRunsControl from './CurrentRunsControl';
import RunReviewCard, { LineupMissingBadge, SummaryBadge } from './RunReviewCard';
import { RunTime } from './RunTime';

interface Props {
  data: EventDetailData;
  onChange: () => void | Promise<void>;
}

const EMPTY: Penalty[] = [];

/**
 * "Läufe": current runs per lane, live review of errors, hand times,
 * lineup correction and the run list of a discipline.
 */
export default function RunsPanel({ data, onChange }: Props) {
  const [live, setLive] = useState<EventDetailData>(data);
  const [penalties, setPenalties] = useState<Penalty[] | null>(null);
  const [catalogue, setCatalogue] = useState<CatalogueEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [stale, setStale] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [disciplineId, setDisciplineId] = useState<string>(
    data.event.current_discipline_id ?? [...data.disciplines].sort((a, b) => a.position - b.position)[0]?.id ?? '',
  );

  const { event, disciplines, teams, runners } = live;
  const discipline = disciplines.find((d) => d.id === disciplineId) ?? null;
  const isActiveDiscipline = discipline != null && event.current_discipline_id === discipline.id;
  const laneCount = discipline ? effectiveLaneCount(discipline) : 1;
  const currentIds = isActiveDiscipline ? (event.current_run_ids ?? []) : [];

  // Parent data changed (e.g. after another tab saved) → adopt it.
  useEffect(() => setLive(data), [data]);
  // Keep the parent in sync when leaving the tab.
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  useEffect(() => () => void onChangeRef.current(), []);

  useEffect(() => {
    let cancelled = false;
    apiClient.catalogue
      .list()
      .then((c) => !cancelled && setCatalogue(c))
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);
  const catalogueIndex = useMemo(() => indexCatalogue(catalogue), [catalogue]);

  // ===== polling =====
  const load = useCallback(async () => {
    const [detail, list] = await Promise.all([
      apiClient.events.detail(event.id),
      disciplineId ? apiClient.penalties.list(event.id, { discipline_id: disciplineId }) : Promise.resolve([]),
    ]);
    setLive(detail);
    setPenalties(list);
    setStale(false);
  }, [event.id, disciplineId]);
  const loadRef = useRef(load);
  loadRef.current = load;

  const pollerRef = useRef<Poller | null>(null);
  useEffect(() => {
    const p = createPoller(
      () => loadRef.current(),
      pollIntervalMs(false),
      undefined,
      () => setStale(true),
    );
    pollerRef.current = p;
    p.start();
    return () => p.stop();
  }, []);
  const hasCurrent = currentIds.length > 0;
  useEffect(() => {
    pollerRef.current?.setInterval(pollIntervalMs(hasCurrent));
  }, [hasCurrent]);
  // Initial load + reload on discipline change.
  useEffect(() => {
    setPenalties(null);
    void pollerRef.current?.trigger();
  }, [disciplineId]);

  const refreshNow = () => pollerRef.current?.trigger();

  // ===== derived =====
  const entries = useMemo(
    () => live.team_entries.filter((e) => e.discipline_id === disciplineId),
    [live.team_entries, disciplineId],
  );
  const runs = useMemo(
    () => sortRuns(live.runs.filter((r) => r.discipline_id === disciplineId)),
    [live.runs, disciplineId],
  );
  const byRun = useMemo(() => penaltiesByRun(penalties ?? []), [penalties]);
  const teamById = useMemo(() => new Map(teams.map((t) => [t.id, t])), [teams]);
  const currentRuns = currentIds
    .map((id) => runs.find((r) => r.id === id))
    .filter((r): r is Run => r != null);

  const updateRun = (r: Run) =>
    setLive((cur) => ({ ...cur, runs: cur.runs.map((x) => (x.id === r.id ? r : x)) }));

  const cardFor = (run: Run, laneLabel?: string) => (
    <RunReviewCard
      key={run.id}
      eventId={event.id}
      disciplineId={disciplineId}
      run={run}
      team={teamById.get(run.team_id)}
      entry={entryForRun(run, entries)}
      roster={rosterForTeam(runners, run.team_id)}
      penalties={byRun.get(run.id) ?? EMPTY}
      catalogue={catalogueIndex}
      laneLabel={laneLabel}
      highlight={laneLabel != null}
      onPenaltyUpdated={(p) => setPenalties((cur) => replacePenalty(cur ?? [], p))}
      onRunUpdated={updateRun}
      onTakenOver={async (runId) => {
        setPenalties((cur) => markRunApproved(cur ?? [], runId));
        await refreshNow();
      }}
      onError={setError}
    />
  );

  if (disciplines.length === 0) {
    return (
      <p className="surface px-5 py-4 text-[13px] text-ink-200">
        Noch keine Wettkämpfe. Lege zuerst unter „Wettkämpfe" einen an.
      </p>
    );
  }

  return (
    <section className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <p className="max-w-2xl font-mono text-[12px] text-ink-200">
          Laufende Läufe wählen, Fehler mit dem Mannschaftsführer klären (Bestätigen /
          Zurücknehmen) und den Lauf übernehmen. Offene Fehler zählen beim Übernehmen als
          bestätigt.
        </p>
        <div className="flex items-end gap-3">
          {stale && (
            <span className="pill bg-status-armed/15 text-status-armed" title="Letzte Aktualisierung fehlgeschlagen">
              offline?
            </span>
          )}
          <div className="field">
            <label className="label" htmlFor="runs-discipline">
              Wettkampf
            </label>
            <select
              id="runs-discipline"
              className="input"
              value={disciplineId}
              onChange={(e) => {
                setDisciplineId(e.target.value);
                setExpanded(new Set());
              }}
            >
              {[...disciplines]
                .sort((a, b) => a.position - b.position)
                .map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.name}
                    {event.current_discipline_id === d.id ? ' (aktiv)' : ''}
                  </option>
                ))}
            </select>
          </div>
        </div>
      </div>

      {error && (
        <div className="surface flex items-start justify-between gap-3 px-5 py-3 text-[13px] text-status-unknown" role="alert">
          <span>{error}</span>
          <button type="button" className="text-ink-300 hover:text-ink-50" aria-label="Schließen" onClick={() => setError(null)}>
            ✕
          </button>
        </div>
      )}

      {discipline && !isActiveDiscipline && (
        <div className="surface flex flex-wrap items-center justify-between gap-3 px-5 py-3">
          <p className="text-[13px] text-ink-200">
            „{discipline.name}" ist nicht der aktive Wettkampf. Laufende Läufe können nur im aktiven
            Wettkampf gesetzt werden.
          </p>
          <button
            type="button"
            className="btn btn-primary"
            onClick={async () => {
              try {
                const ev = await apiClient.events.setCurrent(event.id, {
                  discipline_id: discipline.id,
                  run_ids: [],
                });
                setLive((cur) => ({ ...cur, event: ev }));
              } catch (e) {
                setError(e instanceof Error ? e.message : 'Aktiv schalten fehlgeschlagen');
              }
            }}
          >
            Aktiv schalten
          </button>
        </div>
      )}

      {discipline && isActiveDiscipline && (
        <CurrentRunsControl
          event={event}
          discipline={discipline}
          runs={runs}
          entries={entries}
          teams={teams}
          onSaved={(ev) => {
            setLive((cur) => ({ ...cur, event: ev }));
            void refreshNow();
          }}
          onError={setError}
        />
      )}

      {currentRuns.length > 0 && (
        <div className={`grid grid-cols-1 gap-4 ${currentRuns.length > 1 ? 'lg:grid-cols-2' : ''}`}>
          {currentRuns.map((r) => {
            const lane = effectiveLane(r, entries, laneCount);
            return cardFor(r, lane != null ? `Bahn ${lane}` : 'Laufend');
          })}
        </div>
      )}

      {discipline && (
        <RunsList
          eventId={event.id}
          disciplineId={discipline.id}
          laneCount={laneCount}
          runs={runs}
          entries={entries}
          teams={teams}
          currentIds={currentIds}
          penaltiesByRun={byRun}
          penaltiesLoaded={penalties != null}
          expanded={expanded}
          onToggle={(id) =>
            setExpanded((cur) => {
              const next = new Set(cur);
              if (next.has(id)) next.delete(id);
              else next.add(id);
              return next;
            })
          }
          renderCard={(r) => cardFor(r)}
          onRunUpdated={updateRun}
          onCreated={() => void refreshNow()}
          onError={setError}
        />
      )}
    </section>
  );
}

function RunsList({
  eventId,
  disciplineId,
  laneCount,
  runs,
  entries,
  teams,
  currentIds,
  penaltiesByRun: byRun,
  penaltiesLoaded,
  expanded,
  onToggle,
  renderCard,
  onRunUpdated,
  onCreated,
  onError,
}: {
  eventId: string;
  disciplineId: string;
  laneCount: number;
  runs: Run[];
  entries: EventDetailData['team_entries'];
  teams: EventDetailData['teams'];
  currentIds: string[];
  penaltiesByRun: Map<string, Penalty[]>;
  penaltiesLoaded: boolean;
  expanded: Set<string>;
  onToggle: (id: string) => void;
  renderCard: (r: Run) => React.ReactNode;
  onRunUpdated: (r: Run) => void;
  onCreated: () => void;
  onError: (msg: string) => void;
}) {
  const [teamId, setTeamId] = useState('');
  const [busy, setBusy] = useState(false);
  const sortedEntries = useMemo(() => [...entries].sort((a, b) => a.start_number - b.start_number), [entries]);
  const teamName = (id: string) => teams.find((t) => t.id === id)?.name ?? 'Unbekannte Mannschaft';

  const createOne = async () => {
    const entry = entries.find((e) => e.team_id === teamId);
    if (!entry) return;
    setBusy(true);
    try {
      await apiClient.runs.create(eventId, disciplineId, createRunBody(teamId, runs, laneCount, entry.lane));
      setTeamId('');
      onCreated();
    } catch (e) {
      onError(e instanceof Error ? e.message : 'Lauf anlegen fehlgeschlagen');
    } finally {
      setBusy(false);
    }
  };

  const createRound = async () => {
    const bodies = roundBodies(entries, runs, laneCount);
    if (bodies.length === 0) return;
    if (!window.confirm(`Durchgang anlegen: ${bodies.length} Läufe (einer je Meldung, nach Startnummer)?`)) return;
    setBusy(true);
    try {
      for (const b of bodies) {
        await apiClient.runs.create(eventId, disciplineId, b);
      }
      onCreated();
    } catch (e) {
      onError(e instanceof Error ? e.message : 'Durchgang anlegen fehlgeschlagen');
      onCreated();
    } finally {
      setBusy(false);
    }
  };

  const setStatus = async (run: Run, status: RunStatus) => {
    try {
      onRunUpdated(await apiClient.runs.update(eventId, disciplineId, run.run_number, run.id, { status }));
    } catch (e) {
      onError(e instanceof Error ? e.message : 'Status ändern fehlgeschlagen');
    }
  };

  return (
    <section className="flex flex-col gap-3">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <span className="label">Alle Läufe ({runs.length})</span>
        <div className="flex flex-wrap items-end gap-2">
          <select
            className="input w-auto"
            aria-label="Mannschaft für neuen Lauf"
            value={teamId}
            onChange={(e) => setTeamId(e.target.value)}
          >
            <option value="">Mannschaft wählen …</option>
            {sortedEntries.map((e) => (
              <option key={e.team_id} value={e.team_id}>
                #{e.start_number} {teamName(e.team_id)} · {wgLabel(e.wg, WG_MISSING_LABEL)}
              </option>
            ))}
          </select>
          <button type="button" className="btn" disabled={busy || !teamId} onClick={createOne}>
            + Lauf
          </button>
          <button type="button" className="btn btn-primary" disabled={busy || entries.length === 0} onClick={createRound}>
            Durchgang anlegen ({entries.length})
          </button>
        </div>
      </div>

      {entries.length === 0 && (
        <p className="surface px-5 py-3 text-[13px] text-ink-200">
          Keine Meldungen für diesen Wettkampf – zuerst unter „Meldungen" Mannschaften melden.
        </p>
      )}

      {runs.length === 0 ? (
        <p className="surface px-5 py-4 text-[13px] text-ink-200">Noch keine Läufe.</p>
      ) : (
        <div className="surface overflow-x-auto">
          <table className="w-full text-[13px]">
            <thead>
              <tr className="border-b border-[color:var(--hairline-color)] text-left">
                <Th>Lauf</Th>
                <Th>Bahn</Th>
                <Th>Mannschaft</Th>
                <Th>WG</Th>
                <Th>Status</Th>
                <Th>Zeit</Th>
                <Th>Fehler</Th>
                <Th>Besetzung</Th>
                <Th className="text-right">Prüfung</Th>
              </tr>
            </thead>
            <tbody>
              {runs.map((r) => {
                const entry = entryForRun(r, entries);
                const pens = byRun.get(r.id) ?? EMPTY;
                const summary = summarizeRun(pens);
                const isCurrent = currentIds.includes(r.id);
                const open = expanded.has(r.id);
                const lane = effectiveLane(r, entries, laneCount);
                return (
                  <FragmentRows key={r.id}>
                    <tr
                      className={[
                        'border-b border-[color:var(--hairline-color)]',
                        isCurrent ? 'bg-status-running/5' : '',
                      ].join(' ')}
                    >
                      <Td className="font-mono">
                        {r.run_number}
                        {isCurrent && (
                          <span className="ml-2 pill bg-status-running/15 text-status-running">läuft</span>
                        )}
                      </Td>
                      <Td className="font-mono">{lane ?? '—'}</Td>
                      <Td>
                        <span className="font-display text-ink-50">{teamName(r.team_id)}</span>
                        <span className="ml-2 font-mono text-[11px] text-ink-200">
                          {entry ? `#${entry.start_number}` : ''}
                        </span>
                      </Td>
                      <Td className="font-mono text-[11px]">{entry ? wgLabel(entry.wg, WG_MISSING_LABEL) : '—'}</Td>
                      <Td>
                        <select
                          className="input w-auto py-1 text-[12px]"
                          aria-label={`Status Lauf ${r.run_number}`}
                          value={r.status}
                          onChange={(e) => void setStatus(r, e.target.value as RunStatus)}
                        >
                          {RUN_STATUSES.map((s) => (
                            <option key={s} value={s}>
                              {RUN_STATUS_LABELS[s]}
                            </option>
                          ))}
                        </select>
                      </Td>
                      <Td>
                        <RunTime run={r} summary={summary} compact />
                      </Td>
                      <Td>{penaltiesLoaded ? <SummaryBadge summary={summary} /> : <span className="label">…</span>}</Td>
                      <Td>
                        {lineupMissing(r) ? (
                          <LineupMissingBadge />
                        ) : (
                          <span className="font-mono text-[11px] text-ink-200">
                            {(r.lineup ?? []).length > 0 ? `${r.lineup.length} Läufer` : '—'}
                          </span>
                        )}
                      </Td>
                      <Td className="text-right">
                        <button type="button" className="btn" onClick={() => onToggle(r.id)} aria-expanded={open}>
                          {open ? 'Schließen' : summary.pending > 0 ? `Prüfen (${summary.pending})` : 'Prüfen'}
                        </button>
                      </Td>
                    </tr>
                    {open && (
                      <tr className="border-b border-[color:var(--hairline-color)]">
                        <td colSpan={9} className="p-3">
                          {renderCard(r)}
                        </td>
                      </tr>
                    )}
                  </FragmentRows>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function FragmentRows({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}

function Th({ children, className }: { children: React.ReactNode; className?: string }) {
  return (
    <th className={['px-3 py-2 font-mono text-[10px] uppercase tracking-[0.18em] text-ink-200', className ?? ''].join(' ')}>
      {children}
    </th>
  );
}

function Td({ children, className }: { children: React.ReactNode; className?: string }) {
  return <td className={['px-3 py-2 align-middle', className ?? ''].join(' ')}>{children}</td>;
}
