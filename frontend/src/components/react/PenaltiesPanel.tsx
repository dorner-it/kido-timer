// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useMemo, useState } from 'react';

import {
  PENALTY_STATUS_LABELS,
  STATION_KIND_LABELS,
  apiClient,
  type EventDetail as EventDetailData,
  type Penalty,
  type PenaltyStatus,
  type StationKind,
} from '../../lib/api';
import { formatTimeMs } from '../../lib/format';

interface Props {
  data: EventDetailData;
  onChange: () => void;
}

const STATUSES: PenaltyStatus[] = ['pending', 'approved', 'rejected'];
const STATIONS: StationKind[] = [
  'start',
  'verteiler',
  'strahlrohr',
  'knoten',
  'kuebelspritze',
  'podest',
  'strahlrohrlinie',
];

export default function PenaltiesPanel({ data, onChange }: Props) {
  const { event, disciplines, teams, runners, runs } = data;
  const [status, setStatus] = useState<PenaltyStatus | 'all'>('pending');
  const [disciplineFilter, setDisciplineFilter] = useState<string>('all');
  const [stationFilter, setStationFilter] = useState<StationKind | 'all'>('all');
  const [list, setList] = useState<Penalty[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [bulkBusy, setBulkBusy] = useState(false);

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, disciplineFilter, stationFilter]);

  const refresh = async () => {
    try {
      const filter: { status?: PenaltyStatus; discipline_id?: string } = {};
      if (status !== 'all') filter.status = status;
      if (disciplineFilter !== 'all') filter.discipline_id = disciplineFilter;
      let result = await apiClient.penalties.list(event.id, filter);
      result.sort((a, b) => (a.created_at < b.created_at ? 1 : -1));
      if (stationFilter !== 'all') {
        result = result.filter((p) => p.station === stationFilter);
      }
      setList(result);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Konnte Strafen nicht laden');
    }
  };

  const teamById = useMemo(
    () => new Map(teams.map((t) => [t.id, t])),
    [teams],
  );
  const runnerById = useMemo(
    () => new Map(runners.map((r) => [r.id, r])),
    [runners],
  );
  const disciplineById = useMemo(
    () => new Map(disciplines.map((d) => [d.id, d])),
    [disciplines],
  );
  const runById = useMemo(() => new Map(runs.map((r) => [r.id, r])), [runs]);

  const approve = async (id: string) => {
    setBusyId(id);
    try {
      await apiClient.penalties.approve(event.id, id);
      await refresh();
      onChange();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Genehmigen fehlgeschlagen');
    } finally {
      setBusyId(null);
    }
  };

  const reject = async (id: string) => {
    setBusyId(id);
    try {
      await apiClient.penalties.reject(event.id, id);
      await refresh();
      onChange();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Ablehnen fehlgeschlagen');
    } finally {
      setBusyId(null);
    }
  };

  // "Lauf abschließen" — bulk-approve every pending penalty for one
  // run. Groups the current list by run_id (after every filter the
  // operator already chose) and presents one button per run with
  // pending rows.
  const pendingByRun = useMemo(() => {
    if (!list) return new Map<string, Penalty[]>();
    const out = new Map<string, Penalty[]>();
    for (const p of list) {
      if (p.status !== 'pending') continue;
      const arr = out.get(p.run_id) ?? [];
      arr.push(p);
      out.set(p.run_id, arr);
    }
    return out;
  }, [list]);

  const bulkApprove = async (runId: string) => {
    const pendings = pendingByRun.get(runId) ?? [];
    if (pendings.length === 0) return;
    const run = runById.get(runId);
    const team = run ? teamById.get(run.team_id) : null;
    const lines = pendings
      .map((p) => {
        const ds = p.station ? STATION_KIND_LABELS[p.station] : 'ohne Station';
        const delta =
          p.is_disqualification
            ? 'DQ'
            : `${p.delta_ms >= 0 ? '+' : ''}${formatTimeMs(Math.abs(p.delta_ms))}`;
        return `• ${ds}: ${delta}${p.reason_code ? ` (${p.reason_code})` : ''}`;
      })
      .join('\n');
    const header = run
      ? `Lauf ${run.run_number} – ${team?.name ?? '?'}`
      : `Lauf ${runId.slice(0, 6)}`;
    const ok = window.confirm(
      `Lauf abschließen?\n\n${header}\n\nFolgende ${pendings.length} ausstehende Strafe(n) werden genehmigt:\n${lines}`,
    );
    if (!ok) return;
    setBulkBusy(true);
    try {
      const res = await apiClient.penalties.bulkApprove(event.id, runId);
      await refresh();
      onChange();
      setError(null);
      // Soft feedback via the existing error slot so the operator
      // sees what happened without a toast system.
      // eslint-disable-next-line no-alert
      window.alert(
        `Genehmigt: ${res.approved}, davon ${res.dq} Disqualifikation(en).`,
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Lauf-Abschluss fehlgeschlagen');
    } finally {
      setBulkBusy(false);
    }
  };

  return (
    <section className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <p className="font-mono text-[12px] text-ink-200">
          Strafrichter-Einreichungen. Genehmigte Strafen fließen automatisch in
          die Korrekturzeit der zugehörigen Läufe ein.
        </p>
        <div className="flex gap-2">
          <div className="field">
            <label className="label" htmlFor="penalty-status">
              Status
            </label>
            <select
              id="penalty-status"
              className="input"
              value={status}
              onChange={(e) => setStatus(e.target.value as PenaltyStatus | 'all')}
            >
              <option value="all">Alle</option>
              {STATUSES.map((s) => (
                <option key={s} value={s}>
                  {PENALTY_STATUS_LABELS[s]}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label className="label" htmlFor="penalty-discipline">
              Wettkampf
            </label>
            <select
              id="penalty-discipline"
              className="input"
              value={disciplineFilter}
              onChange={(e) => setDisciplineFilter(e.target.value)}
            >
              <option value="all">Alle Wettkämpfe</option>
              {disciplines.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label className="label" htmlFor="penalty-station">
              Station
            </label>
            <select
              id="penalty-station"
              className="input"
              value={stationFilter}
              onChange={(e) =>
                setStationFilter(e.target.value as StationKind | 'all')
              }
            >
              <option value="all">Alle Stationen</option>
              {STATIONS.map((s) => (
                <option key={s} value={s}>
                  {STATION_KIND_LABELS[s]}
                </option>
              ))}
            </select>
          </div>
        </div>
      </div>

      {pendingByRun.size > 0 && (
        <div className="surface flex flex-wrap items-center justify-between gap-3 px-5 py-3">
          <p className="font-mono text-[12px] text-ink-200">
            {pendingByRun.size === 1
              ? '1 Lauf mit ausstehenden Strafen.'
              : `${pendingByRun.size} Läufe mit ausstehenden Strafen.`}{' '}
            Mit „Lauf abschließen" werden alle pendings eines Laufs auf einmal
            genehmigt.
          </p>
          <div className="flex flex-wrap gap-2">
            {Array.from(pendingByRun.entries()).map(([runId, pendings]) => {
              const run = runById.get(runId);
              const team = run ? teamById.get(run.team_id) : null;
              return (
                <button
                  key={runId}
                  type="button"
                  className="btn btn-primary"
                  disabled={bulkBusy}
                  onClick={() => bulkApprove(runId)}
                >
                  Lauf {run?.run_number ?? '?'} – {team?.name ?? '?'}{' '}
                  abschließen ({pendings.length})
                </button>
              );
            })}
          </div>
        </div>
      )}

      {error && (
        <p className="surface px-5 py-3 text-[13px] text-status-unknown">{error}</p>
      )}

      {list == null ? (
        <p className="label">Lade …</p>
      ) : list.length === 0 ? (
        <p className="surface px-5 py-4 text-[13px] text-ink-200">
          Keine Strafen mit diesem Filter.
        </p>
      ) : (
        <div className="surface overflow-hidden">
          <table className="w-full text-[13px]">
            <thead>
              <tr className="border-b border-[color:var(--hairline-color)] text-left">
                <Th>Zeit</Th>
                <Th>Wettkampf</Th>
                <Th>Mannschaft / Läufer</Th>
                <Th>Station</Th>
                <Th>Slot</Th>
                <Th className="text-right">Δ</Th>
                <Th>Grund</Th>
                <Th>Status</Th>
                <Th className="text-right">Aktion</Th>
              </tr>
            </thead>
            <tbody>
              {list.map((p) => {
                const team = teamById.get(p.team_id);
                const runner = p.runner_id ? runnerById.get(p.runner_id) : null;
                const disc = disciplineById.get(p.discipline_id);
                return (
                  <tr
                    key={p.id}
                    className="border-b border-[color:var(--hairline-color)] last:border-0"
                  >
                    <Td className="font-mono text-[11px] text-ink-200">
                      {formatDateTime(p.created_at)}
                    </Td>
                    <Td>{disc?.name ?? '?'}</Td>
                    <Td>
                      <span className="font-display text-ink-50">
                        {team?.name ?? '?'}
                      </span>
                      {runner && (
                        <span className="ml-2 font-mono text-[11px] text-ink-200">
                          {runner.first_name} {runner.last_name}
                        </span>
                      )}
                    </Td>
                    <Td className="font-mono text-[11px]">
                      {p.station ? STATION_KIND_LABELS[p.station] : '—'}
                    </Td>
                    <Td className="font-mono">
                      {p.decider_slot > 0 ? `B${p.decider_slot}` : '—'}
                    </Td>
                    <Td className="text-right font-mono tnum text-signal-glow">
                      {p.is_disqualification ? (
                        <span className="text-status-unknown">DQ</span>
                      ) : (
                        <>
                          {p.delta_ms >= 0 ? '+' : ''}
                          {formatTimeMs(Math.abs(p.delta_ms))}
                        </>
                      )}
                    </Td>
                    <Td className="font-mono text-[11px] text-ink-200">
                      {p.reason_code ?? '—'}
                    </Td>
                    <Td>
                      <StatusPill status={p.status} />
                    </Td>
                    <Td className="text-right">
                      {p.status === 'pending' ? (
                        <div className="inline-flex gap-1">
                          <button
                            type="button"
                            className="btn btn-confirm"
                            disabled={busyId === p.id}
                            onClick={() => approve(p.id)}
                          >
                            ✓
                          </button>
                          <button
                            type="button"
                            className="btn btn-danger"
                            disabled={busyId === p.id}
                            onClick={() => reject(p.id)}
                          >
                            ✕
                          </button>
                        </div>
                      ) : (
                        <span className="font-mono text-[11px] text-ink-300">
                          {p.reviewed_at ? formatDateTime(p.reviewed_at) : '—'}
                        </span>
                      )}
                    </Td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

function Th({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <th
      className={[
        'px-3 py-2 font-mono text-[10px] uppercase tracking-[0.18em] text-ink-200',
        className ?? '',
      ].join(' ')}
    >
      {children}
    </th>
  );
}

function Td({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return <td className={['px-3 py-2 align-top', className ?? ''].join(' ')}>{children}</td>;
}

function StatusPill({ status }: { status: PenaltyStatus }) {
  const cls =
    status === 'approved'
      ? 'bg-status-confirmed/15 text-status-confirmed'
      : status === 'rejected'
        ? 'bg-status-unknown/15 text-status-unknown'
        : 'bg-status-armed/15 text-status-armed';
  return (
    <span className={`pill ${cls}`}>
      <span className="h-1.5 w-1.5 rounded-full bg-current" />
      {PENALTY_STATUS_LABELS[status]}
    </span>
  );
}

function formatDateTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString('de-DE', {
      day: '2-digit',
      month: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  } catch {
    return iso;
  }
}
