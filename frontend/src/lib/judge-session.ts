// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Persisted judge "login": station + (in `per_station_and_lane` mode
 * with 2 lanes) the judge's lane, per event, in `localStorage`.
 *
 * Storage:
 * - current: `kido.judge.session.<event_id>` → JSON `{ v: 2, station, lane }`
 * - legacy (PR 1 and older): `kido.decider.station.<event_id>` → plain
 *   station key, possibly a pre-2027 key like `knoten`. Migrated on the
 *   first read (written to the new key, old key removed).
 *
 * Reads never throw: broken JSON, unknown stations, out-of-range lanes
 * are dropped and the judge simply picks again.
 */

import type { JudgeMode } from './api';
import {
  normalizeStation,
  stationStorageKey,
  type KeyValueStorage,
  type StationKind,
} from './stations';

export const JUDGE_SESSION_PREFIX = 'kido.judge.session.';
export const JUDGE_SESSION_VERSION = 2;

export function judgeSessionKey(eventId: string): string {
  return `${JUDGE_SESSION_PREFIX}${eventId}`;
}

export interface JudgeSession {
  station: StationKind | null;
  /** `null` = no lane chosen / not needed. */
  lane: number | null;
}

export interface JudgeDisciplineLike {
  lane_count?: number | null;
  judge_mode?: JudgeMode | null;
}

export const EMPTY_SESSION: JudgeSession = { station: null, lane: null };

/** Lane count of the state's discipline (defaults to 1 without one). */
export function disciplineLaneCount(d: JudgeDisciplineLike | null | undefined): number {
  const n = d?.lane_count;
  return typeof n === 'number' && Number.isInteger(n) && n >= 1 ? n : 1;
}

/** A judge must pick a lane only in `per_station_and_lane` mode with
 *  more than one lane. */
export function judgeNeedsLane(d: JudgeDisciplineLike | null | undefined): boolean {
  return d?.judge_mode === 'per_station_and_lane' && disciplineLaneCount(d) > 1;
}

export function isValidLane(lane: unknown, laneCount: number): lane is number {
  return typeof lane === 'number' && Number.isInteger(lane) && lane >= 1 && lane <= laneCount;
}

/** Lanes offered by the lane picker: 1..laneCount. */
export function laneChoices(d: JudgeDisciplineLike | null | undefined): number[] {
  return Array.from({ length: disciplineLaneCount(d) }, (_, i) => i + 1);
}

/** Parses a stored value: JSON session or a bare (legacy) station key. */
export function parseStoredSession(raw: string | null): JudgeSession | null {
  if (raw == null || raw === '') return null;
  const trimmed = raw.trim();
  if (trimmed.startsWith('{')) {
    try {
      const obj = JSON.parse(trimmed) as Record<string, unknown>;
      const station = normalizeStation(obj.station);
      if (!station) return null;
      const lane =
        typeof obj.lane === 'number' && Number.isInteger(obj.lane) && obj.lane >= 1
          ? obj.lane
          : null;
      return { station, lane };
    } catch {
      return null;
    }
  }
  // Older builds may have JSON-stringified the bare key ("\"knoten\"").
  const bare = trimmed.replace(/^"(.*)"$/, '$1');
  const station = normalizeStation(bare);
  return station ? { station, lane: null } : null;
}

export function serializeSession(s: JudgeSession): string {
  return JSON.stringify({ v: JUDGE_SESSION_VERSION, station: s.station, lane: s.lane });
}

function defaultStorage(): KeyValueStorage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

function safeGet(storage: KeyValueStorage, key: string): string | null {
  try {
    return storage.getItem(key);
  } catch {
    return null;
  }
}

function safeSet(storage: KeyValueStorage, key: string, value: string): void {
  try {
    storage.setItem(key, value);
  } catch {
    // quota / private mode → the judge picks again after a reload
  }
}

function safeRemove(storage: KeyValueStorage, key: string): void {
  try {
    storage.removeItem(key);
  } catch {
    // ignore
  }
}

/** Reads the raw stored session (migrating the legacy key in place). */
export function readStoredSession(
  eventId: string,
  storage: KeyValueStorage | null = defaultStorage(),
): JudgeSession | null {
  if (!storage) return null;
  const key = judgeSessionKey(eventId);
  const current = parseStoredSession(safeGet(storage, key));
  if (current) return current;

  const legacyKey = stationStorageKey(eventId);
  const legacyRaw = safeGet(storage, legacyKey);
  if (legacyRaw == null) {
    // broken current value → clean up
    if (safeGet(storage, key) != null) safeRemove(storage, key);
    return null;
  }
  const legacy = parseStoredSession(legacyRaw);
  safeRemove(storage, legacyKey);
  if (!legacy) return null;
  safeSet(storage, key, serializeSession(legacy));
  return legacy;
}

/**
 * Validates a stored session against the current discipline:
 * - station must be in `allowed` (else everything is dropped),
 * - the lane is kept only when the mode needs one and it is in range;
 *   otherwise `lane = null` (the picker asks again if needed).
 */
export function validateSession(
  stored: JudgeSession | null,
  allowed: readonly StationKind[],
  discipline: JudgeDisciplineLike | null | undefined,
): JudgeSession {
  if (!stored?.station || !allowed.includes(stored.station)) return { ...EMPTY_SESSION };
  const needsLane = judgeNeedsLane(discipline);
  const lane =
    needsLane && isValidLane(stored.lane, disciplineLaneCount(discipline)) ? stored.lane : null;
  return { station: stored.station, lane };
}

/** Station + lane (if needed) chosen → the judge can work. */
export function isSessionComplete(
  s: JudgeSession,
  discipline: JudgeDisciplineLike | null | undefined,
): boolean {
  if (!s.station) return false;
  if (!judgeNeedsLane(discipline)) return true;
  return isValidLane(s.lane, disciplineLaneCount(discipline));
}

/** Which login step to show next. */
export type LoginStep = 'station' | 'lane' | 'done';

export function nextLoginStep(
  s: JudgeSession,
  discipline: JudgeDisciplineLike | null | undefined,
): LoginStep {
  if (!s.station) return 'station';
  return isSessionComplete(s, discipline) ? 'done' : 'lane';
}

/** Load + validate in one go. */
export function loadJudgeSession(
  eventId: string | null,
  allowed: readonly StationKind[],
  discipline: JudgeDisciplineLike | null | undefined,
  storage: KeyValueStorage | null = defaultStorage(),
): JudgeSession {
  if (!eventId) return { ...EMPTY_SESSION };
  return validateSession(readStoredSession(eventId, storage), allowed, discipline);
}

export function saveJudgeSession(
  eventId: string,
  session: JudgeSession,
  storage: KeyValueStorage | null = defaultStorage(),
): void {
  if (!storage) return;
  if (!session.station) {
    safeRemove(storage, judgeSessionKey(eventId));
    return;
  }
  safeSet(storage, judgeSessionKey(eventId), serializeSession(session));
}

export function clearJudgeSession(
  eventId: string,
  storage: KeyValueStorage | null = defaultStorage(),
): void {
  if (!storage) return;
  safeRemove(storage, judgeSessionKey(eventId));
  safeRemove(storage, stationStorageKey(eventId));
}

/** Header text, e.g. `"Knotengestell · Bahn 2"`. */
export function sessionLabel(
  s: JudgeSession,
  stationLabel: (st: StationKind) => string,
  discipline: JudgeDisciplineLike | null | undefined,
): string {
  if (!s.station) return '—';
  const base = stationLabel(s.station);
  return judgeNeedsLane(discipline) && s.lane != null ? `${base} · Bahn ${s.lane}` : base;
}
