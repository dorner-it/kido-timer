// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useMemo, useState } from 'react';

import type { Run, Runner, Team } from '../../../lib/api';
import { currentRunLabel } from '../../../lib/decider';
import {
  initialSelection,
  isLineupConfirmed,
  isLineupDirty,
  lineupCountLabel,
  lineupRequestBody,
  lineupSignature,
  lineupSizeHint,
  lineupStatusText,
  runnerName,
  sortRoster,
  toggleRunner,
  type LineupRequestBody,
} from '../../../lib/lineup';
import { BTN_PRIMARY, CARD } from './ui';

export interface LineupRun {
  run: Run;
  team: Team;
  runners: Runner[];
}

interface Draft {
  selection: Set<string>;
  /** `lineupSignature` the draft was seeded from. */
  seed: string;
  touched: boolean;
}

/**
 * "Besetzung" (Startlinie only): one tick list per current run. Drafts
 * are kept per run id while the tab is switched; untouched drafts
 * follow server changes (e.g. the organizer corrected the lineup).
 */
export function LineupPanel({
  runs,
  onConfirm,
}: {
  runs: LineupRun[];
  onConfirm: (runId: string, body: LineupRequestBody) => Promise<void>;
}) {
  const [activeRunId, setActiveRunId] = useState<string | null>(runs[0]?.run.id ?? null);
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  const active = runs.find((r) => r.run.id === activeRunId) ?? runs[0] ?? null;

  useEffect(() => {
    if (!runs.some((r) => r.run.id === activeRunId)) setActiveRunId(runs[0]?.run.id ?? null);
  }, [runs, activeRunId]);

  // Seed / re-seed drafts from the server state.
  useEffect(() => {
    setDrafts((prev) => {
      let changed = false;
      const next: Record<string, Draft> = {};
      for (const r of runs) {
        const sig = lineupSignature(r.run);
        const old = prev[r.run.id];
        if (old && (old.touched || old.seed === sig)) {
          next[r.run.id] = old;
        } else {
          next[r.run.id] = { selection: initialSelection(r.run, r.runners), seed: sig, touched: false };
          changed = true;
        }
      }
      if (Object.keys(prev).length !== runs.length) changed = true;
      return changed ? next : prev;
    });
  }, [runs]);

  const roster = useMemo(() => (active ? sortRoster(active.runners) : []), [active]);

  if (runs.length === 0) {
    return <p className="text-[16px] text-ink-200">Kein Lauf aktiv – keine Besetzung zu prüfen.</p>;
  }
  if (!active) return null;

  const draft = drafts[active.run.id];
  const selection = draft?.selection ?? initialSelection(active.run, active.runners);
  const hint = lineupSizeHint(selection.size);
  const confirmed = isLineupConfirmed(active.run);
  const dirty = isLineupDirty(selection, active.run, active.runners);

  const toggle = (id: string) => {
    setMessage(null);
    setDrafts((d) => ({
      ...d,
      [active.run.id]: {
        selection: toggleRunner(selection, id),
        seed: d[active.run.id]?.seed ?? lineupSignature(active.run),
        touched: true,
      },
    }));
  };

  const confirm = async () => {
    setSaving(true);
    setMessage(null);
    try {
      await onConfirm(active.run.id, lineupRequestBody(selection, active.runners));
      setDrafts((d) => {
        const { [active.run.id]: _drop, ...rest } = d;
        return rest;
      });
      setMessage({ ok: true, text: 'Besetzung bestätigt.' });
    } catch (e) {
      setMessage({
        ok: false,
        text: `Speichern fehlgeschlagen: ${e instanceof Error ? e.message : 'Netzwerkfehler'}`,
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex flex-col gap-4">
      {runs.length > 1 && (
        <div className="grid grid-cols-2 gap-2" role="tablist" aria-label="Lauf wählen">
          {runs.map((r) => {
            const on = r.run.id === active.run.id;
            const ok = isLineupConfirmed(r.run);
            return (
              <button
                key={r.run.id}
                type="button"
                role="tab"
                aria-selected={on}
                onClick={() => {
                  setActiveRunId(r.run.id);
                  setMessage(null);
                }}
                className={`min-h-[64px] rounded-xl border-2 px-3 py-2 text-left text-[15px] font-semibold leading-tight ${
                  on ? 'border-ink-50 bg-ink-50 text-white' : 'border-ink-50 bg-white text-ink-50'
                }`}
              >
                {currentRunLabel(r)}
                <span className={`mt-1 block text-[12px] font-bold ${ok ? '' : on ? 'text-amber-300' : 'text-amber-800'}`}>
                  {ok ? '✓ bestätigt' : 'offen'}
                </span>
              </button>
            );
          })}
        </div>
      )}

      <div className={`${CARD} p-4`}>
        <div className="flex items-baseline justify-between gap-3">
          <div className="flex flex-col">
            <span className="text-[13px] font-bold uppercase tracking-wide text-ink-200">
              Besetzung · Lauf {active.run.run_number}
            </span>
            <span className="text-[20px] font-bold leading-tight text-ink-50">{currentRunLabel(active)}</span>
          </div>
          <span className="shrink-0 text-[15px] font-bold text-ink-50 tnum">
            {lineupCountLabel(selection.size, roster.length)}
          </span>
        </div>
        <p
          className={`mt-2 text-[14px] font-semibold ${confirmed ? 'text-green-800' : 'text-amber-800'}`}
          role="status"
        >
          {lineupStatusText(active.run)}
          {confirmed && dirty ? ' – geändert, noch nicht gespeichert' : ''}
        </p>

        {roster.length === 0 ? (
          <p className="mt-4 text-[15px] text-ink-200">
            Für diese Mannschaft ist kein Kader hinterlegt. Bitte die Wettkampfleitung informieren.
          </p>
        ) : (
          <ul className="mt-4 flex flex-col gap-2">
            {roster.map((r) => {
              const on = selection.has(r.id);
              return (
                <li key={r.id}>
                  <button
                    type="button"
                    role="checkbox"
                    aria-checked={on}
                    onClick={() => toggle(r.id)}
                    className={`flex min-h-[60px] w-full items-center gap-3 rounded-xl border-2 px-3 py-2 text-left ${
                      on ? 'border-ink-50 bg-ink-700' : 'border-ink-400 bg-white'
                    }`}
                  >
                    <span
                      className={`grid h-9 w-9 shrink-0 place-items-center rounded-md border-2 text-[22px] font-black leading-none ${
                        on ? 'border-ink-50 bg-ink-50 text-white' : 'border-ink-300 bg-white text-transparent'
                      }`}
                      aria-hidden="true"
                    >
                      ✕
                    </span>
                    <span className="w-8 shrink-0 text-[14px] font-bold text-ink-200 tnum">
                      {r.position}
                    </span>
                    <span className={`text-[17px] font-semibold ${on ? 'text-ink-50' : 'text-ink-200'}`}>
                      {runnerName(r)}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}

        {hint.message && roster.length > 0 && (
          <p className="mt-3 rounded-lg border-2 border-amber-600 bg-amber-50 px-3 py-2 text-[14px] font-semibold text-amber-900">
            {hint.message}
          </p>
        )}

        {message && (
          <p
            className={`mt-3 text-[15px] font-semibold ${message.ok ? 'text-green-800' : 'text-red-800'}`}
            role="alert"
          >
            {message.text}
          </p>
        )}

        <div className="mt-4">
          <button
            type="button"
            className={BTN_PRIMARY}
            disabled={saving || roster.length === 0 || (confirmed && !dirty)}
            onClick={confirm}
          >
            {saving
              ? 'Speichere …'
              : confirmed && !dirty
                ? '✓ Besetzung bestätigt'
                : confirmed
                  ? 'Änderung bestätigen'
                  : 'Besetzung bestätigen'}
          </button>
        </div>
      </div>
    </div>
  );
}
