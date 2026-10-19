// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  LINEUP_MISSING_TEXT,
  formatClock,
  lineupBody,
  lineupChanged,
  lineupConfirmationText,
  lineupMissing,
  rosterForTeam,
  rostersByTeam,
  toggleLineup,
  unknownLineupIds,
} from '../lineup';
import {
  createRunBody,
  entryForRun,
  laneForNewRun,
  nextRunNumber,
  roundBodies,
  runLabel,
  runStatusClasses,
  sortRuns,
} from '../runs';

describe('lineup confirmation text', () => {
  const clock = (iso: string | null | undefined) => (iso ? '10:42' : null);
  it('Startlinie', () => {
    expect(
      lineupConfirmationText({ lineup_confirmed_by: 'startlinie', lineup_confirmed_at: '2027-05-01T08:42:00Z' }, clock),
    ).toBe('Startlinie, 10:42');
  });
  it('Organisator', () => {
    expect(
      lineupConfirmationText({ lineup_confirmed_by: 'organizer', lineup_confirmed_at: '2027-05-01T08:42:00Z' }, clock),
    ).toBe('Organisator, 10:42');
  });
  it('without timestamp', () => {
    expect(lineupConfirmationText({ lineup_confirmed_by: 'organizer', lineup_confirmed_at: null }, clock)).toBe(
      'Organisator',
    );
  });
  it('nobody', () => {
    expect(lineupConfirmationText({ lineup_confirmed_by: null, lineup_confirmed_at: null })).toBe('nicht bestätigt');
  });
  it('formatClock', () => {
    expect(formatClock(null)).toBeNull();
    expect(formatClock('nope')).toBeNull();
    expect(formatClock('2027-05-01T08:42:00Z')).toMatch(/^\d{2}:\d{2}$/);
  });
});

describe('lineupMissing', () => {
  it('only completed runs without lineup', () => {
    expect(lineupMissing({ status: 'completed', lineup: [] })).toBe(true);
    expect(lineupMissing({ status: 'completed', lineup: ['r1'] })).toBe(false);
    expect(lineupMissing({ status: 'active', lineup: [] })).toBe(false);
    expect(lineupMissing({ status: 'pending', lineup: [] })).toBe(false);
    expect(lineupMissing({ status: 'cancelled', lineup: [] })).toBe(false);
  });
  it('warning text', () => {
    expect(LINEUP_MISSING_TEXT).toBe('Besetzung fehlt — Tagessieger nutzt dann den ganzen Kader');
  });
});

describe('roster & lineup editing', () => {
  const runners = [
    { id: 'r3', team_id: 't1', position: 3, first_name: 'C', last_name: 'Z' },
    { id: 'r1', team_id: 't1', position: 1, first_name: 'A', last_name: 'X' },
    { id: 'o1', team_id: 't2', position: 1, first_name: 'O', last_name: 'O' },
    { id: 'r2', team_id: 't1', position: 1, first_name: 'B', last_name: 'Y' },
  ];
  it('rosterForTeam filters + sorts by position, then name', () => {
    expect(rosterForTeam(runners, 't1').map((r) => r.id)).toEqual(['r1', 'r2', 'r3']);
  });
  it('rostersByTeam groups per team in roster order (duplicate positions by name)', () => {
    const m = rostersByTeam(runners);
    expect(m.get('t1')?.map((r) => r.id)).toEqual(['r1', 'r2', 'r3']);
    expect(m.get('t2')?.map((r) => r.id)).toEqual(['o1']);
    expect(m.get('t3')).toBeUndefined();
  });
  it('rostersByTeam does not mutate its input', () => {
    const input = [...runners];
    rostersByTeam(input);
    expect(input.map((r) => r.id)).toEqual(runners.map((r) => r.id));
  });
  it('rostersByTeam shows a server-side swap after reload', () => {
    // Before: A at 1, B at 2. A moved to 2 → server swaps, reload returns both.
    const a = { id: 'a', team_id: 't1', position: 1, first_name: 'A', last_name: 'Anton' };
    const b = { id: 'b', team_id: 't1', position: 2, first_name: 'B', last_name: 'Berta' };
    const before = [a, b];
    const afterReload = [
      { ...a, position: 2 },
      { ...b, position: 1 },
    ];
    expect(rostersByTeam(before).get('t1')?.map((r) => r.id)).toEqual(['a', 'b']);
    expect(rostersByTeam(afterReload).get('t1')?.map((r) => [r.id, r.position])).toEqual([
      ['b', 1],
      ['a', 2],
    ]);
  });
  it('toggleLineup', () => {
    expect(toggleLineup([], 'r1')).toEqual(['r1']);
    expect(toggleLineup(['r1', 'r2'], 'r1')).toEqual(['r2']);
  });
  it('lineupBody: roster order, dedup, drops foreign ids', () => {
    const roster = rosterForTeam(runners, 't1');
    expect(lineupBody(['r3', 'o1', 'r1', 'r3'], roster)).toEqual({ runner_ids: ['r1', 'r3'] });
    expect(lineupBody([], roster)).toEqual({ runner_ids: [] });
  });
  it('unknownLineupIds', () => {
    expect(unknownLineupIds(['r1', 'gone'], rosterForTeam(runners, 't1'))).toEqual(['gone']);
  });
  it('lineupChanged is order-insensitive', () => {
    expect(lineupChanged(['a', 'b'], ['b', 'a'])).toBe(false);
    expect(lineupChanged(['a'], ['a', 'b'])).toBe(true);
    expect(lineupChanged(['a', 'c'], ['a', 'b'])).toBe(true);
    expect(lineupChanged([], [])).toBe(false);
  });
});

describe('runs helpers', () => {
  it('nextRunNumber', () => {
    expect(nextRunNumber([])).toBe(1);
    expect(nextRunNumber([{ run_number: 3 }, { run_number: 7 }, { run_number: 1 }])).toBe(8);
  });
  it('laneForNewRun', () => {
    expect(laneForNewRun(1, null)).toBe(1);
    expect(laneForNewRun(1, 2)).toBe(1);
    expect(laneForNewRun(2, 2)).toBe(2);
    expect(laneForNewRun(2, null)).toBeNull();
    expect(laneForNewRun(2, 3)).toBeNull(); // lane conflict → unassigned
    expect(laneForNewRun(4, 4)).toBe(4);
  });
  it('createRunBody', () => {
    expect(createRunBody('t1', [{ run_number: 2 }], 2, 1)).toEqual({
      team_id: 't1',
      runner_id: null,
      run_number: 3,
      lane: 1,
    });
  });
  it('roundBodies: start-number order, sequential numbers', () => {
    const entries = [
      { team_id: 'b', start_number: 2, lane: 2 },
      { team_id: 'a', start_number: 1, lane: 1 },
      { team_id: 'c', start_number: 3, lane: null },
    ];
    expect(roundBodies(entries, [{ run_number: 4 }], 2)).toEqual([
      { team_id: 'a', runner_id: null, run_number: 5, lane: 1 },
      { team_id: 'b', runner_id: null, run_number: 6, lane: 2 },
      { team_id: 'c', runner_id: null, run_number: 7, lane: null },
    ]);
    expect(roundBodies([], [], 2)).toEqual([]);
    expect(roundBodies(entries, [], 1).map((b) => b.lane)).toEqual([1, 1, 1]);
  });
  it('sortRuns', () => {
    const r = sortRuns([
      { run_number: 2, lane: 1 },
      { run_number: 1, lane: null },
      { run_number: 1, lane: 2 },
    ]);
    expect(r).toEqual([
      { run_number: 1, lane: 2 },
      { run_number: 1, lane: null },
      { run_number: 2, lane: 1 },
    ]);
  });
  it('entryForRun & runLabel', () => {
    const teams = [{ id: 't1', name: 'FF Musterdorf' }];
    const entries = [{ team_id: 't1', start_number: 12, wg: 'wg2' as const }];
    expect(entryForRun({ team_id: 't1' }, entries)).toBe(entries[0]);
    expect(runLabel({ run_number: 3, team_id: 't1' }, teams, entries)).toBe('Lauf 3 · #12 FF Musterdorf · WG2');
    expect(runLabel({ run_number: 4, team_id: 'tx' }, teams, entries)).toBe(
      'Lauf 4 · Unbekannte Mannschaft · WG fehlt',
    );
    expect(runLabel({ run_number: 5, team_id: 't1' }, teams, [{ team_id: 't1', start_number: 1, wg: null }])).toBe(
      'Lauf 5 · #1 FF Musterdorf · WG fehlt',
    );
  });
  it('runStatusClasses covers all statuses', () => {
    for (const s of ['pending', 'active', 'completed', 'cancelled'] as const) {
      expect(runStatusClasses(s)).toMatch(/^bg-status-/);
    }
  });
});
