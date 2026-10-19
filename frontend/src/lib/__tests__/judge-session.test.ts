// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  clearJudgeSession,
  disciplineLaneCount,
  isSessionComplete,
  isValidLane,
  judgeNeedsLane,
  judgeSessionKey,
  laneChoices,
  loadJudgeSession,
  nextLoginStep,
  parseStoredSession,
  readStoredSession,
  saveJudgeSession,
  serializeSession,
  sessionLabel,
  validateSession,
} from '../judge-session';
import { STATION_KIND_LABELS, stationStorageKey, type KeyValueStorage, type StationKind } from '../stations';

class FakeStorage implements KeyValueStorage {
  map = new Map<string, string>();
  getItem(k: string) {
    return this.map.has(k) ? this.map.get(k)! : null;
  }
  setItem(k: string, v: string) {
    this.map.set(k, v);
  }
  removeItem(k: string) {
    this.map.delete(k);
  }
}

class ThrowingStorage implements KeyValueStorage {
  getItem(): string | null {
    throw new Error('SecurityError');
  }
  setItem(): void {
    throw new Error('QuotaExceeded');
  }
  removeItem(): void {
    throw new Error('SecurityError');
  }
}

const GS: StationKind[] = ['startlinie', 'verteilerlinie', 'strahlrohrlinie', 'knotengestell', 'angriffslinie'];
const perStation2 = { lane_count: 2, judge_mode: 'per_station' as const };
const perLane2 = { lane_count: 2, judge_mode: 'per_station_and_lane' as const };
const perLane1 = { lane_count: 1, judge_mode: 'per_station_and_lane' as const };

describe('lane need', () => {
  it.each([
    [perStation2, false],
    [perLane2, true],
    [perLane1, false],
    [{ lane_count: 1, judge_mode: 'per_station' as const }, false],
    [null, false],
    [{}, false],
  ])('%j → %s', (d, expected) => {
    expect(judgeNeedsLane(d)).toBe(expected);
  });

  it('disciplineLaneCount defaults to 1', () => {
    expect(disciplineLaneCount(null)).toBe(1);
    expect(disciplineLaneCount({ lane_count: 0 })).toBe(1);
    expect(disciplineLaneCount({ lane_count: 1.5 })).toBe(1);
    expect(disciplineLaneCount({ lane_count: 2 })).toBe(2);
  });

  it('laneChoices', () => {
    expect(laneChoices(perLane2)).toEqual([1, 2]);
    expect(laneChoices(null)).toEqual([1]);
  });

  it('isValidLane', () => {
    expect(isValidLane(1, 2)).toBe(true);
    expect(isValidLane(2, 2)).toBe(true);
    expect(isValidLane(3, 2)).toBe(false);
    expect(isValidLane(0, 2)).toBe(false);
    expect(isValidLane(null, 2)).toBe(false);
    expect(isValidLane('1', 2)).toBe(false);
  });
});

describe('parseStoredSession', () => {
  it('JSON session', () => {
    expect(parseStoredSession('{"v":2,"station":"knotengestell","lane":2}')).toEqual({
      station: 'knotengestell',
      lane: 2,
    });
  });

  it('JSON with legacy station key is normalized', () => {
    expect(parseStoredSession('{"station":"kuebelspritze","lane":1}')).toEqual({
      station: 'angriffslinie',
      lane: 1,
    });
  });

  it('bare canonical key (PR 1 format)', () => {
    expect(parseStoredSession('startlinie')).toEqual({ station: 'startlinie', lane: null });
  });

  it('bare legacy key (pre-2027)', () => {
    expect(parseStoredSession('knoten')).toEqual({ station: 'knotengestell', lane: null });
    expect(parseStoredSession('start')).toEqual({ station: 'startlinie', lane: null });
  });

  it('quoted bare key', () => {
    expect(parseStoredSession('"verteiler"')).toEqual({ station: 'verteilerlinie', lane: null });
  });

  it('invalid lanes become null', () => {
    expect(parseStoredSession('{"station":"startlinie","lane":0}')?.lane).toBeNull();
    expect(parseStoredSession('{"station":"startlinie","lane":"2"}')?.lane).toBeNull();
    expect(parseStoredSession('{"station":"startlinie","lane":1.5}')?.lane).toBeNull();
  });

  it('garbage → null', () => {
    expect(parseStoredSession(null)).toBeNull();
    expect(parseStoredSession('')).toBeNull();
    expect(parseStoredSession('{broken')).toBeNull();
    expect(parseStoredSession('{"station":"mars"}')).toBeNull();
    expect(parseStoredSession('mars')).toBeNull();
  });

  it('serialize roundtrip', () => {
    const s = { station: 'strahlrohrlinie' as const, lane: 2 };
    expect(parseStoredSession(serializeSession(s))).toEqual(s);
    expect(JSON.parse(serializeSession(s)).v).toBe(2);
  });
});

describe('readStoredSession migration', () => {
  it('migrates the PR 1 key (bare station) to the new key', () => {
    const st = new FakeStorage();
    st.setItem(stationStorageKey('e1'), 'knotengestell');
    expect(readStoredSession('e1', st)).toEqual({ station: 'knotengestell', lane: null });
    expect(st.getItem(stationStorageKey('e1'))).toBeNull();
    expect(JSON.parse(st.getItem(judgeSessionKey('e1'))!)).toEqual({
      v: 2,
      station: 'knotengestell',
      lane: null,
    });
  });

  it('migrates a pre-2027 legacy key', () => {
    const st = new FakeStorage();
    st.setItem(stationStorageKey('e1'), 'kuebelspritze');
    expect(readStoredSession('e1', st)?.station).toBe('angriffslinie');
  });

  it('drops an unknown legacy value', () => {
    const st = new FakeStorage();
    st.setItem(stationStorageKey('e1'), 'mars');
    expect(readStoredSession('e1', st)).toBeNull();
    expect(st.getItem(stationStorageKey('e1'))).toBeNull();
    expect(st.getItem(judgeSessionKey('e1'))).toBeNull();
  });

  it('new key wins over legacy key', () => {
    const st = new FakeStorage();
    st.setItem(judgeSessionKey('e1'), serializeSession({ station: 'startlinie', lane: 1 }));
    st.setItem(stationStorageKey('e1'), 'knoten');
    expect(readStoredSession('e1', st)).toEqual({ station: 'startlinie', lane: 1 });
  });

  it('broken new value without legacy → cleaned up', () => {
    const st = new FakeStorage();
    st.setItem(judgeSessionKey('e1'), '{oops');
    expect(readStoredSession('e1', st)).toBeNull();
    expect(st.getItem(judgeSessionKey('e1'))).toBeNull();
  });

  it('scoped per event', () => {
    const st = new FakeStorage();
    saveJudgeSession('e1', { station: 'startlinie', lane: null }, st);
    expect(readStoredSession('e2', st)).toBeNull();
  });

  it('null storage / throwing storage never throw', () => {
    expect(readStoredSession('e1', null)).toBeNull();
    expect(readStoredSession('e1', new ThrowingStorage())).toBeNull();
    expect(() => saveJudgeSession('e1', { station: 'startlinie', lane: 1 }, new ThrowingStorage())).not.toThrow();
    expect(() => clearJudgeSession('e1', new ThrowingStorage())).not.toThrow();
  });
});

describe('validateSession', () => {
  it('station not allowed → empty', () => {
    expect(validateSession({ station: 'podest', lane: null }, GS, perStation2)).toEqual({
      station: null,
      lane: null,
    });
  });

  it('per_station drops a stored lane', () => {
    expect(validateSession({ station: 'startlinie', lane: 2 }, GS, perStation2)).toEqual({
      station: 'startlinie',
      lane: null,
    });
  });

  it('per_station_and_lane keeps a valid lane', () => {
    expect(validateSession({ station: 'startlinie', lane: 2 }, GS, perLane2).lane).toBe(2);
  });

  it('per_station_and_lane drops an out-of-range lane (lane count reduced)', () => {
    expect(validateSession({ station: 'startlinie', lane: 3 }, GS, perLane2)).toEqual({
      station: 'startlinie',
      lane: null,
    });
  });

  it('lane_count 1 never needs a lane', () => {
    expect(validateSession({ station: 'startlinie', lane: 1 }, GS, perLane1).lane).toBeNull();
  });

  it('null stored → empty', () => {
    expect(validateSession(null, GS, perLane2)).toEqual({ station: null, lane: null });
  });
});

describe('login steps', () => {
  it.each([
    [{ station: null, lane: null }, perStation2, 'station'],
    [{ station: 'startlinie' as const, lane: null }, perStation2, 'done'],
    [{ station: 'startlinie' as const, lane: null }, perLane2, 'lane'],
    [{ station: 'startlinie' as const, lane: 1 }, perLane2, 'done'],
    [{ station: 'startlinie' as const, lane: 5 }, perLane2, 'lane'],
    [{ station: 'startlinie' as const, lane: null }, perLane1, 'done'],
  ])('%j / %j → %s', (s, d, step) => {
    expect(nextLoginStep(s, d)).toBe(step);
    expect(isSessionComplete(s, d)).toBe(step === 'done');
  });
});

describe('load / save / clear', () => {
  it('save + load roundtrip with lane', () => {
    const st = new FakeStorage();
    saveJudgeSession('e1', { station: 'knotengestell', lane: 2 }, st);
    expect(loadJudgeSession('e1', GS, perLane2, st)).toEqual({ station: 'knotengestell', lane: 2 });
  });

  it('load validates against the discipline', () => {
    const st = new FakeStorage();
    saveJudgeSession('e1', { station: 'knotengestell', lane: 2 }, st);
    expect(loadJudgeSession('e1', GS, perStation2, st)).toEqual({ station: 'knotengestell', lane: null });
    expect(loadJudgeSession('e1', ['podest', 'angriffslinie'], perStation2, st).station).toBeNull();
  });

  it('load without event id → empty', () => {
    expect(loadJudgeSession(null, GS, perLane2, new FakeStorage())).toEqual({ station: null, lane: null });
  });

  it('saving an empty session removes the key', () => {
    const st = new FakeStorage();
    saveJudgeSession('e1', { station: 'startlinie', lane: null }, st);
    saveJudgeSession('e1', { station: null, lane: null }, st);
    expect(st.getItem(judgeSessionKey('e1'))).toBeNull();
  });

  it('clear removes new and legacy keys', () => {
    const st = new FakeStorage();
    saveJudgeSession('e1', { station: 'startlinie', lane: 1 }, st);
    st.setItem(stationStorageKey('e1'), 'knoten');
    clearJudgeSession('e1', st);
    expect(st.map.size).toBe(0);
  });
});

describe('sessionLabel', () => {
  const label = (s: StationKind) => STATION_KIND_LABELS[s];
  it('with lane only when lane-bound', () => {
    expect(sessionLabel({ station: 'knotengestell', lane: 2 }, label, perLane2)).toBe('Knotengestell · Bahn 2');
    expect(sessionLabel({ station: 'knotengestell', lane: 2 }, label, perStation2)).toBe('Knotengestell');
    expect(sessionLabel({ station: null, lane: null }, label, perLane2)).toBe('—');
  });
});
