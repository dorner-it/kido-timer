// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  EXPECTED_LINEUP_SIZE,
  initialSelection,
  isLineupConfirmed,
  isLineupDirty,
  lineupCountLabel,
  lineupRequestBody,
  lineupSignature,
  lineupSizeHint,
  lineupStatusText,
  orderedSelection,
  runnerName,
  sortRoster,
  toggleRunner,
} from '../lineup';

const r = (id: string, position: number, last = id.toUpperCase(), first = 'Max') => ({
  id,
  first_name: first,
  last_name: last,
  position,
});

const roster7 = [r('g', 7), r('a', 1), r('c', 3), r('b', 2), r('e', 5), r('d', 4), r('f', 6)];
const roster6 = roster7.filter((x) => x.id !== 'g');

describe('team size', () => {
  it('Wertungsbögen name WK 1 … WK 6', () => {
    expect(EXPECTED_LINEUP_SIZE).toBe(6);
  });
});

describe('sortRoster / runnerName', () => {
  it('by position, then last name, then first name', () => {
    expect(sortRoster(roster7).map((x) => x.id)).toEqual(['a', 'b', 'c', 'd', 'e', 'f', 'g']);
    const tie = [r('x', 1, 'Zahn'), r('y', 1, 'Adler', 'Ben'), r('z', 1, 'Adler', 'Anna')];
    expect(sortRoster(tie).map((x) => x.id)).toEqual(['z', 'y', 'x']);
  });

  it('does not mutate', () => {
    const copy = [...roster7];
    sortRoster(roster7);
    expect(roster7).toEqual(copy);
  });

  it('name', () => {
    expect(runnerName(r('a', 1, 'Muster', 'Erika'))).toBe('Erika Muster');
  });
});

describe('initialSelection', () => {
  it('stored lineup wins (unknown ids dropped)', () => {
    const s = initialSelection({ lineup: ['a', 'c', 'ghost'] }, roster7);
    expect([...s].sort()).toEqual(['a', 'c']);
  });

  it('no lineup, roster ≤ 6 → all ticked', () => {
    expect(initialSelection({ lineup: [] }, roster6).size).toBe(6);
  });

  it('no lineup, roster > 6 (substitutes) → nothing ticked', () => {
    expect(initialSelection({ lineup: [] }, roster7).size).toBe(0);
  });

  it('stored lineup with only unknown ids falls back like empty', () => {
    expect(initialSelection({ lineup: ['ghost'] }, roster6).size).toBe(6);
    expect(initialSelection({ lineup: ['ghost'] }, roster7).size).toBe(0);
  });

  it('empty roster → empty', () => {
    expect(initialSelection({ lineup: null }, []).size).toBe(0);
  });

  it('custom expected size', () => {
    expect(initialSelection({}, roster7, 7).size).toBe(7);
  });
});

describe('toggleRunner', () => {
  it('adds and removes without mutating', () => {
    const s0 = new Set(['a']);
    const s1 = toggleRunner(s0, 'b');
    expect([...s1].sort()).toEqual(['a', 'b']);
    expect([...s0]).toEqual(['a']);
    expect([...toggleRunner(s1, 'a')]).toEqual(['b']);
  });
});

describe('request body', () => {
  it('runner_ids in roster order + station startlinie', () => {
    expect(lineupRequestBody(new Set(['f', 'a', 'c']), roster7)).toEqual({
      runner_ids: ['a', 'c', 'f'],
      station: 'startlinie',
    });
  });

  it('drops ids not on the roster', () => {
    expect(orderedSelection(new Set(['a', 'ghost']), roster7)).toEqual(['a']);
  });

  it('empty selection → empty list (clears)', () => {
    expect(lineupRequestBody(new Set(), roster7)).toEqual({ runner_ids: [], station: 'startlinie' });
  });
});

describe('lineupSizeHint', () => {
  it('6 → ok', () => {
    expect(lineupSizeHint(6)).toEqual({ level: 'ok', message: null });
  });
  it('0 → warn nobody', () => {
    expect(lineupSizeHint(0)).toEqual({ level: 'warn', message: 'Niemand angekreuzt.' });
  });
  it('fewer → warn', () => {
    expect(lineupSizeHint(5).message).toBe('Nur 5 von üblicherweise 6 Wettkämpfern angekreuzt.');
  });
  it('more → warn, never blocks', () => {
    expect(lineupSizeHint(7)).toEqual({ level: 'warn', message: '7 Wettkämpfer angekreuzt – üblich sind 6.' });
  });
  it('custom expected', () => {
    expect(lineupSizeHint(4, 4).level).toBe('ok');
  });
  it('count label', () => {
    expect(lineupCountLabel(5, 7)).toBe('5 / 7 angekreuzt');
  });
});

describe('confirmed state', () => {
  it('isLineupConfirmed', () => {
    expect(isLineupConfirmed({ lineup_confirmed_at: ts() })).toBe(true);
    expect(isLineupConfirmed({ lineup_confirmed_at: null })).toBe(false);
    expect(isLineupConfirmed({})).toBe(false);
  });

  it('unconfirmed is always dirty (needs confirmation)', () => {
    expect(isLineupDirty(new Set(['a']), { lineup: ['a'] }, roster7)).toBe(true);
  });

  it('confirmed + same selection (any order) → clean', () => {
    const run = { lineup: ['b', 'a'], lineup_confirmed_at: ts() };
    expect(isLineupDirty(new Set(['a', 'b']), run, roster7)).toBe(false);
  });

  it('confirmed + changed selection → dirty', () => {
    const run = { lineup: ['a', 'b'], lineup_confirmed_at: ts() };
    expect(isLineupDirty(new Set(['a']), run, roster7)).toBe(true);
    expect(isLineupDirty(new Set(['a', 'c']), run, roster7)).toBe(true);
    expect(isLineupDirty(new Set(['a', 'b', 'c']), run, roster7)).toBe(true);
  });

  it('confirmed with ids no longer on the roster compares the remaining ones', () => {
    const run = { lineup: ['a', 'ghost'], lineup_confirmed_at: ts() };
    expect(isLineupDirty(new Set(['a']), run, roster7)).toBe(false);
  });

  it('status text', () => {
    expect(lineupStatusText({})).toBe('Noch nicht bestätigt');
    const local = new Date(2027, 5, 12, 9, 5).toISOString();
    expect(lineupStatusText({ lineup_confirmed_at: local, lineup_confirmed_by: 'startlinie' })).toBe(
      'Bestätigt 09:05 (Startlinie)',
    );
    expect(lineupStatusText({ lineup_confirmed_at: local, lineup_confirmed_by: 'organizer' })).toBe(
      'Bestätigt 09:05 (Wettkampfleitung)',
    );
    expect(lineupStatusText({ lineup_confirmed_at: 'not-a-date' })).toBe('Bestätigt');
  });

  it('signature changes with lineup or confirmation, not with order', () => {
    const a = lineupSignature({ lineup: ['a', 'b'], lineup_confirmed_at: 'x' });
    expect(lineupSignature({ lineup: ['b', 'a'], lineup_confirmed_at: 'x' })).toBe(a);
    expect(lineupSignature({ lineup: ['a'], lineup_confirmed_at: 'x' })).not.toBe(a);
    expect(lineupSignature({ lineup: ['a', 'b'], lineup_confirmed_at: 'y' })).not.toBe(a);
    expect(lineupSignature({})).toBe('|');
  });
});

function ts(): string {
  return '2027-06-12T10:00:00Z';
}
