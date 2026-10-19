// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import type { JudgeMode } from '../api';
import {
  canEnterErrors,
  isLaneBoundJudge,
  resolveSubmitTarget,
  visibleRuns,
  type JudgeContext,
} from '../decider';

const cr = (id: string, lane: number | null, name = id) => ({
  run: { id, lane, run_number: 1 },
  team: { name },
});

const L1 = cr('r1', 1, 'FF Nord');
const L2 = cr('r2', 2, 'FF Süd');
const NOLANE = cr('r0', null, 'Ohne Bahn');

describe('isLaneBoundJudge', () => {
  it.each<[JudgeMode | null | undefined, number | null | undefined, boolean]>([
    ['per_station', 1, false],
    ['per_station', 2, false],
    ['per_station_and_lane', 1, false],
    ['per_station_and_lane', 2, true],
    ['per_station_and_lane', 4, true],
    [null, 2, false],
    [undefined, undefined, false],
    ['per_station_and_lane', null, false],
  ])('mode %s × lanes %s → %s', (judgeMode, laneCount, expected) => {
    expect(isLaneBoundJudge({ judgeMode, laneCount })).toBe(expected);
  });
});

describe('visibleRuns', () => {
  it('sorts by lane, lane-less runs last', () => {
    expect(visibleRuns([NOLANE, L2, L1]).map((r) => r.run.id)).toEqual(['r1', 'r2', 'r0']);
  });

  it('does not mutate the input', () => {
    const input = [L2, L1];
    visibleRuns(input);
    expect(input.map((r) => r.run.id)).toEqual(['r2', 'r1']);
  });

  it('per_station shows all runs', () => {
    const ctx: JudgeContext = { judgeMode: 'per_station', laneCount: 2, myLane: 1 };
    expect(visibleRuns([L1, L2], ctx).map((r) => r.run.id)).toEqual(['r1', 'r2']);
  });

  it('per_station_and_lane shows only the own lane', () => {
    const ctx: JudgeContext = { judgeMode: 'per_station_and_lane', laneCount: 2, myLane: 2 };
    expect(visibleRuns([L1, L2], ctx).map((r) => r.run.id)).toEqual(['r2']);
  });

  it('per_station_and_lane keeps lane-less runs visible', () => {
    const ctx: JudgeContext = { judgeMode: 'per_station_and_lane', laneCount: 2, myLane: 1 };
    expect(visibleRuns([NOLANE, L2], ctx).map((r) => r.run.id)).toEqual(['r0']);
  });

  it('per_station_and_lane without chosen lane shows nothing', () => {
    const ctx: JudgeContext = { judgeMode: 'per_station_and_lane', laneCount: 2, myLane: null };
    expect(visibleRuns([L1, L2], ctx)).toEqual([]);
  });

  it('per_station_and_lane with 1 lane behaves like per_station', () => {
    const ctx: JudgeContext = { judgeMode: 'per_station_and_lane', laneCount: 1, myLane: null };
    expect(visibleRuns([L1], ctx).map((r) => r.run.id)).toEqual(['r1']);
  });
});

/**
 * Full matrix: judge mode × lane count × number of current runs
 * (× own lane for lane-bound judges).
 */
describe('resolveSubmitTarget matrix', () => {
  const modes: JudgeMode[] = ['per_station', 'per_station_and_lane'];

  describe.each(modes)('%s', (judgeMode) => {
    it('lane_count 1, 0 runs → no_run', () => {
      const t = resolveSubmitTarget([], { judgeMode, laneCount: 1 });
      expect(t.ok).toBe(false);
      if (!t.ok) expect(t.reason).toBe('no_run');
    });

    it('lane_count 1, 1 run → direct', () => {
      const t = resolveSubmitTarget([L1], { judgeMode, laneCount: 1 });
      expect(t.ok && t.run_id).toBe('r1');
    });

    it('lane_count 1, run without lane → direct', () => {
      const t = resolveSubmitTarget([NOLANE], { judgeMode, laneCount: 1 });
      expect(t.ok && t.run_id).toBe('r0');
    });

    it('lane_count 2, 0 runs → no_run (even without lane)', () => {
      const t = resolveSubmitTarget([], { judgeMode, laneCount: 2, myLane: null });
      expect(!t.ok && t.reason).toBe('no_run');
    });
  });

  describe('per_station, lane_count 2', () => {
    const base: JudgeContext = { judgeMode: 'per_station', laneCount: 2 };

    it('1 run (lane 1) → direct', () => {
      expect(resolveSubmitTarget([L1], base)).toMatchObject({ ok: true, run_id: 'r1' });
    });

    it('1 run (lane 2) → direct', () => {
      expect(resolveSubmitTarget([L2], base)).toMatchObject({ ok: true, run_id: 'r2' });
    });

    it('2 runs, nothing selected → choose_run with both options ordered by lane', () => {
      const t = resolveSubmitTarget([L2, L1], base);
      expect(t.ok).toBe(false);
      if (!t.ok) {
        expect(t.reason).toBe('choose_run');
        expect(t.message).toBe('Bitte die Bahn wählen.');
        expect(t.options.map((o) => o.run.id)).toEqual(['r1', 'r2']);
      }
    });

    it('2 runs, lane 2 selected → that run', () => {
      expect(resolveSubmitTarget([L1, L2], { ...base, selectedRunId: 'r2' })).toMatchObject({
        ok: true,
        run_id: 'r2',
      });
    });

    it('2 runs, stale selection → choose again', () => {
      const t = resolveSubmitTarget([L1, L2], { ...base, selectedRunId: 'gone' });
      expect(!t.ok && t.reason).toBe('choose_run');
    });

    it('myLane is ignored in per_station mode', () => {
      const t = resolveSubmitTarget([L1, L2], { ...base, myLane: 1 });
      expect(!t.ok && t.reason).toBe('choose_run');
    });
  });

  describe('per_station_and_lane, lane_count 2', () => {
    const base: JudgeContext = { judgeMode: 'per_station_and_lane', laneCount: 2 };

    it('no lane chosen → no_lane', () => {
      const t = resolveSubmitTarget([L1, L2], { ...base, myLane: null });
      expect(t.ok).toBe(false);
      if (!t.ok) {
        expect(t.reason).toBe('no_lane');
        expect(t.message).toBe('Bitte zuerst deine Bahn wählen.');
      }
    });

    it.each([
      [1, 'r1'],
      [2, 'r2'],
    ])('2 runs, own lane %i → %s directly (no choice)', (myLane, expected) => {
      expect(resolveSubmitTarget([L1, L2], { ...base, myLane })).toMatchObject({
        ok: true,
        run_id: expected,
      });
    });

    it('1 run on the own lane → direct', () => {
      expect(resolveSubmitTarget([L1], { ...base, myLane: 1 })).toMatchObject({
        ok: true,
        run_id: 'r1',
      });
    });

    it('1 run on the other lane → no_run_on_lane', () => {
      const t = resolveSubmitTarget([L1], { ...base, myLane: 2 });
      expect(t.ok).toBe(false);
      if (!t.ok) {
        expect(t.reason).toBe('no_run_on_lane');
        expect(t.message).toBe('Auf Bahn 2 läuft gerade kein Lauf.');
      }
    });

    it('selection of the other lane is ignored', () => {
      expect(
        resolveSubmitTarget([L1, L2], { ...base, myLane: 1, selectedRunId: 'r2' }),
      ).toMatchObject({ ok: true, run_id: 'r1' });
    });

    it('run without lane is targeted by a lane judge', () => {
      expect(resolveSubmitTarget([NOLANE], { ...base, myLane: 2 })).toMatchObject({
        ok: true,
        run_id: 'r0',
      });
    });
  });

  it('result carries the run object for labels', () => {
    const t = resolveSubmitTarget([L1]);
    expect(t.ok && t.run.team.name).toBe('FF Nord');
  });
});

describe('canEnterErrors', () => {
  it('ok and choose_run allow entry; others not', () => {
    expect(canEnterErrors(resolveSubmitTarget([L1]))).toBe(true);
    expect(canEnterErrors(resolveSubmitTarget([L1, L2]))).toBe(true);
    expect(canEnterErrors(resolveSubmitTarget([]))).toBe(false);
    expect(
      canEnterErrors(
        resolveSubmitTarget([L1], { judgeMode: 'per_station_and_lane', laneCount: 2, myLane: 2 }),
      ),
    ).toBe(false);
    expect(
      canEnterErrors(
        resolveSubmitTarget([L1], { judgeMode: 'per_station_and_lane', laneCount: 2 }),
      ),
    ).toBe(false);
  });
});
