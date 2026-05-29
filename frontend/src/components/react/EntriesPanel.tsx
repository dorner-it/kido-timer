// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useMemo, useState } from 'react';

import {
  ApiError,
  TIMER_MODE_LABELS,
  TIMER_MODE_LANE_COUNT,
  WERTUNGSGRUPPE_DESCRIPTIONS,
  WERTUNGSGRUPPE_LABELS,
  apiClient,
  type Discipline,
  type EventDetail as EventDetailData,
  type Team,
  type TeamEntry,
} from '../../lib/api';

interface Props {
  data: EventDetailData;
  onChange: () => void;
}

/**
 * Meldungen-Tab: each discipline shows which teams compete in it, with
 * their per-discipline start number and (for two-lane modes) lane
 * assignment. Teams can be added via a dropdown that lists the event's
 * teams not yet entered in that discipline.
 */
export default function EntriesPanel({ data, onChange }: Props) {
  const { event, disciplines, teams, team_entries } = data;
  const [error, setError] = useState<string | null>(null);

  const teamById = useMemo(
    () => new Map(teams.map((t) => [t.id, t])),
    [teams],
  );

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
        Welche Mannschaft startet in welchem Wettkampf? Startnummern und
        Bahn-Zuordnung pro Disziplin (2 Bahnen bei Gruppenstaffette,
        4 Bahnen bei Löschangriff).
      </p>

      {error && (
        <p className="surface px-5 py-3 text-[13px] text-status-unknown">{error}</p>
      )}

      {sortedDisciplines.map((d) => {
        const entries = (entriesByDisc.get(d.id) ?? []).sort(
          (a, b) => a.start_number - b.start_number,
        );
        const usedTeamIds = new Set(entries.map((e) => e.team_id));
        const availableTeams = teams
          .filter((t) => !usedTeamIds.has(t.id))
          .sort((a, b) => a.position - b.position);
        // Lane count comes from the discipline mode (2 for
        // Gruppenstaffette, 4 for Löschangriff). Both v3 modes have >1
        // lanes, so a Bahn-Spalte is always shown.
        const laneCount = TIMER_MODE_LANE_COUNT[d.mode];
        const nextStartNumber =
          (entries[entries.length - 1]?.start_number ?? 0) + 1;
        return (
          <DisciplineEntriesBlock
            key={d.id}
            event_id={event.id}
            discipline={d}
            entries={entries}
            teamById={teamById}
            availableTeams={availableTeams}
            laneCount={laneCount}
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
  availableTeams: Team[];
  laneCount: number;
  nextStartNumber: number;
  onChange: () => void;
  onError: (msg: string) => void;
}) {
  const wantsLanes = laneCount > 1;
  const laneOptions = Array.from({ length: laneCount }, (_, i) => i + 1);
  const [addOpen, setAddOpen] = useState(false);

  const upsert = async (entry: {
    team_id: string;
    start_number: number;
    lane: number | null;
  }) => {
    try {
      await apiClient.teamEntries.upsert(event_id, discipline.id, entry);
      onChange();
    } catch (e) {
      if (e instanceof ApiError && e.code === 'wg_inconsistent') {
        onError(`Wertungsgruppe konnte nicht abgeleitet werden: ${e.message}`);
      } else {
        onError(e instanceof Error ? e.message : 'Speichern fehlgeschlagen');
      }
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

  return (
    <div className="surface overflow-hidden">
      <header className="flex items-center justify-between border-b border-[color:var(--hairline-color)] px-5 py-3">
        <div className="flex flex-col">
          <span className="label">{TIMER_MODE_LABELS[discipline.mode]}</span>
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

      {addOpen && availableTeams.length > 0 && (
        <AddEntryRow
          availableTeams={availableTeams}
          wantsLanes={wantsLanes}
          laneOptions={laneOptions}
          defaultStartNumber={nextStartNumber}
          onSubmit={async (entry) => {
            await upsert(entry);
            setAddOpen(false);
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
              <Th className="w-24">WG</Th>
              {wantsLanes && <Th className="w-20">Bahn</Th>}
              <Th className="w-12 text-right"> </Th>
            </tr>
          </thead>
          <tbody>
            {entries.map((e) => {
              const team = teamById.get(e.team_id);
              return (
                <EntryRow
                  key={e.team_id}
                  entry={e}
                  team={team}
                  wantsLanes={wantsLanes}
                  laneOptions={laneOptions}
                  onCommit={(patch) => upsert({ ...e, ...patch })}
                  onRemove={() => remove(e.team_id)}
                />
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}

function AddEntryRow({
  availableTeams,
  wantsLanes,
  laneOptions,
  defaultStartNumber,
  onSubmit,
  onCancel,
}: {
  availableTeams: Team[];
  wantsLanes: boolean;
  laneOptions: number[];
  defaultStartNumber: number;
  onSubmit: (input: {
    team_id: string;
    start_number: number;
    lane: number | null;
  }) => Promise<void>;
  onCancel: () => void;
}) {
  const [teamId, setTeamId] = useState(availableTeams[0]?.id ?? '');
  const [startNumber, setStartNumber] = useState(defaultStartNumber);
  const [lane, setLane] = useState<string>('');
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!teamId) return;
    setBusy(true);
    try {
      const laneNum = lane === '' ? null : parseInt(lane, 10);
      await onSubmit({
        team_id: teamId,
        start_number: startNumber,
        lane: laneNum,
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      onSubmit={submit}
      className="grid grid-cols-1 gap-3 border-b border-[color:var(--hairline-color)] bg-[color:var(--soft-fill)] px-5 py-3 md:grid-cols-[2fr_1fr_1fr_auto]"
    >
      <div className="field">
        <label className="label">Mannschaft</label>
        <select
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
      </div>
      <div className="field">
        <label className="label">Startnummer</label>
        <input
          type="number"
          min={1}
          className="input"
          value={startNumber}
          onChange={(e) => setStartNumber(parseInt(e.target.value || '1', 10))}
        />
      </div>
      <div className="field">
        <label className="label">Bahn</label>
        {wantsLanes ? (
          <select
            className="input"
            value={lane}
            onChange={(e) => setLane(e.target.value)}
          >
            <option value="">—</option>
            {laneOptions.map((n) => (
              <option key={n} value={String(n)}>
                {n}
              </option>
            ))}
          </select>
        ) : (
          <input className="input" value="—" disabled />
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
  wantsLanes,
  laneOptions,
  onCommit,
  onRemove,
}: {
  entry: TeamEntry;
  team: Team | undefined;
  wantsLanes: boolean;
  laneOptions: number[];
  onCommit: (patch: { start_number?: number; lane?: number | null }) => void;
  onRemove: () => void;
}) {
  return (
    <tr className="border-b border-[color:var(--hairline-color)] last:border-0">
      <td className="px-3 py-2">
        <NumberCell
          value={entry.start_number}
          onCommit={(n) => {
            if (n > 0 && n !== entry.start_number) onCommit({ start_number: n });
          }}
        />
      </td>
      <td className="px-3 py-2 font-display text-ink-50">{team?.name ?? '?'}</td>
      <td className="px-3 py-2">
        <WgBadge wg={entry.wg} />
      </td>
      {wantsLanes && (
        <td className="px-3 py-2">
          <select
            className="input"
            value={entry.lane?.toString() ?? ''}
            onChange={(e) => {
              const v = e.target.value === '' ? null : parseInt(e.target.value, 10);
              onCommit({ lane: v });
            }}
          >
            <option value="">—</option>
            {laneOptions.map((n) => (
              <option key={n} value={String(n)}>
                {n}
              </option>
            ))}
          </select>
        </td>
      )}
      <td className="px-3 py-2 text-right">
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

/** Compact WG badge with hover tooltip. Renders a warning glyph when
 *  the entry has no WG stamp (created before the migration, or the
 *  roster was inconsistent at the time of the last write). */
function WgBadge({ wg }: { wg: TeamEntry['wg'] }) {
  if (!wg) {
    return (
      <span
        className="inline-flex items-center gap-1 rounded bg-status-unknown/15 px-2 py-0.5 font-mono text-[11px] text-status-unknown"
        title="Wertungsgruppe nicht gesetzt — Roster prüfen und Meldung neu speichern."
        aria-label="Wertungsgruppe nicht gesetzt"
      >
        ⚠ —
      </span>
    );
  }
  return (
    <span
      className="inline-flex items-center rounded bg-signal/15 px-2 py-0.5 font-mono text-[11px] font-semibold text-signal-glow"
      title={WERTUNGSGRUPPE_DESCRIPTIONS[wg]}
    >
      {WERTUNGSGRUPPE_LABELS[wg]}
    </span>
  );
}
