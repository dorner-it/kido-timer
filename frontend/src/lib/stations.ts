// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Wertungsrichter-Stationen (judging posts), mirrors `kido_core::StationKind`.
 *
 * Canonical wire keys follow the FBL Wertungsbögen (2027). The backend
 * always emits canonical keys, but pre-2027 rows / old clients / values
 * persisted in `localStorage` may still carry the legacy keys, so every
 * read goes through {@link normalizeStation}.
 */

import type { TimerMode } from './api';

export type StationKind =
  | 'startlinie'
  | 'verteilerlinie'
  | 'strahlrohrlinie'
  | 'knotengestell'
  | 'angriffslinie'
  | 'podest';

/** Canonical order (= `StationKind::ALL`). */
export const STATION_KINDS: readonly StationKind[] = [
  'startlinie',
  'verteilerlinie',
  'strahlrohrlinie',
  'knotengestell',
  'angriffslinie',
  'podest',
];

/** Pre-2027 keys → canonical keys. `strahlrohrlinie` and `podest` keep
 *  their names and are therefore not listed. */
export const LEGACY_STATION_KEYS: Readonly<Record<string, StationKind>> = {
  start: 'startlinie',
  verteiler: 'verteilerlinie',
  strahlrohr: 'strahlrohrlinie',
  knoten: 'knotengestell',
  kuebelspritze: 'angriffslinie',
};

export const STATION_KIND_LABELS: Record<StationKind, string> = {
  startlinie: 'Startlinie/Zeitnehmer',
  verteilerlinie: 'Verteilerlinie',
  strahlrohrlinie: 'Strahlrohrlinie',
  knotengestell: 'Knotengestell',
  angriffslinie: 'Angriffslinie',
  podest: 'Podest/Wasserbehälter',
};

/** Stations staffed per discipline, in canonical order. */
export const STATIONS_BY_MODE: Record<TimerMode, readonly StationKind[]> = {
  gruppenstaffette: [
    'startlinie',
    'verteilerlinie',
    'strahlrohrlinie',
    'knotengestell',
    'angriffslinie',
  ],
  loeschangriff: ['podest', 'angriffslinie'],
};

export function isStationKind(value: unknown): value is StationKind {
  return typeof value === 'string' && (STATION_KINDS as readonly string[]).includes(value);
}

/** Canonical key for a canonical or legacy key; `null` for anything else. */
export function normalizeStation(raw: unknown): StationKind | null {
  if (typeof raw !== 'string') return null;
  const key = raw.trim().toLowerCase();
  if (isStationKind(key)) return key;
  return LEGACY_STATION_KEYS[key] ?? null;
}

/** Normalizes a list, drops unknown keys and duplicates, keeps order. */
export function normalizeStations(raw: unknown): StationKind[] {
  if (!Array.isArray(raw)) return [];
  const out: StationKind[] = [];
  for (const r of raw) {
    const s = normalizeStation(r);
    if (s && !out.includes(s)) out.push(s);
  }
  return out;
}

export function stationsForMode(mode: TimerMode): StationKind[] {
  return [...STATIONS_BY_MODE[mode]];
}

export function isStationAllowedForMode(station: StationKind, mode: TimerMode): boolean {
  return STATIONS_BY_MODE[mode].includes(station);
}

/**
 * Stations the judge app offers: what the backend reports
 * (`allowed_stations`, normalized), filtered by the discipline mode.
 * Falls back to the mode's station list when the backend sends none.
 */
export function pickerStations(
  allowedFromServer: unknown,
  mode: TimerMode | null | undefined,
): StationKind[] {
  const fromServer = normalizeStations(allowedFromServer);
  if (!mode) return fromServer;
  const byMode = stationsForMode(mode);
  if (fromServer.length === 0) return byMode;
  return byMode.filter((s) => fromServer.includes(s));
}

/** Display label; tolerates legacy keys and `null`. */
export function stationLabel(raw: unknown, fallback = '—'): string {
  const s = normalizeStation(raw);
  if (s) return STATION_KIND_LABELS[s];
  return typeof raw === 'string' && raw ? raw : fallback;
}

/** Only the Startlinie judge records the lineup (PR 2). */
export function stationRecordsLineup(station: StationKind | null): boolean {
  return station === 'startlinie';
}

// ===== persisted judge station (localStorage) =====

const STORAGE_PREFIX = 'kido.decider.station.';

export function stationStorageKey(eventId: string): string {
  return `${STORAGE_PREFIX}${eventId}`;
}

/** Minimal storage surface so tests can inject a fake. */
export interface KeyValueStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

function defaultStorage(): KeyValueStorage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

/**
 * Read the persisted station for an event. Legacy keys (e.g. `knoten`)
 * are migrated in place to the canonical key. Returns `null` when
 * nothing is stored or the stored station is not in `allowed` (a stale
 * pick from another discipline must not slip through).
 */
export function loadStoredStation(
  eventId: string | null,
  allowed: readonly StationKind[],
  storage: KeyValueStorage | null = defaultStorage(),
): StationKind | null {
  if (!eventId || !storage) return null;
  const key = stationStorageKey(eventId);
  const raw = storage.getItem(key);
  if (!raw) return null;
  const station = normalizeStation(raw);
  if (!station) {
    storage.removeItem(key);
    return null;
  }
  if (station !== raw) storage.setItem(key, station);
  return allowed.includes(station) ? station : null;
}

export function saveStoredStation(
  eventId: string,
  station: StationKind,
  storage: KeyValueStorage | null = defaultStorage(),
): void {
  storage?.setItem(stationStorageKey(eventId), station);
}

export function clearStoredStation(
  eventId: string,
  storage: KeyValueStorage | null = defaultStorage(),
): void {
  storage?.removeItem(stationStorageKey(eventId));
}
