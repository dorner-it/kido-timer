// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useMemo, useState } from 'react';

import {
  PENALTY_STATUS_LABELS,
  apiClient,
  type EventDetail as EventDetailData,
  type Penalty,
  type PenaltyStatus,
} from '../../lib/api';
import { formatTimeMs } from '../../lib/format';

interface Props {
  data: EventDetailData;
  onChange: () => void;
}

const STATUSES: PenaltyStatus[] = ['pending', 'approved', 'rejected'];

export default function PenaltiesPanel({ data, onChange }: Props) {
  const { event, disciplines, teams, runners } = data;
  const [status, setStatus] = useState<PenaltyStatus | 'all'>('pending');
  const [disciplineFilter, setDisciplineFilter] = useState<string>('all');
  const [list, setList] = useState<Penalty[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, disciplineFilter]);

  const refresh = async () => {
    try {
      const filter: { status?: PenaltyStatus; discipline_id?: string } = {};
      if (status !== 'all') filter.status = status;
      if (disciplineFilter !== 'all') filter.discipline_id = disciplineFilter;
      const result = await apiClient.penalties.list(event.id, filter);
      result.sort((a, b) => (a.created_at < b.created_at ? 1 : -1));
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
        </div>
      </div>

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
                    <Td className="font-mono">B{p.decider_slot}</Td>
                    <Td className="text-right font-mono tnum text-signal-glow">
                      {p.delta_ms >= 0 ? '+' : ''}
                      {formatTimeMs(Math.abs(p.delta_ms))}
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
