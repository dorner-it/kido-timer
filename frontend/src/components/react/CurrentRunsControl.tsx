// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useMemo, useState } from 'react';

import {
  RUN_STATUS_LABELS,
  apiClient,
  type Discipline,
  type Event,
  type Run,
  type Team,
  type TeamEntry,
} from '../../lib/api';
import {
  candidatesForLane,
  selectionChanged,
  selectionFromCurrent,
  setCurrentBody,
  suggestNext,
  validateSelection,
} from '../../lib/current-runs';
import { effectiveLaneCount, laneCountLabel } from '../../lib/lanes';
import { entryForRun, runLabel } from '../../lib/runs';
import { wgLabel, WG_MISSING_LABEL } from '../../lib/wg';

interface Props {
  event: Event;
  discipline: Discipline;
  runs: Run[];
  entries: TeamEntry[];
  teams: Team[];
  onSaved: (event: Event) => void;
  onError: (msg: string) => void;
}

/** "Laufende Läufe": pick the current run per lane (max `lane_count`). */
export default function CurrentRunsControl({ event, discipline, runs, entries, teams, onSaved, onError }: Props) {
  const laneCount = effectiveLaneCount(discipline);
  const stored = useMemo(
    () => selectionFromCurrent(event.current_run_ids ?? [], runs, entries, laneCount),
    [event.current_run_ids, runs, entries, laneCount],
  );
  const [selection, setSelection] = useState<(string | null)[]>(stored);
  const [dirty, setDirty] = useState(false);
  const [busy, setBusy] = useState(false);

  // Follow server state (other tab / desktop) while the organizer isn't editing.
  const storedKey = stored.join('|');
  useEffect(() => {
    if (!dirty) setSelection(stored);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [storedKey, dirty]);

  const errors = validateSelection(selection, runs, entries, laneCount, discipline.id);
  const changed = selectionChanged(selection, event.current_run_ids ?? []);
  const runById = useMemo(() => new Map(runs.map((r) => [r.id, r])), [runs]);

  const save = async (next: (string | null)[]) => {
    const errs = validateSelection(next, runs, entries, laneCount, discipline.id);
    if (errs.length > 0) return;
    setBusy(true);
    try {
      const ev = await apiClient.events.setCurrent(event.id, setCurrentBody(discipline.id, next));
      setDirty(false);
      onSaved(ev);
    } catch (e) {
      onError(e instanceof Error ? e.message : 'Laufende Läufe konnten nicht gesetzt werden');
    } finally {
      setBusy(false);
    }
  };

  const setSlot = (idx: number, id: string | null) => {
    setDirty(true);
    setSelection((cur) => cur.map((v, i) => (i === idx ? id : v)));
  };

  return (
    <section className="surface flex flex-col gap-4 p-5">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <div className="flex flex-col gap-1">
          <span className="label">Laufende Läufe</span>
          <span className="font-mono text-[11px] text-ink-200">
            {discipline.name} · {laneCountLabel(laneCount)} · ein Lauf je Bahn
          </span>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => {
              setDirty(true);
              setSelection(suggestNext(selection, runs, entries, laneCount));
            }}
            title="Pro Bahn den nächsten vorgemerkten Lauf auswählen"
          >
            Nächste Läufe
          </button>
          <button
            type="button"
            className="btn"
            disabled={busy || selection.every((s) => s == null)}
            onClick={() => {
              setDirty(true);
              setSelection(selection.map(() => null));
            }}
          >
            Bahnen leeren
          </button>
          {dirty && (
            <button
              type="button"
              className="btn"
              disabled={busy}
              onClick={() => {
                setDirty(false);
                setSelection(stored);
              }}
            >
              Verwerfen
            </button>
          )}
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy || !changed || errors.length > 0}
            onClick={() => save(selection)}
          >
            {busy ? 'Setze …' : 'Auf die Bahn'}
          </button>
        </div>
      </div>

      <div className={`grid grid-cols-1 gap-3 ${laneCount > 1 ? 'md:grid-cols-2' : ''}`}>
        {selection.slice(0, laneCount).map((id, idx) => {
          const lane = idx + 1;
          const run = id ? runById.get(id) : undefined;
          const entry = run ? entryForRun(run, entries) : undefined;
          const team = run ? teams.find((t) => t.id === run.team_id) : undefined;
          const options = candidatesForLane(lane, runs, entries, laneCount);
          const isStored = (event.current_run_ids ?? []).includes(id ?? '');
          return (
            <div
              key={lane}
              className={[
                'flex flex-col gap-2 rounded-xl border p-4',
                run && isStored
                  ? 'border-status-running/40 bg-status-running/5'
                  : 'border-[color:var(--hairline-color)] bg-[color:var(--soft-fill)]',
              ].join(' ')}
            >
              <div className="flex items-center justify-between">
                <span className="font-stencil text-[22px] font-bold leading-none text-ink-50">Bahn {lane}</span>
                {run && isStored && (
                  <span className="pill bg-status-running/15 text-status-running">
                    <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-current" />
                    läuft
                  </span>
                )}
              </div>
              <select
                className="input"
                aria-label={`Lauf auf Bahn ${lane}`}
                value={id ?? ''}
                onChange={(e) => setSlot(idx, e.target.value || null)}
              >
                <option value="">— kein Lauf —</option>
                {id && !options.some((o) => o.id === id) && (
                  <option value={id}>{run ? runLabel(run, teams, entries) : 'Unbekannter Lauf'}</option>
                )}
                {options.map((r) => (
                  <option key={r.id} value={r.id}>
                    {runLabel(r, teams, entries)}
                    {r.status !== 'pending' ? ` (${RUN_STATUS_LABELS[r.status]})` : ''}
                  </option>
                ))}
              </select>
              {run ? (
                <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1 text-[13px]">
                  <span className="font-display font-semibold text-ink-50">{team?.name ?? '?'}</span>
                  <span className="font-mono text-[11px] text-ink-200">
                    Startnr. {entry?.start_number ?? '—'}
                  </span>
                  <span className="font-mono text-[11px] text-ink-200">
                    {entry ? wgLabel(entry.wg, WG_MISSING_LABEL) : WG_MISSING_LABEL}
                  </span>
                  <span className="font-mono text-[11px] text-ink-300">Lauf {run.run_number}</span>
                </div>
              ) : (
                <span className="text-[12px] text-ink-300">Bahn frei</span>
              )}
            </div>
          );
        })}
      </div>

      {errors.length > 0 && (
        <ul className="flex flex-col gap-1 text-[12px] text-status-unknown" role="alert">
          {errors.map((e) => (
            <li key={e}>⚠ {e}</li>
          ))}
        </ul>
      )}
      {runs.length === 0 && (
        <p className="text-[12px] text-ink-200">Noch keine Läufe angelegt – unten „Durchgang anlegen".</p>
      )}
    </section>
  );
}
