// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import { STATION_KIND_LABELS, type StationKind } from '../../lib/api';

/**
 * One-shot picker shown after the decider page loads. Stores the
 * choice in `localStorage` so a reload (or a tab swap) skips the
 * picker until the operator explicitly chooses "Station ändern".
 *
 * Storage key: `kido.decider.station.<event_id>`. We deliberately
 * scope by event so a second event on the same device starts clean.
 */
export interface StationPickerProps {
  eventId: string | null;
  allowed: StationKind[];
  onPicked: (station: StationKind) => void;
}

const STORAGE_PREFIX = 'kido.decider.station.';

export function storageKey(eventId: string): string {
  return `${STORAGE_PREFIX}${eventId}`;
}

/** Read a persisted station for the given event. Validates the value
 *  against the current `allowed` list so a stale pick from yesterday
 *  doesn't slip through. */
export function loadStation(
  eventId: string | null,
  allowed: StationKind[],
): StationKind | null {
  if (!eventId || typeof localStorage === 'undefined') return null;
  const raw = localStorage.getItem(storageKey(eventId));
  if (!raw) return null;
  return (allowed as readonly string[]).includes(raw) ? (raw as StationKind) : null;
}

export function saveStation(eventId: string, station: StationKind): void {
  if (typeof localStorage === 'undefined') return;
  localStorage.setItem(storageKey(eventId), station);
}

export function clearStation(eventId: string): void {
  if (typeof localStorage === 'undefined') return;
  localStorage.removeItem(storageKey(eventId));
}

export default function StationPicker({
  eventId,
  allowed,
  onPicked,
}: StationPickerProps) {
  const [busy, setBusy] = useState(false);
  // If only a single station is allowed, auto-pick it. (Unusual but
  // possible during a transitional setup phase.)
  useEffect(() => {
    if (allowed.length === 1 && eventId) {
      saveStation(eventId, allowed[0]!);
      onPicked(allowed[0]!);
    }
  }, [allowed, eventId, onPicked]);

  const pick = (s: StationKind) => {
    if (!eventId) return;
    setBusy(true);
    saveStation(eventId, s);
    onPicked(s);
  };

  return (
    <div className="min-h-screen flex flex-col items-center justify-center px-6 py-10">
      <div className="surface w-full max-w-lg p-8 animate-rise">
        <span className="label">Strafrichter-Station wählen</span>
        <p className="mt-2 font-mono text-[12px] text-ink-200">
          Bitte wähle deinen Posten. Die Auswahl bleibt für diese Veranstaltung
          gespeichert; ein Wechsel ist später über den Button „Station ändern"
          möglich.
        </p>
        <div className="mt-6 grid grid-cols-2 gap-3 stagger-rise">
          {allowed.map((s) => (
            <button
              key={s}
              type="button"
              disabled={busy}
              onClick={() => pick(s)}
              className="surface flex flex-col items-center justify-center gap-1 py-7 transition active:scale-[0.97] disabled:opacity-40"
            >
              <span className="font-stencil text-[20px] font-bold text-ink-50 leading-none">
                {STATION_KIND_LABELS[s]}
              </span>
            </button>
          ))}
        </div>
        {allowed.length === 0 && (
          <p className="mt-6 font-mono text-[12px] text-status-armed">
            Aktuell ist keine Disziplin auf der Bahn. Sobald der Organisator
            einen Wettkampf startet, erscheinen hier die zulässigen Stationen.
          </p>
        )}
      </div>
    </div>
  );
}
