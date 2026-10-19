// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import { apiClient, type Run, type Runner } from '../../lib/api';
import {
  lineupBody,
  lineupChanged,
  lineupConfirmationText,
  toggleLineup,
  unknownLineupIds,
} from '../../lib/lineup';
import { runnerDisplayName } from '../../lib/wg';

interface Props {
  eventId: string;
  disciplineId: string;
  run: Run;
  /** Whole team roster incl. substitutes, in roster order. */
  roster: Runner[];
  onSaved: (run: Run) => void;
  onClose: () => void;
}

/** Organizer lineup correction (tick list of the whole roster). */
export default function LineupEditor({ eventId, disciplineId, run, roster, onSaved, onClose }: Props) {
  const [ids, setIds] = useState<string[]>(run.lineup ?? []);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);

  // Follow server updates (e.g. Startlinie confirms) while untouched.
  useEffect(() => {
    if (!dirty) setIds(run.lineup ?? []);
  }, [run.lineup, dirty]);

  const unknown = unknownLineupIds(run.lineup ?? [], roster);
  const changed = lineupChanged(ids, run.lineup ?? []) || unknown.length > 0;

  const save = async () => {
    const body = lineupBody(ids, roster);
    if (
      body.runner_ids.length === 0 &&
      !window.confirm('Besetzung leeren? Der Tagessieger nutzt dann den ganzen Kader.')
    ) {
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      const saved = await apiClient.runs.setLineup(eventId, disciplineId, run.id, body.runner_ids);
      setDirty(false);
      onSaved(saved);
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Speichern fehlgeschlagen');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3 rounded-xl border border-[color:var(--hairline-color)] bg-[color:var(--soft-fill)] p-4">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <span className="label">Besetzung korrigieren · Kader inkl. Ersatz</span>
        <span className="font-mono text-[11px] text-ink-200">
          Bestätigt: {lineupConfirmationText(run)}
        </span>
      </div>
      {roster.length === 0 ? (
        <p className="text-[12px] text-ink-200">
          Für diese Mannschaft sind keine Läufer erfasst (Tab „Mannschaften & Läufer").
        </p>
      ) : (
        <ul className="grid grid-cols-1 gap-1 sm:grid-cols-2">
          {roster.map((r) => {
            const checked = ids.includes(r.id);
            return (
              <li key={r.id}>
                <label
                  className={[
                    'flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-[13px]',
                    checked ? 'bg-status-confirmed/10 text-ink-50' : 'text-ink-200 hover:bg-ink-50/5',
                  ].join(' ')}
                >
                  <input
                    type="checkbox"
                    className="h-4 w-4 accent-[rgb(var(--c-status-confirmed))]"
                    checked={checked}
                    onChange={() => {
                      setDirty(true);
                      setIds((cur) => toggleLineup(cur, r.id));
                    }}
                  />
                  <span>{runnerDisplayName(r)}</span>
                  {r.birth_date && (
                    <span className="ml-auto font-mono text-[11px] text-ink-300">
                      {r.birth_date.slice(0, 4)}
                    </span>
                  )}
                </label>
              </li>
            );
          })}
        </ul>
      )}
      {unknown.length > 0 && (
        <p className="text-[12px] text-status-armed">
          ⚠ {unknown.length} gespeicherte Läufer gehören nicht mehr zum Kader und werden beim
          Speichern entfernt.
        </p>
      )}
      {err && <p className="text-[12px] text-status-unknown">{err}</p>}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="font-mono text-[11px] text-ink-200">
          {lineupBody(ids, roster).runner_ids.length} von {roster.length} ausgewählt
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            className="btn"
            onClick={() => {
              setDirty(false);
              setIds(run.lineup ?? []);
              onClose();
            }}
            disabled={busy}
          >
            Schließen
          </button>
          <button type="button" className="btn btn-primary" onClick={save} disabled={busy || !changed}>
            {busy ? 'Speichere …' : 'Besetzung speichern'}
          </button>
        </div>
      </div>
    </div>
  );
}
