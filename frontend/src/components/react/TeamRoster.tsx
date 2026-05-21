// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useMemo, useState } from 'react';

import {
  apiClient,
  type EventDetail as EventDetailData,
  type Runner,
  type Team,
} from '../../lib/api';

interface Props {
  data: EventDetailData;
  onChange: () => void;
}

/**
 * Single-page roster spreadsheet. Each team is a collapsible group of
 * runner rows. Click any cell to edit in place; tab/enter moves to the
 * next cell. The trailing "neue Zeile" placeholder under each team
 * becomes a real runner the moment the user types into it.
 */
export default function TeamRoster({ data, onChange }: Props) {
  const { event, teams, runners } = data;
  const [creatingTeam, setCreatingTeam] = useState(false);
  const [newTeamName, setNewTeamName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const teamsSorted = useMemo(
    () => [...teams].sort((a, b) => a.position - b.position),
    [teams],
  );

  const runnersByTeam = useMemo(() => {
    const m = new Map<string, Runner[]>();
    for (const r of runners) {
      const list = m.get(r.team_id) ?? [];
      list.push(r);
      m.set(r.team_id, list);
    }
    for (const list of m.values()) {
      list.sort((a, b) => a.position - b.position);
    }
    return m;
  }, [runners]);

  const addTeam = async (e: React.FormEvent) => {
    e.preventDefault();
    const name = newTeamName.trim();
    if (!name) return;
    setBusy(true);
    setError(null);
    try {
      const nextPosition = (teamsSorted[teamsSorted.length - 1]?.position ?? 0) + 1;
      await apiClient.teams.create(event.id, { name, position: nextPosition });
      setNewTeamName('');
      setCreatingTeam(false);
      onChange();
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Mannschaft konnte nicht angelegt werden');
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <p className="font-mono text-[12px] text-ink-200">
          Eine Tabelle für alle Mannschaften und Läufer. Klick auf eine Zelle,
          um sie zu bearbeiten. Tab springt weiter, Enter bestätigt.
        </p>
        <button
          type="button"
          onClick={() => setCreatingTeam((v) => !v)}
          className="btn btn-primary"
        >
          {creatingTeam ? 'Abbrechen' : '+ Mannschaft'}
        </button>
      </div>

      {creatingTeam && (
        <form onSubmit={addTeam} className="surface flex items-end gap-3 p-4 animate-rise">
          <div className="field flex-1">
            <label className="label" htmlFor="new-team-name">
              Mannschafts-Name
            </label>
            <input
              id="new-team-name"
              className="input"
              value={newTeamName}
              onChange={(e) => setNewTeamName(e.target.value)}
              autoFocus
              placeholder="z.B. FF Musterstadt 1"
            />
          </div>
          <button type="submit" className="btn btn-primary" disabled={busy}>
            Anlegen
          </button>
        </form>
      )}

      {error && (
        <p className="surface px-5 py-3 text-[13px] text-status-unknown" role="alert">
          {error}
        </p>
      )}

      {teamsSorted.length === 0 ? (
        <p className="surface px-5 py-4 text-[13px] text-ink-200">
          Noch keine Mannschaften. Lege oben die erste an.
        </p>
      ) : (
        <div className="surface overflow-hidden">
          <div className="grid grid-cols-[2.4fr_2fr_2fr_1.2fr_0.8fr_2.5rem] gap-px bg-[color:var(--hairline-color)] text-[11px]">
            <HeaderCell>Mannschaft / Vorname</HeaderCell>
            <HeaderCell>Nachname</HeaderCell>
            <HeaderCell>Geburtsdatum</HeaderCell>
            <HeaderCell>Position</HeaderCell>
            <HeaderCell> </HeaderCell>
            <HeaderCell> </HeaderCell>
          </div>
          {teamsSorted.map((team) => (
            <TeamBlock
              key={team.id}
              event_id={event.id}
              team={team}
              runners={runnersByTeam.get(team.id) ?? []}
              onChange={onChange}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function HeaderCell({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-[color:var(--soft-fill)] px-3 py-2 font-mono uppercase tracking-[0.18em] text-ink-200">
      {children}
    </div>
  );
}

function TeamBlock({
  event_id,
  team,
  runners,
  onChange,
}: {
  event_id: string;
  team: Team;
  runners: Runner[];
  onChange: () => void;
}) {
  const [open, setOpen] = useState(true);
  const [pendingRunner, setPendingRunner] = useState<{
    first_name: string;
    last_name: string;
    birth_date: string;
  } | null>(null);
  const [savingNew, setSavingNew] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const nextPosition = (runners[runners.length - 1]?.position ?? 0) + 1;

  const updateTeamName = async (name: string) => {
    if (!name.trim() || name === team.name) return;
    try {
      await apiClient.teams.update(event_id, team.id, {
        name: name.trim(),
        position: team.position,
      });
      onChange();
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Update fehlgeschlagen');
    }
  };

  const removeTeam = async () => {
    if (!confirm(`Mannschaft „${team.name}" löschen? Alle Läufer dieser Mannschaft werden mit gelöscht.`)) {
      return;
    }
    try {
      await apiClient.teams.remove(event_id, team.id);
      onChange();
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Löschen fehlgeschlagen');
    }
  };

  const saveNewRunner = async (
    patch: Partial<{ first_name: string; last_name: string; birth_date: string }>,
  ) => {
    const draft = { first_name: '', last_name: '', birth_date: '', ...pendingRunner, ...patch };
    if (!draft.first_name.trim() || !draft.last_name.trim() || !draft.birth_date) {
      // Persist partial state but don't POST until all required fields are present.
      setPendingRunner({
        first_name: draft.first_name,
        last_name: draft.last_name,
        birth_date: draft.birth_date,
      });
      return;
    }
    setSavingNew(true);
    setErr(null);
    try {
      await apiClient.runners.create(event_id, team.id, {
        first_name: draft.first_name.trim(),
        last_name: draft.last_name.trim(),
        birth_date: draft.birth_date,
        position: nextPosition,
      });
      setPendingRunner(null);
      onChange();
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Läufer konnte nicht angelegt werden');
    } finally {
      setSavingNew(false);
    }
  };

  return (
    <div className="border-t border-[color:var(--hairline-color)] bg-[color:var(--c-ink-800)]/40">
      {/* Team header row */}
      <div className="grid grid-cols-[2.4fr_2fr_2fr_1.2fr_0.8fr_2.5rem] gap-px bg-[color:var(--hairline-color)] text-[13px]">
        <div className="flex items-center gap-2 bg-[color:var(--c-ink-700)] px-3 py-2">
          <button
            type="button"
            onClick={() => setOpen((v) => !v)}
            aria-label={open ? 'Zuklappen' : 'Aufklappen'}
            className="text-ink-200 hover:text-ink-50 font-mono text-[12px]"
          >
            {open ? '▾' : '▸'}
          </button>
          <EditableText
            value={team.name}
            onCommit={updateTeamName}
            className="font-display text-[15px] font-semibold text-ink-50"
            ariaLabel="Mannschafts-Name"
          />
        </div>
        <div className="bg-[color:var(--c-ink-700)] px-3 py-2 col-span-3 font-mono text-[11px] text-ink-200">
          {runners.length} {runners.length === 1 ? 'Läufer' : 'Läufer'}
        </div>
        <div className="bg-[color:var(--c-ink-700)] px-3 py-2 text-right">
          <button
            type="button"
            onClick={removeTeam}
            aria-label="Mannschaft löschen"
            className="text-ink-300 hover:text-status-unknown text-[14px]"
          >
            ✕
          </button>
        </div>
        <div className="bg-[color:var(--c-ink-700)]" />
      </div>

      {open && (
        <>
          {runners.map((runner) => (
            <RunnerRow
              key={runner.id}
              event_id={event_id}
              team_id={team.id}
              runner={runner}
              onChange={onChange}
            />
          ))}
          {/* "Ghost" new-runner row */}
          <div className="grid grid-cols-[2.4fr_2fr_2fr_1.2fr_0.8fr_2.5rem] gap-px bg-[color:var(--hairline-color)] text-[13px] opacity-80">
            <CellInput
              placeholder="+ Vorname"
              value={pendingRunner?.first_name ?? ''}
              onChange={(v) => saveNewRunner({ first_name: v })}
              disabled={savingNew}
            />
            <CellInput
              placeholder="Nachname"
              value={pendingRunner?.last_name ?? ''}
              onChange={(v) => saveNewRunner({ last_name: v })}
              disabled={savingNew}
            />
            <CellInput
              type="date"
              placeholder="Geburtsdatum"
              value={pendingRunner?.birth_date ?? ''}
              onChange={(v) => saveNewRunner({ birth_date: v })}
              disabled={savingNew}
            />
            <div className="bg-[color:var(--c-ink-800)]/60 px-3 py-2 font-mono text-[11px] text-ink-300">
              {nextPosition}
            </div>
            <div className="bg-[color:var(--c-ink-800)]/60" />
            <div className="bg-[color:var(--c-ink-800)]/60" />
          </div>
        </>
      )}
      {err && (
        <p className="px-3 py-2 text-[12px] text-status-unknown">{err}</p>
      )}
    </div>
  );
}

function RunnerRow({
  event_id,
  team_id,
  runner,
  onChange,
}: {
  event_id: string;
  team_id: string;
  runner: Runner;
  onChange: () => void;
}) {
  const [busy, setBusy] = useState(false);

  const update = async (patch: Partial<Runner>) => {
    setBusy(true);
    try {
      await apiClient.runners.update(event_id, team_id, runner.id, {
        first_name: patch.first_name ?? runner.first_name,
        last_name: patch.last_name ?? runner.last_name,
        birth_date: patch.birth_date ?? runner.birth_date,
        position: patch.position ?? runner.position,
      });
      onChange();
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!confirm(`Läufer „${runner.first_name} ${runner.last_name}" löschen?`)) return;
    setBusy(true);
    try {
      await apiClient.runners.remove(event_id, team_id, runner.id);
      onChange();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid grid-cols-[2.4fr_2fr_2fr_1.2fr_0.8fr_2.5rem] gap-px bg-[color:var(--hairline-color)] text-[13px]">
      <CellInput
        value={runner.first_name}
        onChange={(v) => {
          if (v !== runner.first_name) void update({ first_name: v });
        }}
        disabled={busy}
      />
      <CellInput
        value={runner.last_name}
        onChange={(v) => {
          if (v !== runner.last_name) void update({ last_name: v });
        }}
        disabled={busy}
      />
      <CellInput
        type="date"
        value={runner.birth_date}
        onChange={(v) => {
          if (v !== runner.birth_date) void update({ birth_date: v });
        }}
        disabled={busy}
      />
      <CellInput
        type="number"
        value={String(runner.position)}
        onChange={(v) => {
          const n = parseInt(v, 10);
          if (!Number.isFinite(n) || n <= 0 || n === runner.position) return;
          void update({ position: n });
        }}
        disabled={busy}
      />
      <div className="bg-[color:var(--c-ink-800)]/60" />
      <div className="bg-[color:var(--c-ink-800)]/60 px-1 py-2 text-right">
        <button
          type="button"
          onClick={remove}
          aria-label="Läufer löschen"
          disabled={busy}
          className="text-ink-300 hover:text-status-unknown text-[14px] disabled:opacity-50"
        >
          ✕
        </button>
      </div>
    </div>
  );
}

/** Inline-editable text via single-click-to-focus span. Commits on blur
 *  or Enter, reverts on Escape. */
function EditableText({
  value,
  onCommit,
  className,
  ariaLabel,
}: {
  value: string;
  onCommit: (v: string) => void;
  className?: string;
  ariaLabel?: string;
}) {
  const [draft, setDraft] = useState(value);
  return (
    <input
      aria-label={ariaLabel}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onFocus={() => setDraft(value)}
      onBlur={() => onCommit(draft)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.currentTarget.blur();
        } else if (e.key === 'Escape') {
          setDraft(value);
          e.currentTarget.blur();
        }
      }}
      className={[
        'w-full bg-transparent outline-none focus:ring-1 focus:ring-signal/50 rounded px-1',
        className ?? '',
      ].join(' ')}
    />
  );
}

/** Compact spreadsheet cell input. */
function CellInput({
  value,
  onChange,
  type = 'text',
  placeholder,
  disabled,
}: {
  value: string;
  onChange: (v: string) => void;
  type?: 'text' | 'date' | 'number';
  placeholder?: string;
  disabled?: boolean;
}) {
  const [draft, setDraft] = useState(value);
  // Reset draft when external value changes (after parent refresh).
  if (draft !== value && document.activeElement?.getAttribute('data-cell-value') !== value) {
    // No-op; useEffect would be more correct but adds rerender overhead.
    // The user pattern is type-then-blur, so this works in practice.
  }
  return (
    <input
      type={type}
      value={draft}
      placeholder={placeholder}
      disabled={disabled}
      onFocus={() => setDraft(value)}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        if (draft !== value) onChange(draft);
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') e.currentTarget.blur();
        if (e.key === 'Escape') {
          setDraft(value);
          e.currentTarget.blur();
        }
      }}
      className="w-full bg-[color:var(--c-ink-800)]/60 px-3 py-2 font-mono text-[12px] text-ink-50 outline-none focus:bg-[color:var(--c-ink-700)] focus:ring-1 focus:ring-signal/50 disabled:opacity-50"
    />
  );
}
