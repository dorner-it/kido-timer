// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { Fragment, useMemo, useState } from 'react';

import {
  TIMER_MODE_LABELS,
  apiClient,
  type Discipline,
  type EventDetail as EventDetailData,
  type Runner,
  type Team,
  type TeamEntry,
  type TeamEntryInput,
  type Wertungsgruppe,
} from '../../lib/api';
import {
  effectiveLaneCount,
  isLaneConflict,
  laneConflicts,
  laneCountLabel,
  laneForNewEntry,
  laneOptions,
  showsLanePicker,
} from '../../lib/lanes';
import { fieldErrors, moveWgFormSchema, teamEntryFormSchema } from '../../lib/schemas';
import {
  WERTUNGSGRUPPEN,
  WERTUNGSGRUPPE_DESCRIPTIONS,
  WG_MISSING_LABEL,
  formatWgHistoryEntry,
  formatWgWarnings,
  wgLabel,
  wgOptionLabel,
  wgWarningTooltip,
} from '../../lib/wg';

interface Props {
  data: EventDetailData;
  onChange: () => void;
}

type RunnersById = ReadonlyMap<string, Runner>;

/**
 * Meldungen-Tab: each discipline shows which teams compete in it, with
 * their per-discipline start number, Wertungsgruppe (ticked by the
 * club) and — for multi-lane disciplines — lane assignment. Teams can
 * be added via a dropdown that lists the event's teams not yet entered
 * in that discipline.
 */
export default function EntriesPanel({ data, onChange }: Props) {
  const { event, disciplines, teams, team_entries, runners } = data;
  const [error, setError] = useState<string | null>(null);

  const teamById = useMemo(() => new Map(teams.map((t) => [t.id, t])), [teams]);
  const runnersById = useMemo(() => new Map(runners.map((r) => [r.id, r])), [runners]);

  if (teams.length === 0) {
    return (
      <p className="surface px-5 py-4 text-[13px] text-ink-200">
        Erst Mannschaften unter „Mannschaften & Läufer" anlegen, dann
        Meldungen verteilen.
      </p>
    );
  }

  if (disciplines.length === 0) {
    return (
      <p className="surface px-5 py-4 text-[13px] text-ink-200">
        Erst Wettkämpfe unter „Wettkämpfe" anlegen, dann Meldungen verteilen.
      </p>
    );
  }

  const entriesByDisc = new Map<string, TeamEntry[]>();
  for (const e of team_entries) {
    const list = entriesByDisc.get(e.discipline_id) ?? [];
    list.push(e);
    entriesByDisc.set(e.discipline_id, list);
  }

  const sortedDisciplines = [...disciplines].sort((a, b) => a.position - b.position);

  return (
    <section className="flex flex-col gap-5">
      <p className="font-mono text-[12px] text-ink-200">
        Welche Mannschaft startet in welchem Wettkampf? Startnummer,
        Wertungsgruppe (laut Meldebogen des Vereins) und Bahn pro Disziplin.
        Passt das Alter eines Läufers nicht zur WG, erscheint ein rotes
        Kreuz — gespeichert wird trotzdem.
      </p>

      {error && (
        <p className="surface px-5 py-3 text-[13px] text-status-unknown" role="alert">
          {error}
        </p>
      )}

      {sortedDisciplines.map((d) => {
        const entries = (entriesByDisc.get(d.id) ?? []).sort(
          (a, b) => a.start_number - b.start_number,
        );
        const usedTeamIds = new Set(entries.map((e) => e.team_id));
        const availableTeams = teams
          .filter((t) => !usedTeamIds.has(t.id))
          .sort((a, b) => a.position - b.position);
        const nextStartNumber = (entries[entries.length - 1]?.start_number ?? 0) + 1;
        return (
          <DisciplineEntriesBlock
            key={d.id}
            event_id={event.id}
            discipline={d}
            entries={entries}
            teamById={teamById}
            runnersById={runnersById}
            availableTeams={availableTeams}
            laneCount={effectiveLaneCount(d)}
            nextStartNumber={nextStartNumber}
            onChange={onChange}
            onError={setError}
          />
        );
      })}
    </section>
  );
}

function DisciplineEntriesBlock({
  event_id,
  discipline,
  entries,
  teamById,
  runnersById,
  availableTeams,
  laneCount,
  nextStartNumber,
  onChange,
  onError,
}: {
  event_id: string;
  discipline: Discipline;
  entries: TeamEntry[];
  teamById: Map<string, Team>;
  runnersById: RunnersById;
  availableTeams: Team[];
  laneCount: number;
  nextStartNumber: number;
  onChange: () => void;
  onError: (msg: string | null) => void;
}) {
  const wantsLanes = showsLanePicker(laneCount);
  const [addOpen, setAddOpen] = useState(false);
  const conflicts = laneConflicts(entries, laneCount);

  const upsert = async (entry: TeamEntryInput): Promise<boolean> => {
    try {
      await apiClient.teamEntries.upsert(event_id, discipline.id, entry);
      onError(null);
      onChange();
      return true;
    } catch (e) {
      onError(e instanceof Error ? e.message : 'Speichern fehlgeschlagen');
      return false;
    }
  };

  const remove = async (team_id: string) => {
    if (!confirm('Meldung entfernen?')) return;
    try {
      await apiClient.teamEntries.remove(event_id, discipline.id, team_id);
      onChange();
    } catch (e) {
      onError(e instanceof Error ? e.message : 'Entfernen fehlgeschlagen');
    }
  };

  const moveWg = async (team_id: string, wg: Wertungsgruppe, note?: string) => {
    try {
      await apiClient.teamEntries.moveWg(event_id, discipline.id, team_id, { wg, note });
      onError(null);
      onChange();
      return true;
    } catch (e) {
      onError(e instanceof Error ? e.message : 'Umsetzen fehlgeschlagen');
      return false;
    }
  };

  return (
    <div className="surface overflow-hidden">
      <header className="flex items-center justify-between border-b border-[color:var(--hairline-color)] px-5 py-3">
        <div className="flex flex-col">
          <span className="label">
            {TIMER_MODE_LABELS[discipline.mode]} · {laneCountLabel(laneCount)}
          </span>
          <h3 className="font-display text-[17px] font-semibold text-ink-50">
            {discipline.name}
          </h3>
        </div>
        <div className="flex items-center gap-2">
          <span className="font-mono text-[11px] text-ink-200">
            {entries.length} Meldungen
          </span>
          <button
            type="button"
            onClick={() => setAddOpen((v) => !v)}
            className="btn btn-primary"
            disabled={availableTeams.length === 0}
            title={
              availableTeams.length === 0
                ? 'Alle Mannschaften sind bereits gemeldet'
                : undefined
            }
          >
            + Meldung
          </button>
        </div>
      </header>

      {conflicts.length > 0 && (
        <p
          className="border-b border-[color:var(--hairline-color)] px-5 py-2 font-mono text-[12px] text-status-armed"
          role="alert"
        >
          ⚠ {conflicts.length === 1 ? '1 Meldung liegt' : `${conflicts.length} Meldungen liegen`}{' '}
          auf einer Bahn größer als {laneCount}:{' '}
          {conflicts.map((id) => teamById.get(id)?.name ?? id).join(', ')}
        </p>
      )}

      {addOpen && availableTeams.length > 0 && (
        <AddEntryRow
          availableTeams={availableTeams}
          laneCount={laneCount}
          defaultStartNumber={nextStartNumber}
          onSubmit={async (entry) => {
            if (await upsert(entry)) setAddOpen(false);
          }}
          onCancel={() => setAddOpen(false)}
        />
      )}

      {entries.length === 0 ? (
        <p className="px-5 py-4 font-mono text-[12px] text-ink-200">
          Noch keine Meldungen.
        </p>
      ) : (
        <table className="w-full text-[13px]">
          <thead>
            <tr className="border-b border-[color:var(--hairline-color)] text-left">
              <Th className="w-20">Startnr.</Th>
              <Th>Mannschaft</Th>
              <Th className="w-72">WG</Th>
              {wantsLanes && <Th className="w-28">Bahn</Th>}
              <Th className="w-12 text-right"> </Th>
            </tr>
          </thead>
          <tbody>
            {entries.map((e) => (
              <EntryRow
                key={e.team_id}
                entry={e}
                team={teamById.get(e.team_id)}
                runnersById={runnersById}
                wantsLanes={wantsLanes}
                laneCount={laneCount}
                colSpan={wantsLanes ? 5 : 4}
                onCommit={(patch) => {
                  const wg = patch.wg ?? e.wg;
                  if (!wg) {
                    onError(
                      `${teamById.get(e.team_id)?.name ?? 'Meldung'}: bitte zuerst die Wertungsgruppe setzen.`,
                    );
                    return;
                  }
                  void upsert({
                    team_id: e.team_id,
                    start_number: patch.start_number ?? e.start_number,
                    lane: patch.lane !== undefined ? patch.lane : e.lane,
                    wg,
                  });
                }}
                onMoveWg={(wg, note) => moveWg(e.team_id, wg, note)}
                onRemove={() => remove(e.team_id)}
              />
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function AddEntryRow({
  availableTeams,
  laneCount,
  defaultStartNumber,
  onSubmit,
  onCancel,
}: {
  availableTeams: Team[];
  laneCount: number;
  defaultStartNumber: number;
  onSubmit: (input: TeamEntryInput) => Promise<void>;
  onCancel: () => void;
}) {
  const wantsLanes = showsLanePicker(laneCount);
  const [teamId, setTeamId] = useState(availableTeams[0]?.id ?? '');
  const [startNumber, setStartNumber] = useState(defaultStartNumber);
  const [lane, setLane] = useState<string>('');
  const [wg, setWg] = useState<string>('');
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const chosenLane = lane === '' ? null : parseInt(lane, 10);
    const parsed = teamEntryFormSchema.safeParse({
      team_id: teamId,
      start_number: startNumber,
      lane: laneForNewEntry(laneCount, chosenLane),
      wg: wg || undefined,
    });
    if (!parsed.success) {
      setErrors(fieldErrors(parsed.error));
      return;
    }
    setErrors({});
    setBusy(true);
    try {
      await onSubmit(parsed.data);
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      onSubmit={submit}
      className="grid grid-cols-1 gap-3 border-b border-[color:var(--hairline-color)] bg-[color:var(--soft-fill)] px-5 py-3 md:grid-cols-[2fr_1fr_2fr_1fr_auto]"
    >
      <div className="field">
        <label className="label" htmlFor="entry-team">
          Mannschaft
        </label>
        <select
          id="entry-team"
          className="input"
          value={teamId}
          onChange={(e) => setTeamId(e.target.value)}
          autoFocus
        >
          {availableTeams.map((t) => (
            <option key={t.id} value={t.id}>
              {t.name}
            </option>
          ))}
        </select>
        {errors.team_id && <FieldError>{errors.team_id}</FieldError>}
      </div>
      <div className="field">
        <label className="label" htmlFor="entry-start">
          Startnummer
        </label>
        <input
          id="entry-start"
          type="number"
          min={1}
          className="input"
          value={startNumber}
          onChange={(e) => setStartNumber(parseInt(e.target.value || '1', 10))}
        />
        {errors.start_number && <FieldError>{errors.start_number}</FieldError>}
      </div>
      <div className="field">
        <label className="label" htmlFor="entry-wg">
          Wertungsgruppe *
        </label>
        <select
          id="entry-wg"
          className="input"
          value={wg}
          aria-invalid={errors.wg ? true : undefined}
          onChange={(e) => setWg(e.target.value)}
        >
          <option value="">— WG wählen —</option>
          {WERTUNGSGRUPPEN.map((g) => (
            <option key={g} value={g}>
              {wgOptionLabel(g)}
            </option>
          ))}
        </select>
        {errors.wg && <FieldError>{errors.wg}</FieldError>}
      </div>
      <div className="field">
        <label className="label" htmlFor="entry-lane">
          Bahn
        </label>
        {wantsLanes ? (
          <select
            id="entry-lane"
            className="input"
            value={lane}
            onChange={(e) => setLane(e.target.value)}
          >
            <option value="">—</option>
            {laneOptions(laneCount).map((o) => (
              <option key={o.value} value={String(o.value)}>
                {o.label}
              </option>
            ))}
          </select>
        ) : (
          <input id="entry-lane" className="input" value="1" disabled />
        )}
      </div>
      <div className="flex items-end gap-2">
        <button type="submit" className="btn btn-primary" disabled={busy}>
          {busy ? '…' : 'Anlegen'}
        </button>
        <button type="button" className="btn" onClick={onCancel}>
          Abbrechen
        </button>
      </div>
    </form>
  );
}

function EntryRow({
  entry,
  team,
  runnersById,
  wantsLanes,
  laneCount,
  colSpan,
  onCommit,
  onMoveWg,
  onRemove,
}: {
  entry: TeamEntry;
  team: Team | undefined;
  runnersById: RunnersById;
  wantsLanes: boolean;
  laneCount: number;
  colSpan: number;
  onCommit: (patch: {
    start_number?: number;
    lane?: number | null;
    wg?: Wertungsgruppe;
  }) => void;
  onMoveWg: (wg: Wertungsgruppe, note?: string) => Promise<boolean>;
  onRemove: () => void;
}) {
  const [moveOpen, setMoveOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const history = entry.wg_history ?? [];
  const warnings = formatWgWarnings(entry.wg_warnings, runnersById);
  const laneConflict = isLaneConflict(entry.lane, laneCount);

  return (
    <Fragment>
      <tr className="border-b border-[color:var(--hairline-color)] last:border-0">
        <td className="px-3 py-2 align-top">
          <NumberCell
            value={entry.start_number}
            onCommit={(n) => {
              if (n > 0 && n !== entry.start_number) onCommit({ start_number: n });
            }}
          />
        </td>
        <td className="px-3 py-2 align-top font-display text-ink-50">
          {team?.name ?? '?'}
          {warnings.length > 0 && (
            <ul className="mt-1 flex flex-col gap-0.5 font-mono text-[11px] text-status-unknown">
              {warnings.map((w) => (
                <li key={w}>✕ {w}</li>
              ))}
            </ul>
          )}
        </td>
        <td className="px-3 py-2 align-top">
          <div className="flex flex-wrap items-center gap-2">
            {entry.wg ? (
              <WgBadge wg={entry.wg} />
            ) : (
              <MissingWgPicker onPick={(wg) => onCommit({ wg })} />
            )}
            <WgWarningMarker entry={entry} runnersById={runnersById} />
            {entry.wg && (
              <button
                type="button"
                className="label underline-offset-2 hover:text-ink-50 hover:underline"
                onClick={() => setMoveOpen((v) => !v)}
              >
                {moveOpen ? 'Abbrechen' : 'Umsetzen'}
              </button>
            )}
            {history.length > 0 && (
              <button
                type="button"
                className="label underline-offset-2 hover:text-ink-50 hover:underline"
                onClick={() => setHistoryOpen((v) => !v)}
                aria-expanded={historyOpen}
              >
                Verlauf ({history.length})
              </button>
            )}
          </div>
        </td>
        {wantsLanes && (
          <td className="px-3 py-2 align-top">
            <select
              className={`input ${laneConflict ? 'text-status-armed' : ''}`}
              value={entry.lane?.toString() ?? ''}
              title={
                laneConflict
                  ? `Bahn ${entry.lane} gibt es in diesem Wettkampf nicht mehr.`
                  : undefined
              }
              onChange={(e) => {
                const v = e.target.value === '' ? null : parseInt(e.target.value, 10);
                onCommit({ lane: v });
              }}
            >
              <option value="">—</option>
              {laneOptions(laneCount, entry.lane).map((o) => (
                <option key={o.value} value={String(o.value)}>
                  {o.label}
                </option>
              ))}
            </select>
          </td>
        )}
        <td className="px-3 py-2 text-right align-top">
          <button
            type="button"
            onClick={onRemove}
            aria-label="Meldung entfernen"
            className="text-ink-300 hover:text-status-unknown text-[14px]"
          >
            ✕
          </button>
        </td>
      </tr>
      {moveOpen && entry.wg && (
        <tr className="border-b border-[color:var(--hairline-color)] bg-[color:var(--soft-fill)]">
          <td colSpan={colSpan} className="px-5 py-3">
            <MoveWgForm
              current={entry.wg}
              teamName={team?.name ?? '?'}
              idSuffix={entry.team_id}
              onSubmit={async (wg, note) => {
                if (await onMoveWg(wg, note)) {
                  setMoveOpen(false);
                  setHistoryOpen(true);
                }
              }}
              onCancel={() => setMoveOpen(false)}
            />
          </td>
        </tr>
      )}
      {historyOpen && history.length > 0 && (
        <tr className="border-b border-[color:var(--hairline-color)]">
          <td colSpan={colSpan} className="px-5 py-2">
            <span className="label">WG-Verlauf</span>
            <ul className="mt-1 flex flex-col gap-0.5 font-mono text-[11px] text-ink-200">
              {history.map((h, i) => (
                <li key={`${h.at}-${i}`}>{formatWgHistoryEntry(h)}</li>
              ))}
            </ul>
          </td>
        </tr>
      )}
    </Fragment>
  );
}

/** Small inline dialog for "Umsetzen" (move a team to another WG). */
function MoveWgForm({
  current,
  teamName,
  idSuffix,
  onSubmit,
  onCancel,
}: {
  current: Wertungsgruppe;
  teamName: string;
  idSuffix: string;
  onSubmit: (wg: Wertungsgruppe, note?: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [wg, setWg] = useState<string>('');
  const [note, setNote] = useState('');
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const parsed = moveWgFormSchema.safeParse({ wg: wg || undefined, note });
    if (!parsed.success) {
      setErrors(fieldErrors(parsed.error));
      return;
    }
    if (parsed.data.wg === current) {
      setErrors({ wg: 'Mannschaft ist bereits in dieser WG.' });
      return;
    }
    setErrors({});
    setBusy(true);
    try {
      await onSubmit(parsed.data.wg, parsed.data.note);
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      onSubmit={submit}
      role="dialog"
      aria-label={`${teamName} umsetzen`}
      className="grid grid-cols-1 gap-3 md:grid-cols-[2fr_3fr_auto]"
    >
      <div className="field">
        <label className="label" htmlFor={`move-wg-${idSuffix}`}>
          {teamName}: {wgLabel(current)} → Ziel-WG
        </label>
        <select
          id={`move-wg-${idSuffix}`}
          className="input"
          value={wg}
          autoFocus
          onChange={(e) => setWg(e.target.value)}
        >
          <option value="">— WG wählen —</option>
          {WERTUNGSGRUPPEN.filter((g) => g !== current).map((g) => (
            <option key={g} value={g}>
              {wgOptionLabel(g)}
            </option>
          ))}
        </select>
        {errors.wg && <FieldError>{errors.wg}</FieldError>}
      </div>
      <div className="field">
        <label className="label" htmlFor={`move-note-${idSuffix}`}>
          Notiz (optional)
        </label>
        <input
          id={`move-note-${idSuffix}`}
          className="input"
          value={note}
          maxLength={500}
          placeholder="z.B. 2 Mädchen in der Mannschaft"
          onChange={(e) => setNote(e.target.value)}
        />
        {errors.note && <FieldError>{errors.note}</FieldError>}
      </div>
      <div className="flex items-end gap-2">
        <button type="submit" className="btn btn-primary" disabled={busy}>
          {busy ? '…' : 'Umsetzen'}
        </button>
        <button type="button" className="btn" onClick={onCancel}>
          Abbrechen
        </button>
      </div>
    </form>
  );
}

function NumberCell({
  value,
  onCommit,
}: {
  value: number;
  onCommit: (v: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));
  return (
    <input
      type="number"
      min={1}
      value={draft}
      onFocus={() => setDraft(String(value))}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        const n = parseInt(draft, 10);
        if (Number.isFinite(n) && n > 0) onCommit(n);
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter') e.currentTarget.blur();
        if (e.key === 'Escape') {
          setDraft(String(value));
          e.currentTarget.blur();
        }
      }}
      className="input w-20 font-mono tnum text-[13px]"
    />
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

function FieldError({ children }: { children: React.ReactNode }) {
  return <p className="text-[12px] text-status-unknown">{children}</p>;
}

/** Compact WG badge with the long label as tooltip. */
function WgBadge({ wg }: { wg: Wertungsgruppe }) {
  return (
    <span
      className="inline-flex items-center rounded bg-signal/15 px-2 py-0.5 font-mono text-[11px] font-semibold text-signal-glow"
      title={WERTUNGSGRUPPE_DESCRIPTIONS[wg]}
    >
      {wgLabel(wg)}
    </span>
  );
}

/** Legacy rows without WG: "WG fehlt" badge plus a select to set it. */
function MissingWgPicker({ onPick }: { onPick: (wg: Wertungsgruppe) => void }) {
  return (
    <span className="inline-flex items-center gap-2">
      <span
        className="inline-flex items-center gap-1 rounded bg-status-unknown/15 px-2 py-0.5 font-mono text-[11px] text-status-unknown"
        title="Wertungsgruppe nicht gesetzt — bitte laut Meldebogen auswählen."
      >
        ⚠ {WG_MISSING_LABEL}
      </span>
      <select
        className="input py-0.5 text-[12px]"
        value=""
        aria-label="Wertungsgruppe setzen"
        onChange={(e) => {
          if (e.target.value) onPick(e.target.value as Wertungsgruppe);
        }}
      >
        <option value="">WG setzen …</option>
        {WERTUNGSGRUPPEN.map((g) => (
          <option key={g} value={g}>
            {wgOptionLabel(g)}
          </option>
        ))}
      </select>
    </span>
  );
}

/** Red cross ("rotes Kreuz") when a runner's age doesn't fit the WG. */
function WgWarningMarker({
  entry,
  runnersById,
}: {
  entry: TeamEntry;
  runnersById: RunnersById;
}) {
  const count = entry.wg_warnings?.length ?? 0;
  if (count === 0) return null;
  const tooltip = wgWarningTooltip(entry.wg_warnings, runnersById);
  return (
    <span
      className="inline-flex items-center gap-1 rounded bg-status-unknown/15 px-2 py-0.5 font-mono text-[11px] font-bold text-status-unknown"
      title={tooltip}
      aria-label={tooltip}
      role="img"
    >
      ✕ {count}
    </span>
  );
}
