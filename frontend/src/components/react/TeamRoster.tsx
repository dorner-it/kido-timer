// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';

import {
  apiClient,
  GENDER_LABELS,
  type EventDetail as EventDetailData,
  type Gender,
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
 * next cell. The "+ Läufer hinzufügen" row at the bottom of each team
 * opens an inline editable row — after the year is fully typed the
 * runner is committed and the row stays open, focus back on Vorname,
 * so the operator can chain entries without clicking through.
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
          um sie zu bearbeiten. Beim Datum springt der Fokus nach Tag, Monat
          und Jahr automatisch weiter.
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
          <div className="grid grid-cols-[2.4fr_2fr_2fr_1.3fr_1.2fr_0.8fr_2.5rem] gap-px bg-[color:var(--hairline-color)] text-[11px]">
            <HeaderCell>Mannschaft / Vorname</HeaderCell>
            <HeaderCell>Nachname</HeaderCell>
            <HeaderCell>Geburtsdatum</HeaderCell>
            <HeaderCell>Geschlecht</HeaderCell>
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
  const [adding, setAdding] = useState(false);
  const [draft, setDraft] = useState<{
    first_name: string;
    last_name: string;
    birth_date: string;
    gender: Gender | '';
  }>({ first_name: '', last_name: '', birth_date: '', gender: '' });
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const firstNameRef = useRef<HTMLInputElement | null>(null);

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
    if (
      !confirm(
        `Mannschaft „${team.name}" löschen? Alle Läufer dieser Mannschaft werden mit gelöscht.`,
      )
    ) {
      return;
    }
    try {
      await apiClient.teams.remove(event_id, team.id);
      onChange();
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Löschen fehlgeschlagen');
    }
  };

  const startAdding = () => {
    setDraft({ first_name: '', last_name: '', birth_date: '', gender: '' });
    setErr(null);
    setAdding(true);
    // Focus is taken care of by NewRunnerRow's useLayoutEffect on
    // mount (see the `key` on the JSX site for why this fires).
  };

  const cancelAdding = () => {
    setAdding(false);
    setDraft({ first_name: '', last_name: '', birth_date: '', gender: '' });
    setErr(null);
  };

  /**
   * Validates and POSTs a new runner. Accepts an optional `override`
   * because the date-complete callback fires *while* the parent's
   * `setDraft` for `birth_date` is still in React's pending batch —
   * reading `draft` from the closure would see the stale (empty) value.
   * Passing the freshly built date through avoids that race.
   */
  const tryCommit = async (
    override: Partial<typeof draft> = {},
  ): Promise<boolean> => {
    const d = { ...draft, ...override };
    if (!d.first_name.trim() || !d.last_name.trim() || !d.birth_date) {
      return false;
    }
    if (!/^\d{4}-\d{2}-\d{2}$/.test(d.birth_date)) return false;
    // Gender is required by the backend so the WG derivation works.
    if (d.gender !== 'male' && d.gender !== 'female') return false;
    setSaving(true);
    setErr(null);
    try {
      await apiClient.runners.create(event_id, team.id, {
        first_name: d.first_name.trim(),
        last_name: d.last_name.trim(),
        birth_date: d.birth_date,
        position: nextPosition,
        gender: d.gender,
      });
      // Keep the row open for chained entry. The data refetch from
      // onChange() bumps runners.length, which changes the `key` on
      // NewRunnerRow below, which forces React to mount a fresh
      // instance. The new instance's useLayoutEffect then focuses
      // Vorname synchronously during commit, which the old setTimeout
      // approach couldn't guarantee.
      setDraft({ first_name: '', last_name: '', birth_date: '', gender: '' });
      onChange();
      return true;
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Läufer konnte nicht angelegt werden');
      return false;
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="border-t border-[color:var(--hairline-color)] bg-[color:var(--c-ink-800)]/40">
      {/* Team header row */}
      <div className="grid grid-cols-[2.4fr_2fr_2fr_1.3fr_1.2fr_0.8fr_2.5rem] gap-px bg-[color:var(--hairline-color)] text-[13px]">
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
        <div className="bg-[color:var(--c-ink-700)] px-3 py-2 col-span-4 font-mono text-[11px] text-ink-200">
          {runners.length} Läufer
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

          {adding ? (
            <NewRunnerRow
              // Keyed on runners.length: any successful save bumps the
              // runners array, which forces this row to remount with a
              // pristine DateInput and triggers the focus useLayoutEffect.
              key={`new-${runners.length}`}
              draft={draft}
              setDraft={setDraft}
              onCommit={tryCommit}
              onCancel={cancelAdding}
              saving={saving}
              nextPosition={nextPosition}
              firstNameRef={firstNameRef}
            />
          ) : (
            <AddRunnerButtonRow onClick={startAdding} nextPosition={nextPosition} />
          )}
        </>
      )}

      {err && <p className="px-3 py-2 text-[12px] text-status-unknown">{err}</p>}
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
        // `patch.gender === null` means the caller explicitly cleared
        // it; otherwise fall back to the stored value (which may itself
        // be `null` for pre-migration runners — the backend tolerates
        // that on PUT).
        gender: patch.gender !== undefined ? patch.gender : runner.gender,
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
    <div className="grid grid-cols-[2.4fr_2fr_2fr_1.3fr_1.2fr_0.8fr_2.5rem] gap-px bg-[color:var(--hairline-color)] text-[13px]">
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
      <DateInputCell
        value={runner.birth_date}
        onCommit={(v) => {
          if (v && v !== runner.birth_date) void update({ birth_date: v });
        }}
        disabled={busy}
      />
      <GenderCell
        value={runner.gender}
        onChange={(g) => {
          if (g !== runner.gender) void update({ gender: g });
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

type RunnerDraft = {
  first_name: string;
  last_name: string;
  birth_date: string;
  gender: Gender | '';
};

function NewRunnerRow({
  draft,
  setDraft,
  onCommit,
  onCancel,
  saving,
  nextPosition,
  firstNameRef,
}: {
  draft: RunnerDraft;
  setDraft: React.Dispatch<React.SetStateAction<RunnerDraft>>;
  onCommit: (override?: Partial<RunnerDraft>) => Promise<boolean>;
  onCancel: () => void;
  saving: boolean;
  nextPosition: number;
  firstNameRef: React.MutableRefObject<HTMLInputElement | null>;
}) {
  const cellCls =
    'w-full bg-[color:var(--c-ink-700)] px-3 py-2 font-mono text-[12px] text-ink-50 outline-none focus:bg-[color:var(--c-ink-600)] focus:ring-1 focus:ring-signal/50 disabled:opacity-50 placeholder:text-ink-300';

  // Fires synchronously during commit, *after* refs are attached but
  // *before* the browser paints. Combined with the `key` on this
  // component (tied to runners.length in the parent), every save
  // triggers a remount which lands the cursor reliably on Vorname —
  // even if the previously focused year input was still in the DOM
  // on the committed frame.
  useLayoutEffect(() => {
    firstNameRef.current?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onEsc = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  };

  return (
    <div className="grid grid-cols-[2.4fr_2fr_2fr_1.3fr_1.2fr_0.8fr_2.5rem] gap-px bg-[color:var(--hairline-color)] text-[13px] animate-rise">
      <input
        ref={firstNameRef}
        type="text"
        value={draft.first_name}
        placeholder="Vorname"
        disabled={saving}
        onChange={(e) => setDraft({ ...draft, first_name: e.target.value })}
        onKeyDown={onEsc}
        className={cellCls}
      />
      <input
        type="text"
        value={draft.last_name}
        placeholder="Nachname"
        disabled={saving}
        onChange={(e) => setDraft({ ...draft, last_name: e.target.value })}
        onKeyDown={onEsc}
        className={cellCls}
      />
      <DateInput
        value={draft.birth_date}
        onChange={(v) => setDraft({ ...draft, birth_date: v })}
        onComplete={(built) => {
          // Birth date is complete; only auto-commit if gender is
          // already chosen. Otherwise wait for the GenderRadio to
          // trigger a commit so we don't 400 on a missing field.
          if (draft.gender === 'male' || draft.gender === 'female') {
            void onCommit({ birth_date: built });
          } else {
            setDraft({ ...draft, birth_date: built });
          }
        }}
        onEscape={onCancel}
        disabled={saving}
      />
      <GenderRadio
        value={draft.gender}
        onChange={(g) => {
          setDraft({ ...draft, gender: g });
          // Auto-commit once gender is the last missing field.
          if (
            draft.first_name.trim() &&
            draft.last_name.trim() &&
            /^\d{4}-\d{2}-\d{2}$/.test(draft.birth_date)
          ) {
            void onCommit({ gender: g });
          }
        }}
        disabled={saving}
      />
      <div className="bg-[color:var(--c-ink-700)] px-3 py-2 font-mono text-[11px] text-ink-300">
        {nextPosition}
      </div>
      <div className="bg-[color:var(--c-ink-700)]" />
      <div className="bg-[color:var(--c-ink-700)] px-1 py-2 text-right">
        <button
          type="button"
          onClick={onCancel}
          aria-label="Abbrechen"
          className="text-ink-300 hover:text-status-unknown text-[14px]"
        >
          ✕
        </button>
      </div>
    </div>
  );
}

/** Compact m/w radio pair for the spreadsheet cell. Deutsche labels. */
function GenderRadio({
  value,
  onChange,
  disabled,
}: {
  value: Gender | '';
  onChange: (g: Gender) => void;
  disabled?: boolean;
}) {
  return (
    <div
      role="radiogroup"
      aria-label="Geschlecht"
      className="flex items-center justify-center gap-2 bg-[color:var(--c-ink-700)] px-2 py-2 font-mono text-[12px] text-ink-50"
    >
      {(['male', 'female'] as const).map((g) => (
        <label
          key={g}
          className={[
            'flex cursor-pointer items-center gap-1 rounded px-2 py-0.5 transition',
            value === g
              ? 'bg-signal/20 text-signal-glow ring-1 ring-signal/50'
              : 'text-ink-200 hover:bg-[color:var(--c-ink-600)]',
            disabled ? 'opacity-50 cursor-not-allowed' : '',
          ].join(' ')}
        >
          <input
            type="radio"
            name={`gender-${Math.random()}`}
            checked={value === g}
            onChange={() => onChange(g)}
            disabled={disabled}
            className="sr-only"
          />
          <span>{GENDER_LABELS[g]}</span>
        </label>
      ))}
    </div>
  );
}

/** Read-write gender cell for existing runners. Falls back to a "—"
 *  hint when the row is from before the migration (gender is null). */
function GenderCell({
  value,
  onChange,
  disabled,
}: {
  value: Gender | null;
  onChange: (g: Gender) => void;
  disabled?: boolean;
}) {
  return <GenderRadio value={value ?? ''} onChange={onChange} disabled={disabled} />;
}

function AddRunnerButtonRow({
  onClick,
  nextPosition,
}: {
  onClick: () => void;
  nextPosition: number;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="grid w-full grid-cols-[2.4fr_2fr_2fr_1.3fr_1.2fr_0.8fr_2.5rem] gap-px bg-[color:var(--hairline-color)] text-[13px] text-left cursor-pointer group"
      aria-label="Läufer hinzufügen"
    >
      <div className="col-span-4 flex items-center gap-2 bg-[color:var(--c-ink-800)]/40 px-3 py-2 font-mono text-[11px] uppercase tracking-[0.18em] text-ink-300 transition group-hover:bg-[color:var(--soft-fill)] group-hover:text-signal-glow">
        <span className="text-signal-glow text-[15px] leading-none">+</span>
        Läufer hinzufügen
      </div>
      <div className="bg-[color:var(--c-ink-800)]/40 px-3 py-2 font-mono text-[11px] text-ink-300 transition group-hover:bg-[color:var(--soft-fill)]">
        {nextPosition}
      </div>
      <div className="bg-[color:var(--c-ink-800)]/40 transition group-hover:bg-[color:var(--soft-fill)]" />
      <div className="bg-[color:var(--c-ink-800)]/40 transition group-hover:bg-[color:var(--soft-fill)]" />
    </button>
  );
}

/** Inline-editable text. Commits on blur or Enter, reverts on Escape.
 *  Syncs `draft` back to `value` whenever the parent re-renders with a
 *  changed value **and** the input is not currently focused — without
 *  that sync, a reset (e.g. parent clearing the value after a save)
 *  would still show the stale text the user typed earlier. */
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
  const [focused, setFocused] = useState(false);

  useEffect(() => {
    if (!focused) setDraft(value);
  }, [value, focused]);

  return (
    <input
      aria-label={ariaLabel}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onFocus={() => {
        setDraft(value);
        setFocused(true);
      }}
      onBlur={() => {
        setFocused(false);
        onCommit(draft);
      }}
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

/** Spreadsheet cell input. Same draft-sync rule as EditableText so a
 *  parent-side reset propagates into the cell when focus is elsewhere. */
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
  const [focused, setFocused] = useState(false);

  useEffect(() => {
    if (!focused) setDraft(value);
  }, [value, focused]);

  return (
    <input
      type={type}
      value={draft}
      placeholder={placeholder}
      disabled={disabled}
      onFocus={() => {
        setDraft(value);
        setFocused(true);
      }}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        setFocused(false);
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

/** Compact 3-segment date editor (TT.MM.JJJJ). Auto-advances when a
 *  segment is full (2 digits day, 2 digits month, 4 digits year) and
 *  fires `onComplete(iso)` the moment a complete, in-range date is
 *  reached. `onChange` is fired with the ISO string when it builds
 *  cleanly, or with `''` when partial. */
function DateInput({
  value,
  onChange,
  onComplete,
  onEscape,
  disabled,
}: {
  value: string;
  onChange: (v: string) => void;
  onComplete?: (built: string) => void;
  onEscape?: () => void;
  disabled?: boolean;
}) {
  const wrapperRef = useRef<HTMLDivElement>(null);
  const dayRef = useRef<HTMLInputElement>(null);
  const monthRef = useRef<HTMLInputElement>(null);
  const yearRef = useRef<HTMLInputElement>(null);
  const initial = useMemo(() => parseIsoDate(value), [value]);
  const [day, setDay] = useState(initial.day);
  const [month, setMonth] = useState(initial.month);
  const [year, setYear] = useState(initial.year);
  const lastEmittedRef = useRef(value);

  // Sync from external value when it changes from outside (e.g. after a
  // commit clears the parent draft). Don't fight the user mid-edit.
  useEffect(() => {
    if (value !== lastEmittedRef.current) {
      const p = parseIsoDate(value);
      setDay(p.day);
      setMonth(p.month);
      setYear(p.year);
      lastEmittedRef.current = value;
    }
  }, [value]);

  const emit = (d: string, m: string, y: string) => {
    const built = buildIsoDate(d, m, y);
    if (built !== lastEmittedRef.current) {
      lastEmittedRef.current = built;
      onChange(built);
    }
    if (built && y.length === 4) {
      onComplete?.(built);
    }
  };

  const handleDay = (raw: string) => {
    const c = raw.replace(/\D/g, '').slice(0, 2);
    setDay(c);
    emit(c, month, year);
    if (c.length === 2) {
      monthRef.current?.focus();
      monthRef.current?.select();
    }
  };

  const handleMonth = (raw: string) => {
    const c = raw.replace(/\D/g, '').slice(0, 2);
    setMonth(c);
    emit(day, c, year);
    if (c.length === 2) {
      yearRef.current?.focus();
      yearRef.current?.select();
    }
  };

  const handleYear = (raw: string) => {
    const c = raw.replace(/\D/g, '').slice(0, 4);
    setYear(c);
    emit(day, month, c);
  };

  const onWrapperBlur = (e: React.FocusEvent) => {
    // Only reset when focus actually leaves the wrapper (not when it
    // hops between the segments).
    if (wrapperRef.current?.contains(e.relatedTarget as Node)) return;
    const built = buildIsoDate(day, month, year);
    if (!built && value) {
      // User left a partial edit. Snap back to the last committed value
      // so the cell always shows truth.
      const p = parseIsoDate(value);
      setDay(p.day);
      setMonth(p.month);
      setYear(p.year);
      lastEmittedRef.current = value;
    }
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      onEscape?.();
    }
  };

  const segCls =
    'bg-transparent text-center tabular-nums outline-none focus:bg-[color:var(--c-ink-600)] focus:ring-1 focus:ring-signal/50 disabled:opacity-50 placeholder:text-ink-300';

  return (
    <div
      ref={wrapperRef}
      onBlur={onWrapperBlur}
      onKeyDown={onKey}
      className="flex items-center justify-center gap-0.5 bg-[color:var(--c-ink-800)]/60 px-2 py-2 font-mono text-[12px] text-ink-50"
    >
      <input
        ref={dayRef}
        type="text"
        inputMode="numeric"
        pattern="[0-9]*"
        maxLength={2}
        value={day}
        onChange={(e) => handleDay(e.target.value)}
        onFocus={(e) => e.currentTarget.select()}
        placeholder="TT"
        disabled={disabled}
        aria-label="Tag"
        className={`${segCls} w-7 rounded`}
      />
      <span className="text-ink-300">.</span>
      <input
        ref={monthRef}
        type="text"
        inputMode="numeric"
        pattern="[0-9]*"
        maxLength={2}
        value={month}
        onChange={(e) => handleMonth(e.target.value)}
        onFocus={(e) => e.currentTarget.select()}
        placeholder="MM"
        disabled={disabled}
        aria-label="Monat"
        className={`${segCls} w-7 rounded`}
      />
      <span className="text-ink-300">.</span>
      <input
        ref={yearRef}
        type="text"
        inputMode="numeric"
        pattern="[0-9]*"
        maxLength={4}
        value={year}
        onChange={(e) => handleYear(e.target.value)}
        onFocus={(e) => e.currentTarget.select()}
        placeholder="JJJJ"
        disabled={disabled}
        aria-label="Jahr"
        className={`${segCls} w-12 rounded`}
      />
    </div>
  );
}

/** DateInput variant for existing rows: emits only on full date AND
 *  rolls back the visible segments to the row's `value` if the user
 *  leaves a partial edit. */
function DateInputCell({
  value,
  onCommit,
  disabled,
}: {
  value: string;
  onCommit: (v: string) => void;
  disabled?: boolean;
}) {
  return (
    <DateInput
      value={value}
      onChange={(v) => {
        if (v) onCommit(v);
      }}
      disabled={disabled}
    />
  );
}

function parseIsoDate(value: string): { day: string; month: string; year: string } {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value || '');
  if (m && m[1] && m[2] && m[3]) {
    return { year: m[1], month: m[2], day: m[3] };
  }
  return { day: '', month: '', year: '' };
}

function buildIsoDate(d: string, m: string, y: string): string {
  if (y.length !== 4) return '';
  if (d.length === 0 || m.length === 0) return '';
  const dn = parseInt(d, 10);
  const mn = parseInt(m, 10);
  const yn = parseInt(y, 10);
  if (!Number.isFinite(dn) || !Number.isFinite(mn) || !Number.isFinite(yn)) return '';
  if (dn < 1 || dn > 31 || mn < 1 || mn > 12 || yn < 1900 || yn > 2100) return '';
  return `${y.padStart(4, '0')}-${String(mn).padStart(2, '0')}-${String(dn).padStart(2, '0')}`;
}
