// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import {
  STATION_KIND_LABELS,
  clearStoredStation,
  loadStoredStation,
  saveStoredStation,
  stationStorageKey,
  type StationKind,
} from '../../lib/stations';

/**
 * One-shot picker shown after the decider page loads. Stores the
 * choice in `localStorage` so a reload (or a tab swap) skips the
 * picker until the operator explicitly chooses "Station ändern".
 *
 * Storage key: `kido.decider.station.<event_id>`. We deliberately
 * scope by event so a second event on the same device starts clean.
 * Pre-2027 keys (e.g. `knoten`) are migrated to the new keys on load.
 */
export interface StationPickerProps {
  eventId: string | null;
  allowed: StationKind[];
  onPicked: (station: StationKind) => void;
}

export const storageKey = stationStorageKey;

/** Read a persisted station for the given event (legacy keys are
 *  migrated) and validate it against the current `allowed` list so a
 *  stale pick from yesterday doesn't slip through. */
export function loadStation(eventId: string | null, allowed: StationKind[]): StationKind | null {
  return loadStoredStation(eventId, allowed);
}

export function saveStation(eventId: string, station: StationKind): void {
  saveStoredStation(eventId, station);
}

export function clearStation(eventId: string): void {
  clearStoredStation(eventId);
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
        <span className="label">Wertungsrichter-Station wählen</span>
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
              className="surface flex flex-col items-center justify-center gap-1 px-2 py-7 transition active:scale-[0.97] disabled:opacity-40"
            >
              <span className="text-center font-stencil text-[17px] font-bold text-ink-50 leading-tight">
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
