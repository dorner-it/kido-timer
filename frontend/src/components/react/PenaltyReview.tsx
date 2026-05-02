// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import {
  ApiError,
  apiClient,
  type CompetitionDetail as CompetitionDetailData,
  type Penalty,
} from '../../lib/api';
import { formatDeltaSeconds } from '../../lib/format';

interface Props {
  id: string;
}

export default function PenaltyReview({ id }: Props) {
  const [data, setData] = useState<CompetitionDetailData | null>(null);
  const [pending, setPending] = useState<Penalty[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<Record<string, boolean>>({});

  useEffect(() => {
    refresh();
    const timer = window.setInterval(refresh, 4000);
    return () => window.clearInterval(timer);
  }, [id]);

  const refresh = async () => {
    try {
      const [detail, pendingList] = await Promise.all([
        apiClient.competitions.detail(id),
        apiClient.penalties.list(id, 'pending'),
      ]);
      setData(detail);
      setPending(pendingList);
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        window.location.href = apiClient.auth.loginUrl();
        return;
      }
      setError(err instanceof Error ? err.message : 'Unbekannter Fehler');
    }
  };

  const decide = async (penalty: Penalty, action: 'approve' | 'reject') => {
    setBusy((b) => ({ ...b, [penalty.id]: true }));
    try {
      if (action === 'approve') {
        await apiClient.penalties.approve(id, penalty.id);
      } else {
        await apiClient.penalties.reject(id, penalty.id);
      }
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Aktion fehlgeschlagen');
    } finally {
      setBusy((b) => ({ ...b, [penalty.id]: false }));
    }
  };

  if (error) {
    return (
      <main className="mx-auto w-full max-w-4xl px-7 py-10">
        <p className="surface px-5 py-3 text-[13px] text-status-unknown">{error}</p>
      </main>
    );
  }

  if (!data || !pending) {
    return (
      <main className="mx-auto w-full max-w-4xl px-7 py-10">
        <p className="label">Lade …</p>
      </main>
    );
  }

  const { competition, teams, runners, runs } = data;

  return (
    <main className="mx-auto w-full max-w-4xl px-7 py-10">
      <a href={`/competitions/${id}`} className="label mb-3 inline-block hover:text-ink-50">
        ← {competition.name}
      </a>

      <div className="mb-8 flex items-end justify-between gap-6">
        <div className="flex flex-col gap-2">
          <span className="label">Prüfung</span>
          <h1 className="font-stencil text-[44px] font-bold leading-none tracking-tight text-ink-50">
            Strafen
          </h1>
        </div>
        <span className="pill bg-status-armed/15 text-status-armed">
          <span className="h-1.5 w-1.5 animate-pulse-ring rounded-full bg-current" />
          {pending.length} ausstehend
        </span>
      </div>

      {pending.length === 0 ? (
        <div className="surface flex flex-col items-center gap-3 py-16 animate-rise">
          <span className="grid h-14 w-14 place-items-center rounded-full bg-status-confirmed/15 text-status-confirmed">
            <svg width="22" height="22" viewBox="0 0 22 22" fill="none">
              <path
                d="M5 11.5L9.5 16L17 7.5"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </span>
          <span className="font-display text-[18px] text-ink-100">Alles geprüft</span>
          <span className="label">Keine ausstehenden Strafen</span>
        </div>
      ) : (
        <ul className="flex flex-col gap-4 stagger-rise">
          {pending.map((p) => {
            const run = runs.find((r) => r.id === p.run_id);
            const team = teams.find((t) => t.id === p.team_id);
            const runner = p.runner_id ? runners.find((r) => r.id === p.runner_id) : null;
            return (
              <li key={p.id}>
                <PenaltyCard
                  penalty={p}
                  team={team?.name ?? '—'}
                  startNumber={team?.start_number ?? null}
                  runner={runner ? `${runner.first_name} ${runner.last_name}` : null}
                  runNumber={run?.run_number ?? null}
                  busy={!!busy[p.id]}
                  onApprove={() => decide(p, 'approve')}
                  onReject={() => decide(p, 'reject')}
                />
              </li>
            );
          })}
        </ul>
      )}
    </main>
  );
}

function PenaltyCard(props: {
  penalty: Penalty;
  team: string;
  startNumber: number | null;
  runner: string | null;
  runNumber: number | null;
  busy: boolean;
  onApprove: () => void;
  onReject: () => void;
}) {
  const { penalty, team, startNumber, runner, runNumber, busy, onApprove, onReject } = props;
  return (
    <article className="surface flex items-stretch overflow-hidden">
      <div className="flex w-32 flex-col items-center justify-center gap-1 bg-signal/5 py-6">
        <span className="label">B{penalty.decider_slot}</span>
        <span className="font-stencil text-[36px] font-bold leading-none text-signal-glow tnum">
          {formatDeltaSeconds(penalty.delta_ms)}
        </span>
      </div>
      <div className="flex flex-1 items-center justify-between gap-4 p-5">
        <div className="flex flex-col gap-1.5">
          <span className="label">
            {runNumber ? `Lauf ${runNumber} · ` : ''}
            {startNumber ? `Start ${startNumber.toString().padStart(2, '0')}` : ''}
          </span>
          <span className="font-display text-[18px] text-ink-50">{team}</span>
          {runner && <span className="font-mono text-[12px] text-ink-200">{runner}</span>}
          {penalty.reason_code && (
            <span className="mt-1 font-mono text-[11px] text-ink-300">{penalty.reason_code}</span>
          )}
        </div>
        <div className="flex flex-col gap-2">
          <button type="button" className="btn btn-confirm" disabled={busy} onClick={onApprove}>
            Genehmigen
          </button>
          <button type="button" className="btn btn-danger" disabled={busy} onClick={onReject}>
            Ablehnen
          </button>
        </div>
      </div>
    </article>
  );
}
