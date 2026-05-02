// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import {
  ApiError,
  TIMER_MODE_LABELS,
  apiClient,
  type Competition,
  type CompetitionDetail as CompetitionDetailData,
  type Run,
  type Runner,
  type Team,
} from '../../lib/api';
import { formatDateDE } from '../../lib/format';

interface Props {
  id: string;
}

export default function CompetitionDetail({ id }: Props) {
  const [data, setData] = useState<CompetitionDetailData | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    refresh();
  }, [id]);

  const refresh = async () => {
    try {
      setData(await apiClient.competitions.detail(id));
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        window.location.href = "/login";
        return;
      }
      setError(err instanceof Error ? err.message : 'Unbekannter Fehler');
    }
  };

  const activate = async () => {
    setError(null);
    try {
      await apiClient.competitions.activate(id);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Aktivierung fehlgeschlagen');
    }
  };

  const rotate = async () => {
    if (!confirm('Decider-Codes wirklich neu generieren? Bisherige QR-Codes werden ungültig.')) {
      return;
    }
    try {
      await apiClient.competitions.rotateDeciderTokens(id);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Rotation fehlgeschlagen');
    }
  };

  const remove = async () => {
    if (!confirm('Wettkampf endgültig löschen? Alle Mannschaften, Läufer, Läufe und Strafen werden mit gelöscht.')) {
      return;
    }
    try {
      await apiClient.competitions.remove(id);
      window.location.href = '/competitions';
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Löschen fehlgeschlagen');
    }
  };

  if (error) {
    return (
      <main className="mx-auto w-full max-w-6xl px-7 py-10">
        <p className="surface px-5 py-3 text-[13px] text-status-unknown">{error}</p>
      </main>
    );
  }

  if (!data) {
    return (
      <main className="mx-auto w-full max-w-6xl px-7 py-10">
        <p className="label">Lade …</p>
      </main>
    );
  }

  const { competition, teams, runners, runs } = data;

  return (
    <main className="mx-auto w-full max-w-6xl px-7 py-10">
      <a href="/competitions" className="label mb-3 inline-block hover:text-ink-50">
        ← Übersicht
      </a>

      <div className="mb-8 flex flex-wrap items-end justify-between gap-6">
        <div className="flex flex-col gap-2">
          <span className="label">{TIMER_MODE_LABELS[competition.mode]}</span>
          <h1 className="font-stencil text-[44px] font-bold leading-none tracking-tight text-ink-50">
            {competition.name}
          </h1>
          <span className="font-mono tnum text-[13px] text-ink-200">
            {formatDateDE(competition.date)}
            {competition.location ? ` · ${competition.location}` : ''}
          </span>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          {competition.is_active ? (
            <span className="pill bg-status-running/15 text-status-running">
              <span className="h-1.5 w-1.5 rounded-full bg-current" />
              Aktiv
            </span>
          ) : (
            <button type="button" onClick={activate} className="btn btn-primary">
              Aktivieren
            </button>
          )}
          <a href={`/competitions/${id}/penalties`} className="btn">
            Strafen prüfen
          </a>
          <button type="button" onClick={remove} className="btn btn-danger">
            Löschen
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-[2fr_1fr]">
        <TeamsPanel
          competitionId={id}
          teams={teams}
          runners={runners}
          onChange={refresh}
        />
        <SidePanel
          competition={competition}
          runs={runs}
          teams={teams}
          runners={runners}
          onChange={refresh}
          onRotate={rotate}
        />
      </div>
    </main>
  );
}

// ===== Teams panel =====

function TeamsPanel({
  competitionId,
  teams,
  runners,
  onChange,
}: {
  competitionId: string;
  teams: Team[];
  runners: Runner[];
  onChange: () => Promise<void>;
}) {
  const [adding, setAdding] = useState(false);

  return (
    <section className="flex flex-col gap-5">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-[18px] font-medium text-ink-50">Mannschaften</h2>
        <button type="button" className="btn" onClick={() => setAdding((v) => !v)}>
          {adding ? 'Schließen' : '+ Mannschaft'}
        </button>
      </div>

      {adding && (
        <NewTeamForm
          competitionId={competitionId}
          existingNumbers={teams.map((t) => t.start_number)}
          onCreated={async () => {
            setAdding(false);
            await onChange();
          }}
        />
      )}

      {teams.length === 0 ? (
        <p className="surface px-5 py-6 text-center text-[14px] text-ink-200">
          Noch keine Mannschaften angelegt.
        </p>
      ) : (
        <ul className="flex flex-col gap-4">
          {[...teams]
            .sort((a, b) => a.start_number - b.start_number)
            .map((team) => (
              <TeamCard
                key={team.id}
                competitionId={competitionId}
                team={team}
                runners={runners.filter((r) => r.team_id === team.id)}
                onChange={onChange}
              />
            ))}
        </ul>
      )}
    </section>
  );
}

function NewTeamForm({
  competitionId,
  existingNumbers,
  onCreated,
}: {
  competitionId: string;
  existingNumbers: number[];
  onCreated: () => Promise<void>;
}) {
  const nextNumber = (existingNumbers.reduce((a, b) => Math.max(a, b), 0) || 0) + 1;
  const [name, setName] = useState('');
  const [startNumber, setStartNumber] = useState(nextNumber);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      await apiClient.teams.create(competitionId, { name: name.trim(), start_number: startNumber });
      setName('');
      await onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Anlegen fehlgeschlagen');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form onSubmit={submit} className="surface flex flex-wrap gap-3 p-4 animate-rise">
      <div className="field flex-1 min-w-[180px]">
        <span className="label">Name</span>
        <input
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoFocus
        />
      </div>
      <div className="field w-28">
        <span className="label">Start-Nr.</span>
        <input
          type="number"
          min="1"
          className="input"
          value={startNumber}
          onChange={(e) => setStartNumber(Number(e.target.value))}
        />
      </div>
      <button type="submit" className="btn btn-primary self-end" disabled={submitting}>
        Speichern
      </button>
      {error && (
        <p className="w-full text-[11px] text-status-unknown" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

function TeamCard({
  competitionId,
  team,
  runners,
  onChange,
}: {
  competitionId: string;
  team: Team;
  runners: Runner[];
  onChange: () => Promise<void>;
}) {
  const [addingRunner, setAddingRunner] = useState(false);

  const removeTeam = async () => {
    if (!confirm(`Mannschaft "${team.name}" und alle Läufer löschen?`)) return;
    await apiClient.teams.remove(competitionId, team.id);
    await onChange();
  };

  return (
    <li className="surface p-5">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-4">
          <span className="font-stencil text-[28px] font-bold leading-none text-signal-glow tnum">
            {team.start_number.toString().padStart(2, '0')}
          </span>
          <div className="flex flex-col gap-1">
            <span className="font-display text-[18px] font-medium text-ink-50">{team.name}</span>
            <span className="label">{runners.length} Läufer</span>
          </div>
        </div>
        <div className="flex gap-2">
          <button
            type="button"
            className="btn"
            onClick={() => setAddingRunner((v) => !v)}
          >
            {addingRunner ? '×' : '+ Läufer'}
          </button>
          <button type="button" className="btn btn-danger" onClick={removeTeam}>
            Löschen
          </button>
        </div>
      </div>

      {(runners.length > 0 || addingRunner) && (
        <div className="mt-4 flex flex-col divide-y divide-[color:var(--hairline-color)]">
          {[...runners]
            .sort((a, b) => a.position - b.position)
            .map((r) => (
              <RunnerRow
                key={r.id}
                competitionId={competitionId}
                runner={r}
                onChange={onChange}
              />
            ))}
          {addingRunner && (
            <NewRunnerForm
              competitionId={competitionId}
              teamId={team.id}
              nextPosition={(runners.reduce((a, b) => Math.max(a, b.position), 0) || 0) + 1}
              onCreated={async () => {
                setAddingRunner(false);
                await onChange();
              }}
            />
          )}
        </div>
      )}
    </li>
  );
}

function RunnerRow({
  competitionId,
  runner,
  onChange,
}: {
  competitionId: string;
  runner: Runner;
  onChange: () => Promise<void>;
}) {
  const remove = async () => {
    if (!confirm(`${runner.first_name} ${runner.last_name} entfernen?`)) return;
    await apiClient.runners.remove(competitionId, runner.team_id, runner.id);
    await onChange();
  };

  return (
    <div className="flex items-center justify-between gap-3 py-3">
      <div className="flex items-center gap-4">
        <span className="font-mono tnum text-[12px] text-ink-300 w-6 text-right">
          {runner.position.toString().padStart(2, '0')}
        </span>
        <span className="font-display text-[15px] text-ink-50">
          {runner.first_name} {runner.last_name}
        </span>
        <span className="label">
          *{runner.birth_date.slice(0, 4)}
        </span>
      </div>
      <button type="button" className="btn btn-danger" onClick={remove}>
        ×
      </button>
    </div>
  );
}

function NewRunnerForm({
  competitionId,
  teamId,
  nextPosition,
  onCreated,
}: {
  competitionId: string;
  teamId: string;
  nextPosition: number;
  onCreated: () => Promise<void>;
}) {
  const [first, setFirst] = useState('');
  const [last, setLast] = useState('');
  const [birth, setBirth] = useState('');
  const [position, setPosition] = useState(nextPosition);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!first.trim() || !last.trim() || !birth) return;
    await apiClient.runners.create(competitionId, teamId, {
      first_name: first.trim(),
      last_name: last.trim(),
      birth_date: birth,
      position,
    });
    setFirst('');
    setLast('');
    setBirth('');
    await onCreated();
  };

  return (
    <form onSubmit={submit} className="flex flex-wrap gap-2 py-3 animate-rise">
      <input
        className="input flex-1 min-w-[120px]"
        placeholder="Vorname"
        value={first}
        onChange={(e) => setFirst(e.target.value)}
        autoFocus
      />
      <input
        className="input flex-1 min-w-[120px]"
        placeholder="Nachname"
        value={last}
        onChange={(e) => setLast(e.target.value)}
      />
      <input
        type="date"
        className="input w-40"
        value={birth}
        onChange={(e) => setBirth(e.target.value)}
      />
      <input
        type="number"
        min="1"
        className="input w-20"
        value={position}
        onChange={(e) => setPosition(Number(e.target.value))}
      />
      <button type="submit" className="btn btn-primary">
        +
      </button>
    </form>
  );
}

// ===== Side panel: current run + runs + decider tokens =====

function SidePanel({
  competition,
  runs,
  teams,
  runners,
  onChange,
  onRotate,
}: {
  competition: Competition;
  runs: Run[];
  teams: Team[];
  runners: Runner[];
  onChange: () => Promise<void>;
  onRotate: () => Promise<void>;
}) {
  return (
    <aside className="flex flex-col gap-6">
      <CurrentRunCard
        competition={competition}
        runs={runs}
        teams={teams}
        runners={runners}
        onChange={onChange}
      />
      <RunsList
        competitionId={competition.id}
        runs={runs}
        teams={teams}
        onChange={onChange}
      />
      <DeciderCodesCard competition={competition} onRotate={onRotate} />
    </aside>
  );
}

function CurrentRunCard({
  competition,
  runs,
  teams,
  runners,
  onChange,
}: {
  competition: Competition;
  runs: Run[];
  teams: Team[];
  runners: Runner[];
  onChange: () => Promise<void>;
}) {
  const current = runs.find((r) => r.id === competition.current_run_id) ?? null;
  const team = current ? teams.find((t) => t.id === current.team_id) : null;
  const runner =
    current?.runner_id ? runners.find((r) => r.id === current.runner_id) ?? null : null;

  const setRun = async (id: string | null) => {
    await apiClient.competitions.setCurrentRun(competition.id, id);
    await onChange();
  };

  return (
    <div className="surface p-5">
      <span className="label">Aktueller Lauf</span>
      {current ? (
        <div className="mt-3 flex flex-col gap-3">
          <div className="flex items-center gap-3">
            {team && (
              <span className="font-stencil text-[28px] font-bold leading-none text-signal-glow tnum">
                {team.start_number.toString().padStart(2, '0')}
              </span>
            )}
            <div className="flex flex-col">
              <span className="font-display text-[18px] text-ink-50">
                {team?.name ?? '—'}
              </span>
              {runner && (
                <span className="font-mono text-[12px] text-ink-200">
                  {runner.first_name} {runner.last_name}
                </span>
              )}
              <span className="label">Lauf {current.run_number}</span>
            </div>
          </div>
          <button type="button" className="btn" onClick={() => setRun(null)}>
            Lauf beenden
          </button>
        </div>
      ) : (
        <p className="mt-3 font-mono text-[12px] text-ink-200">Kein Lauf aktiv.</p>
      )}
    </div>
  );
}

function RunsList({
  competitionId,
  runs,
  teams,
  onChange,
}: {
  competitionId: string;
  runs: Run[];
  teams: Team[];
  onChange: () => Promise<void>;
}) {
  const [adding, setAdding] = useState(false);
  const [teamId, setTeamId] = useState(teams[0]?.id ?? '');
  const nextNumber = (runs.reduce((a, b) => Math.max(a, b.run_number), 0) || 0) + 1;

  const create = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!teamId) return;
    await apiClient.runs.create(competitionId, {
      team_id: teamId,
      runner_id: null,
      run_number: nextNumber,
      lane: null,
    });
    setAdding(false);
    await onChange();
  };

  const start = async (run: Run) => {
    await apiClient.competitions.setCurrentRun(competitionId, run.id);
    await apiClient.runs.update(competitionId, run.run_number, run.id, {
      status: 'active',
      started_at: new Date().toISOString(),
    });
    await onChange();
  };

  return (
    <div className="surface p-5">
      <div className="flex items-center justify-between">
        <span className="label">Läufe</span>
        <button type="button" className="btn" onClick={() => setAdding((v) => !v)} disabled={teams.length === 0}>
          {adding ? '×' : '+ Lauf'}
        </button>
      </div>

      {adding && (
        <form onSubmit={create} className="mt-3 flex gap-2 animate-rise">
          <select
            className="input flex-1"
            value={teamId}
            onChange={(e) => setTeamId(e.target.value)}
          >
            {teams.map((t) => (
              <option key={t.id} value={t.id}>
                {t.start_number.toString().padStart(2, '0')} · {t.name}
              </option>
            ))}
          </select>
          <button type="submit" className="btn btn-primary">+</button>
        </form>
      )}

      {runs.length === 0 ? (
        <p className="mt-3 font-mono text-[12px] text-ink-200">Noch keine Läufe.</p>
      ) : (
        <ul className="mt-3 flex flex-col divide-y divide-[color:var(--hairline-color)]">
          {[...runs]
            .sort((a, b) => a.run_number - b.run_number)
            .map((r) => {
              const team = teams.find((t) => t.id === r.team_id);
              return (
                <li key={r.id} className="flex items-center justify-between gap-3 py-2">
                  <div className="flex items-center gap-3">
                    <span className="font-mono tnum text-[12px] text-ink-300 w-6 text-right">
                      #{r.run_number}
                    </span>
                    <span className="font-display text-[14px] text-ink-50">
                      {team?.name ?? '?'}
                    </span>
                    <RunStatusPill status={r.status} />
                  </div>
                  {r.status === 'pending' && (
                    <button type="button" className="btn btn-primary" onClick={() => start(r)}>
                      Starten
                    </button>
                  )}
                </li>
              );
            })}
        </ul>
      )}
    </div>
  );
}

function RunStatusPill({ status }: { status: Run['status'] }) {
  const map: Record<Run['status'], { color: string; label: string }> = {
    pending: { color: 'bg-status-inactive/15 text-status-inactive', label: 'Vorgemerkt' },
    active: { color: 'bg-status-running/15 text-status-running', label: 'Aktiv' },
    completed: { color: 'bg-status-confirmed/15 text-status-confirmed', label: 'Beendet' },
    cancelled: { color: 'bg-status-unknown/15 text-status-unknown', label: 'Abbruch' },
  };
  const s = map[status];
  return <span className={`pill ${s.color}`}>{s.label}</span>;
}

function DeciderCodesCard({
  competition,
  onRotate,
}: {
  competition: Competition;
  onRotate: () => Promise<void>;
}) {
  const baseUrl = typeof window === 'undefined' ? '' : window.location.origin;
  return (
    <div className="surface p-5">
      <div className="flex items-center justify-between">
        <span className="label">Strafrichter (Decider)</span>
        <button type="button" className="btn" onClick={onRotate}>
          Neu generieren
        </button>
      </div>
      <p className="mt-2 font-mono text-[11px] text-ink-200">
        Vier QR-Codes, einer pro Slot. Nur gültig solange dieser Wettkampf aktiv ist.
      </p>
      <ul className="mt-3 flex flex-col gap-2">
        {competition.decider_tokens.map((token, idx) => {
          const slot = idx + 1;
          const url = `${baseUrl}/decider/${competition.owner_sub}/${slot}/${token}`;
          return (
            <li
              key={idx}
              className="flex items-center justify-between gap-3 rounded-lg border border-[color:var(--hairline-color)] px-3 py-2"
            >
              <span className="font-stencil text-[16px] font-bold text-signal-glow">
                B{slot}
              </span>
              <span className="font-mono text-[10px] text-ink-200 truncate flex-1">{url}</span>
              <button
                type="button"
                className="btn"
                onClick={() => navigator.clipboard.writeText(url)}
              >
                Kopieren
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
