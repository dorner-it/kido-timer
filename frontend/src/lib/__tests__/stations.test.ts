// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  LEGACY_STATION_KEYS,
  STATION_KINDS,
  STATION_KIND_LABELS,
  clearStoredStation,
  isStationAllowedForMode,
  isStationKind,
  loadStoredStation,
  normalizeStation,
  normalizeStations,
  pickerStations,
  saveStoredStation,
  stationLabel,
  stationRecordsLineup,
  stationStorageKey,
  stationsForMode,
  type KeyValueStorage,
} from '../stations';

function memoryStorage(initial: Record<string, string> = {}): KeyValueStorage & {
  data: Record<string, string>;
} {
  const data = { ...initial };
  return {
    data,
    getItem: (k) => (k in data ? data[k]! : null),
    setItem: (k, v) => {
      data[k] = v;
    },
    removeItem: (k) => {
      delete data[k];
    },
  };
}

describe('normalizeStation', () => {
  it.each([
    ['start', 'startlinie'],
    ['verteiler', 'verteilerlinie'],
    ['strahlrohr', 'strahlrohrlinie'],
    ['knoten', 'knotengestell'],
    ['kuebelspritze', 'angriffslinie'],
  ])('maps legacy key %s → %s', (legacy, canonical) => {
    expect(normalizeStation(legacy)).toBe(canonical);
  });

  it.each(STATION_KINDS.map((s) => [s]))('keeps canonical key %s', (s) => {
    expect(normalizeStation(s)).toBe(s);
  });

  it('keeps strahlrohrlinie and podest (no legacy alias)', () => {
    expect(normalizeStation('strahlrohrlinie')).toBe('strahlrohrlinie');
    expect(normalizeStation('podest')).toBe('podest');
  });

  it('is tolerant to case and whitespace', () => {
    expect(normalizeStation(' Knoten ')).toBe('knotengestell');
    expect(normalizeStation('STARTLINIE')).toBe('startlinie');
  });

  it('returns null for unknown / non-string values', () => {
    expect(normalizeStation('wasserwerfer')).toBeNull();
    expect(normalizeStation('')).toBeNull();
    expect(normalizeStation(null)).toBeNull();
    expect(normalizeStation(undefined)).toBeNull();
    expect(normalizeStation(3)).toBeNull();
  });

  it('legacy table covers exactly the five aliases', () => {
    expect(Object.keys(LEGACY_STATION_KEYS).sort()).toEqual(
      ['knoten', 'kuebelspritze', 'start', 'strahlrohr', 'verteiler'].sort(),
    );
  });
});

describe('normalizeStations', () => {
  it('maps, dedupes and drops unknown keys keeping order', () => {
    expect(normalizeStations(['knoten', 'start', 'knotengestell', 'foo', 'podest'])).toEqual([
      'knotengestell',
      'startlinie',
      'podest',
    ]);
  });

  it('returns [] for non-arrays', () => {
    expect(normalizeStations(undefined)).toEqual([]);
    expect(normalizeStations('start')).toEqual([]);
  });
});

describe('labels', () => {
  it('has the Wertungsbogen labels', () => {
    expect(STATION_KIND_LABELS).toEqual({
      startlinie: 'Startlinie/Zeitnehmer',
      verteilerlinie: 'Verteilerlinie',
      strahlrohrlinie: 'Strahlrohrlinie',
      knotengestell: 'Knotengestell',
      angriffslinie: 'Angriffslinie',
      podest: 'Podest/Wasserbehälter',
    });
  });

  it('stationLabel resolves legacy keys and falls back', () => {
    expect(stationLabel('kuebelspritze')).toBe('Angriffslinie');
    expect(stationLabel(null)).toBe('—');
    expect(stationLabel(null, 'ohne Station')).toBe('ohne Station');
    expect(stationLabel('mystery')).toBe('mystery');
  });

  it('isStationKind only accepts canonical keys', () => {
    expect(isStationKind('knotengestell')).toBe(true);
    expect(isStationKind('knoten')).toBe(false);
  });
});

describe('stations per mode', () => {
  it('Gruppenstaffette has the five Wertungsbogen stations in order', () => {
    expect(stationsForMode('gruppenstaffette')).toEqual([
      'startlinie',
      'verteilerlinie',
      'strahlrohrlinie',
      'knotengestell',
      'angriffslinie',
    ]);
  });

  it('Löschangriff has Podest + Angriffslinie', () => {
    expect(stationsForMode('loeschangriff')).toEqual(['podest', 'angriffslinie']);
  });

  it('stationsForMode returns a copy', () => {
    const a = stationsForMode('loeschangriff');
    a.push('startlinie');
    expect(stationsForMode('loeschangriff')).toHaveLength(2);
  });

  it('isStationAllowedForMode', () => {
    expect(isStationAllowedForMode('podest', 'gruppenstaffette')).toBe(false);
    expect(isStationAllowedForMode('angriffslinie', 'loeschangriff')).toBe(true);
    expect(isStationAllowedForMode('knotengestell', 'loeschangriff')).toBe(false);
  });

  it('only the Startlinie records the lineup', () => {
    expect(stationRecordsLineup('startlinie')).toBe(true);
    expect(stationRecordsLineup('knotengestell')).toBe(false);
    expect(stationRecordsLineup(null)).toBe(false);
  });
});

describe('pickerStations', () => {
  it('normalizes legacy server keys and orders by mode', () => {
    expect(
      pickerStations(['kuebelspritze', 'knoten', 'start'], 'gruppenstaffette'),
    ).toEqual(['startlinie', 'knotengestell', 'angriffslinie']);
  });

  it('filters stations not valid for the mode', () => {
    expect(pickerStations(['podest', 'strahlrohrlinie', 'angriffslinie'], 'loeschangriff')).toEqual(
      ['podest', 'angriffslinie'],
    );
  });

  it('falls back to the mode list when the server sends nothing', () => {
    expect(pickerStations(undefined, 'loeschangriff')).toEqual(['podest', 'angriffslinie']);
    expect(pickerStations([], 'gruppenstaffette')).toHaveLength(5);
  });

  it('without a mode returns the normalized server list', () => {
    expect(pickerStations(['start'], null)).toEqual(['startlinie']);
    expect(pickerStations(undefined, null)).toEqual([]);
  });
});

describe('persisted station (localStorage)', () => {
  const allowed = stationsForMode('gruppenstaffette');

  it('uses an event-scoped key', () => {
    expect(stationStorageKey('ev1')).toBe('kido.decider.station.ev1');
  });

  it('migrates a stored legacy key to the new key', () => {
    const st = memoryStorage({ 'kido.decider.station.ev1': 'knoten' });
    expect(loadStoredStation('ev1', allowed, st)).toBe('knotengestell');
    expect(st.data['kido.decider.station.ev1']).toBe('knotengestell');
  });

  it('migrates kuebelspritze → angriffslinie even if not allowed right now', () => {
    const st = memoryStorage({ 'kido.decider.station.ev1': 'kuebelspritze' });
    expect(loadStoredStation('ev1', ['podest'], st)).toBeNull();
    expect(st.data['kido.decider.station.ev1']).toBe('angriffslinie');
  });

  it('returns canonical values unchanged', () => {
    const st = memoryStorage({ 'kido.decider.station.ev1': 'verteilerlinie' });
    expect(loadStoredStation('ev1', allowed, st)).toBe('verteilerlinie');
  });

  it('rejects a station not allowed for the current discipline', () => {
    const st = memoryStorage({ 'kido.decider.station.ev1': 'podest' });
    expect(loadStoredStation('ev1', allowed, st)).toBeNull();
  });

  it('removes garbage values', () => {
    const st = memoryStorage({ 'kido.decider.station.ev1': 'garbage' });
    expect(loadStoredStation('ev1', allowed, st)).toBeNull();
    expect(st.data).toEqual({});
  });

  it('handles missing event id / storage / value', () => {
    expect(loadStoredStation(null, allowed, memoryStorage())).toBeNull();
    expect(loadStoredStation('ev1', allowed, null)).toBeNull();
    expect(loadStoredStation('ev1', allowed, memoryStorage())).toBeNull();
  });

  it('save + clear round trip', () => {
    const st = memoryStorage();
    saveStoredStation('ev2', 'startlinie', st);
    expect(loadStoredStation('ev2', allowed, st)).toBe('startlinie');
    clearStoredStation('ev2', st);
    expect(loadStoredStation('ev2', allowed, st)).toBeNull();
  });
});
