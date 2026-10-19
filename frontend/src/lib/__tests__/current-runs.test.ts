// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import type { Run, RunStatus } from '../api';
import {
  candidatesForLane,
  effectiveLane,
  selectionChanged,
  selectionFromCurrent,
  setCurrentBody,
  suggestNext,
  validateSelection,
} from '../current-runs';

type R = Pick<Run, 'id' | 'team_id' | 'lane' | 'run_number' | 'status' | 'discipline_id'>;
function run(id: string, run_number: number, lane: number | null, team_id = `t-${id}`, status: RunStatus = 'pending'): R {
  return { id, team_id, lane, run_number, status, discipline_id: 'd1' };
}

const entries = [
  { team_id: 't-a', lane: 1 },
  { team_id: 't-b', lane: 2 },
  { team_id: 't-x', lane: 2 },
];

describe('effectiveLane', () => {
  it('single lane → always 1', () => {
    expect(effectiveLane(run('a', 1, 2), entries, 1)).toBe(1);
    expect(effectiveLane(run('a', 1, null), [], 1)).toBe(1);
  });
  it('run lane wins over entry lane', () => {
    expect(effectiveLane(run('a', 1, 2, 't-a'), entries, 2)).toBe(2);
  });
  it('falls back to entry lane', () => {
    expect(effectiveLane(run('a', 1, null, 't-b'), entries, 2)).toBe(2);
  });
  it('null when unknown', () => {
    expect(effectiveLane(run('a', 1, null, 't-none'), entries, 2)).toBeNull();
  });
});

describe('selectionFromCurrent', () => {
  const runs = [run('a', 1, 1), run('b', 2, 2), run('c', 3, null, 't-none')];
  it('places runs on their lane', () => {
    expect(selectionFromCurrent(['b', 'a'], runs, entries, 2)).toEqual(['a', 'b']);
  });
  it('one lane', () => {
    expect(selectionFromCurrent(['a'], runs, entries, 1)).toEqual(['a']);
  });
  it('empty', () => {
    expect(selectionFromCurrent([], runs, entries, 2)).toEqual([null, null]);
  });
  it('run without lane fills a free slot', () => {
    expect(selectionFromCurrent(['c', 'b'], runs, entries, 2)).toEqual(['c', 'b']);
  });
  it('unknown id still shown in a free slot', () => {
    expect(selectionFromCurrent(['zzz'], runs, entries, 2)).toEqual(['zzz', null]);
  });
  it('conflicting lanes: second goes to the free slot', () => {
    const r = [run('a', 1, 1), run('a2', 2, 1)];
    expect(selectionFromCurrent(['a', 'a2'], r, entries, 2)).toEqual(['a', 'a2']);
  });
  it('more ids than lanes are dropped', () => {
    expect(selectionFromCurrent(['a', 'b'], runs, entries, 1)).toEqual(['a']);
  });
});

describe('candidatesForLane', () => {
  const runs = [
    run('b2', 4, 2),
    run('a1', 3, 1),
    run('n', 2, null, 't-none'),
    run('cx', 1, 1, 't-c', 'cancelled'),
    run('done', 5, 1, 't-d', 'completed'),
  ];
  it('lane 1: own lane + unassigned, no cancelled, sorted', () => {
    expect(candidatesForLane(1, runs, entries, 2).map((r) => r.id)).toEqual(['n', 'a1', 'done']);
  });
  it('lane 2', () => {
    expect(candidatesForLane(2, runs, entries, 2).map((r) => r.id)).toEqual(['n', 'b2']);
  });
  it('single lane: all non-cancelled', () => {
    expect(candidatesForLane(1, runs, entries, 1).map((r) => r.id)).toEqual(['n', 'a1', 'b2', 'done']);
  });
});

describe('validateSelection', () => {
  const runs = [run('a', 1, 1), run('b', 2, 2), run('n', 3, null, 't-none'), run('cx', 4, 1, 't-c', 'cancelled'), { ...run('o', 5, 1), discipline_id: 'other' }];
  it('valid two-lane selection', () => {
    expect(validateSelection(['a', 'b'], runs, entries, 2, 'd1')).toEqual([]);
  });
  it('empty selection is valid (clears)', () => {
    expect(validateSelection([null, null], runs, entries, 2, 'd1')).toEqual([]);
  });
  it('one lane free is valid', () => {
    expect(validateSelection([null, 'b'], runs, entries, 2, 'd1')).toEqual([]);
  });
  it('unassigned run fits any lane', () => {
    expect(validateSelection([null, 'n'], runs, entries, 2, 'd1')).toEqual([]);
  });
  it('too many for lane_count 1', () => {
    const errs = validateSelection(['a', 'n'], runs, entries, 1, 'd1');
    expect(errs.some((e) => e.startsWith('Höchstens 1'))).toBe(true);
    expect(errs.some((e) => e.includes('nur 1 Bahn'))).toBe(true);
  });
  it('duplicate run', () => {
    expect(validateSelection(['n', 'n'], runs, entries, 2, 'd1')).toContain(
      'Ein Lauf kann nicht auf zwei Bahnen gleichzeitig laufen.',
    );
  });
  it('wrong lane', () => {
    expect(validateSelection(['b', 'a'], runs, entries, 2, 'd1')).toEqual([
      'Bahn 1: Lauf 2 ist Bahn 2 zugeordnet.',
      'Bahn 2: Lauf 1 ist Bahn 1 zugeordnet.',
    ]);
  });
  it('cancelled run', () => {
    expect(validateSelection(['cx', null], runs, entries, 2, 'd1')).toEqual(['Bahn 1: Lauf 4 ist abgebrochen.']);
  });
  it('foreign / unknown run', () => {
    expect(validateSelection(['o', null], runs, entries, 2, 'd1')).toEqual([
      'Bahn 1: Lauf gehört nicht zu diesem Wettkampf.',
    ]);
    expect(validateSelection(['zzz', null], runs, entries, 2, 'd1')).toHaveLength(1);
  });
  it('lane_count 1 ignores run lanes', () => {
    expect(validateSelection(['b'], runs, entries, 1, 'd1')).toEqual([]);
  });
});

describe('setCurrentBody', () => {
  it('run_ids ordered by lane, empty slots dropped', () => {
    expect(setCurrentBody('d1', ['a', 'b'])).toEqual({ discipline_id: 'd1', run_ids: ['a', 'b'] });
    expect(setCurrentBody('d1', [null, 'b'])).toEqual({ discipline_id: 'd1', run_ids: ['b'] });
    expect(setCurrentBody('d1', [null, null])).toEqual({ discipline_id: 'd1', run_ids: [] });
  });
});

describe('selectionChanged', () => {
  it('detects changes', () => {
    expect(selectionChanged(['a', 'b'], ['a', 'b'])).toBe(false);
    expect(selectionChanged([null, 'b'], ['b'])).toBe(false);
    expect(selectionChanged(['a', null], ['a', 'b'])).toBe(true);
    expect(selectionChanged(['b', 'a'], ['a', 'b'])).toBe(true);
    expect(selectionChanged([null, null], [])).toBe(false);
  });
});

describe('suggestNext', () => {
  const runs = [
    run('a1', 1, 1, 't-a', 'completed'),
    run('b1', 2, 2, 't-b', 'completed'),
    run('a2', 3, 1),
    run('b2', 4, 2),
    run('a3', 5, 1),
    run('n', 6, null, 't-none'),
  ];
  it('next pending per lane after the current one', () => {
    expect(suggestNext(['a1', 'b1'], runs, entries, 2)).toEqual(['a2', 'b2']);
    expect(suggestNext(['a2', 'b2'], runs, entries, 2)).toEqual(['a3', 'n']);
  });
  it('empty lanes start with the first pending run', () => {
    expect(suggestNext([null, null], runs, entries, 2)).toEqual(['a2', 'b2']);
  });
  it('an unassigned run is not suggested twice', () => {
    const r = [run('n1', 1, null, 't-none'), run('n2', 2, null, 't-none2')];
    expect(suggestNext([null, null], r, entries, 2)).toEqual(['n1', 'n2']);
  });
  it('lanes without further runs become empty', () => {
    expect(suggestNext(['a3', 'n'], runs, entries, 2)).toEqual([null, null]);
  });
  it('single lane', () => {
    expect(suggestNext(['a2'], runs, entries, 1)).toEqual(['b2']);
  });
});
