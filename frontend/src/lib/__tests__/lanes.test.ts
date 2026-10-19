// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  JUDGE_MODES,
  JUDGE_MODE_LABELS,
  allowedLaneCounts,
  defaultLaneCount,
  effectiveLaneCount,
  isJudgeMode,
  isLaneConflict,
  isLaneCountEditable,
  isValidLaneCount,
  laneConflicts,
  laneCountLabel,
  laneForNewEntry,
  laneOptions,
  maxLanes,
  showsLanePicker,
} from '../lanes';

describe('lane counts per mode', () => {
  it('Gruppenstaffette: 1 or 2, default 2', () => {
    expect(allowedLaneCounts('gruppenstaffette')).toEqual([1, 2]);
    expect(defaultLaneCount('gruppenstaffette')).toBe(2);
    expect(maxLanes('gruppenstaffette')).toBe(2);
    expect(isLaneCountEditable('gruppenstaffette')).toBe(true);
  });

  it('Löschangriff: fixed 4, read-only', () => {
    expect(allowedLaneCounts('loeschangriff')).toEqual([4]);
    expect(defaultLaneCount('loeschangriff')).toBe(4);
    expect(isLaneCountEditable('loeschangriff')).toBe(false);
  });

  it.each([
    ['gruppenstaffette', 1, true],
    ['gruppenstaffette', 2, true],
    ['gruppenstaffette', 3, false],
    ['gruppenstaffette', 4, false],
    ['gruppenstaffette', 0, false],
    ['gruppenstaffette', 1.5, false],
    ['loeschangriff', 4, true],
    ['loeschangriff', 2, false],
    ['loeschangriff', 1, false],
  ] as const)('isValidLaneCount(%s, %s) = %s', (mode, n, ok) => {
    expect(isValidLaneCount(mode, n)).toBe(ok);
  });

  it('isValidLaneCount rejects non-numbers', () => {
    expect(isValidLaneCount('gruppenstaffette', '2')).toBe(false);
    expect(isValidLaneCount('gruppenstaffette', null)).toBe(false);
  });
});

describe('effectiveLaneCount', () => {
  it('uses the stored value when valid', () => {
    expect(effectiveLaneCount({ mode: 'gruppenstaffette', lane_count: 1 })).toBe(1);
  });
  it('defaults legacy rows without lane_count', () => {
    expect(effectiveLaneCount({ mode: 'gruppenstaffette' })).toBe(2);
    expect(effectiveLaneCount({ mode: 'loeschangriff', lane_count: null })).toBe(4);
  });
  it('replaces invalid values with the default', () => {
    expect(effectiveLaneCount({ mode: 'gruppenstaffette', lane_count: 7 })).toBe(2);
    expect(effectiveLaneCount({ mode: 'loeschangriff', lane_count: 2 })).toBe(4);
  });
});

describe('lane picker', () => {
  it('is hidden for one lane', () => {
    expect(showsLanePicker(1)).toBe(false);
    expect(showsLanePicker(2)).toBe(true);
    expect(showsLanePicker(4)).toBe(true);
  });

  it('laneOptions lists 1..n', () => {
    expect(laneOptions(2).map((o) => o.value)).toEqual([1, 2]);
    expect(laneOptions(4).map((o) => o.label)).toEqual(['1', '2', '3', '4']);
    expect(laneOptions(1)).toEqual([{ value: 1, label: '1', conflict: false }]);
    expect(laneOptions(0)).toEqual([]);
  });

  it('laneOptions keeps an out-of-range current value as conflict', () => {
    const opts = laneOptions(1, 2);
    expect(opts).toHaveLength(2);
    expect(opts[1]).toEqual({ value: 2, label: '2 (ungültig)', conflict: true });
  });

  it('laneOptions does not duplicate an in-range current value', () => {
    expect(laneOptions(2, 2)).toHaveLength(2);
    expect(laneOptions(2, null)).toHaveLength(2);
  });

  it('laneForNewEntry fixes lane 1 on single-lane disciplines', () => {
    expect(laneForNewEntry(1, null)).toBe(1);
    expect(laneForNewEntry(1, 2)).toBe(1);
    expect(laneForNewEntry(2, null)).toBeNull();
    expect(laneForNewEntry(2, 2)).toBe(2);
    expect(laneForNewEntry(4, 3)).toBe(3);
  });

  it('laneCountLabel', () => {
    expect(laneCountLabel(1)).toBe('1 Bahn');
    expect(laneCountLabel(2)).toBe('2 Bahnen');
    expect(laneCountLabel(4)).toBe('4 Bahnen');
  });
});

describe('lane conflicts', () => {
  it('flags lanes above the lane count', () => {
    expect(isLaneConflict(2, 1)).toBe(true);
    expect(isLaneConflict(1, 1)).toBe(false);
    expect(isLaneConflict(null, 1)).toBe(false);
    expect(isLaneConflict(undefined, 2)).toBe(false);
  });

  it('laneConflicts returns the team ids', () => {
    expect(
      laneConflicts(
        [
          { team_id: 'a', lane: 1 },
          { team_id: 'b', lane: 2 },
          { team_id: 'c', lane: null },
        ],
        1,
      ),
    ).toEqual(['b']);
    expect(laneConflicts([{ team_id: 'a', lane: 2 }], 2)).toEqual([]);
  });
});

describe('judge mode', () => {
  it('has the two modes with German labels', () => {
    expect(JUDGE_MODES).toEqual(['per_station', 'per_station_and_lane']);
    expect(JUDGE_MODE_LABELS.per_station).toBe('Ein Richter je Station');
    expect(JUDGE_MODE_LABELS.per_station_and_lane).toBe('Ein Richter je Station und Bahn');
  });

  it('isJudgeMode', () => {
    expect(isJudgeMode('per_station')).toBe(true);
    expect(isJudgeMode('per_lane')).toBe(false);
    expect(isJudgeMode(undefined)).toBe(false);
  });
});
