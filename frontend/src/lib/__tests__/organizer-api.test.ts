// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

// Wire format of every endpoint the organizer review (PR 3) uses, fed
// with the bodies built by the pure helpers in src/lib.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { apiClient } from '../api';
import { setCurrentBody } from '../current-runs';
import { clearHandTimesBody, handTimesBody, validateHandTimes } from '../hand-time';
import { lineupBody } from '../lineup';
import { createRunBody, roundBodies } from '../runs';

let fetchMock: ReturnType<typeof vi.fn>;

function respondWith(body: unknown, status = 200) {
  fetchMock.mockResolvedValueOnce(
    new Response(status === 204 ? null : JSON.stringify(body), {
      status,
      headers: { 'Content-Type': 'application/json' },
    }),
  );
}

function lastCall(): { url: string; method: string; body: unknown } {
  const call = fetchMock.mock.calls[fetchMock.mock.calls.length - 1]!;
  const init = (call[1] ?? {}) as RequestInit;
  return {
    url: call[0] as string,
    method: init.method ?? 'GET',
    body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined,
  };
}

const ts = '2027-05-01T10:00:00Z';
const run = {
  id: 'run1',
  event_id: 'e1',
  discipline_id: 'd1',
  team_id: 't1',
  runner_id: null,
  run_number: 3,
  lane: 1,
  status: 'completed',
  original_time_ms: 47_240,
  correction_ms: 0,
  started_at: null,
  ended_at: null,
  lineup: [],
  lineup_confirmed_at: null,
  lineup_confirmed_by: null,
  time_source: 'lichtschranke',
  hand_time_1_ms: null,
  hand_time_2_ms: null,
  created_at: ts,
  updated_at: ts,
};
const event = {
  id: 'e1',
  owner_sub: 'o',
  name: 'E',
  date: '2027-05-01',
  location: null,
  is_active: true,
  current_discipline_id: 'd1',
  current_run_id: 'a',
  current_run_ids: ['a', 'b'],
  decider_tokens: ['1', '2', '3', '4'],
  station_token: 'x',
  created_at: ts,
  updated_at: ts,
};
const penalty = {
  id: 'p1',
  event_id: 'e1',
  discipline_id: 'd1',
  run_id: 'run1',
  team_id: 't1',
  runner_id: null,
  station: 'knotengestell',
  kind: 'fv',
  code: 'knotengestell.mehrere_knoten_falsch',
  is_disqualification: true,
  decider_slot: 0,
  delta_ms: 0,
  reason_code: null,
  status: 'approved',
  created_at: ts,
  reviewed_at: ts,
  reviewed_by: 'sub',
};

beforeEach(() => {
  fetchMock = vi.fn();
  vi.stubGlobal('fetch', fetchMock);
});
afterEach(() => {
  vi.unstubAllGlobals();
});

describe('Laufende Läufe', () => {
  it('POST /current with run_ids ordered by lane', async () => {
    respondWith(event);
    const ev = await apiClient.events.setCurrent('e1', setCurrentBody('d1', ['a', 'b']));
    expect(lastCall()).toEqual({
      url: '/api/events/e1/current',
      method: 'POST',
      body: { discipline_id: 'd1', run_ids: ['a', 'b'] },
    });
    expect(ev.current_run_ids).toEqual(['a', 'b']);
  });

  it('clearing sends an empty run_ids list', async () => {
    respondWith({ ...event, current_run_id: null, current_run_ids: [] });
    await apiClient.events.setCurrent('e1', setCurrentBody('d1', [null, null]));
    expect(lastCall().body).toEqual({ discipline_id: 'd1', run_ids: [] });
  });

  it('legacy event row without current_run_ids', async () => {
    const { current_run_ids: _drop, ...legacy } = event;
    respondWith(legacy);
    const ev = await apiClient.events.setCurrent('e1', setCurrentBody('d1', ['a']));
    expect(Array.isArray(ev.current_run_ids)).toBe(true);
  });
});

describe('review', () => {
  it('list per discipline (all statuses)', async () => {
    respondWith([penalty]);
    const list = await apiClient.penalties.list('e1', { discipline_id: 'd1' });
    expect(lastCall()).toEqual({ url: '/api/events/e1/penalties?discipline_id=d1', method: 'GET', body: undefined });
    expect(list[0]!.kind).toBe('fv');
  });

  it('Bestätigen = approve', async () => {
    respondWith(penalty);
    await apiClient.penalties.approve('e1', 'p1');
    expect(lastCall()).toEqual({ url: '/api/events/e1/penalties/p1/approve', method: 'POST', body: undefined });
  });

  it('Zurücknehmen = reject', async () => {
    respondWith({ ...penalty, status: 'rejected' });
    const p = await apiClient.penalties.reject('e1', 'p1');
    expect(lastCall()).toEqual({ url: '/api/events/e1/penalties/p1/reject', method: 'POST', body: undefined });
    expect(p.status).toBe('rejected');
  });

  it('Übernehmen = bulk-approve', async () => {
    respondWith({ approved: 2, rejected: 0, dq: 1 });
    const res = await apiClient.penalties.bulkApprove('e1', 'run1');
    expect(lastCall()).toEqual({
      url: '/api/events/e1/runs/run1/penalties/bulk-approve',
      method: 'POST',
      body: undefined,
    });
    expect(res).toEqual({ approved: 2, rejected: 0, dq: 1 });
  });
});

describe('hand times', () => {
  it('PUT hand-times with the parsed values', async () => {
    const v = validateHandTimes('47,23', '0:47,245');
    if (!v.ok) throw new Error('unexpected');
    respondWith({ ...run, time_source: 'handstopp', hand_time_1_ms: 47_230, hand_time_2_ms: 47_245, original_time_ms: v.mean });
    const r = await apiClient.runs.setHandTimes('e1', 'd1', 'run1', handTimesBody(v.t1, v.t2));
    expect(lastCall()).toEqual({
      url: '/api/events/e1/disciplines/d1/runs/run1/hand-times',
      method: 'PUT',
      body: { hand_time_1_ms: 47_230, hand_time_2_ms: 47_245 },
    });
    expect(r.time_source).toBe('handstopp');
    expect(r.original_time_ms).toBe(47_240);
  });

  it('Handzeit entfernen → { clear: true }', async () => {
    respondWith(run);
    await apiClient.runs.setHandTimes('e1', 'd1', 'run1', clearHandTimesBody());
    expect(lastCall().body).toEqual({ clear: true });
  });
});

describe('lineup', () => {
  it('PUT lineup in roster order', async () => {
    respondWith({ ...run, lineup: ['r1', 'r2'], lineup_confirmed_by: 'organizer', lineup_confirmed_at: ts });
    const body = lineupBody(['r2', 'r1', 'r2'], [{ id: 'r1' }, { id: 'r2' }, { id: 'r3' }]);
    const r = await apiClient.runs.setLineup('e1', 'd1', 'run1', body.runner_ids);
    expect(lastCall()).toEqual({
      url: '/api/events/e1/disciplines/d1/runs/run1/lineup',
      method: 'PUT',
      body: { runner_ids: ['r1', 'r2'] },
    });
    expect(r.lineup_confirmed_by).toBe('organizer');
  });
});

describe('run management', () => {
  it('create run', async () => {
    respondWith({ ...run, status: 'pending' });
    await apiClient.runs.create('e1', 'd1', createRunBody('t1', [{ run_number: 2 }], 2, 2));
    expect(lastCall()).toEqual({
      url: '/api/events/e1/disciplines/d1/runs',
      method: 'POST',
      body: { team_id: 't1', runner_id: null, run_number: 3, lane: 2 },
    });
  });

  it('round bodies are posted one by one', async () => {
    const bodies = roundBodies(
      [
        { team_id: 'a', start_number: 1, lane: 1 },
        { team_id: 'b', start_number: 2, lane: 2 },
      ],
      [],
      2,
    );
    for (const b of bodies) {
      respondWith({ ...run, ...b });
      await apiClient.runs.create('e1', 'd1', b);
    }
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(lastCall().body).toEqual({ team_id: 'b', runner_id: null, run_number: 2, lane: 2 });
  });

  it('status change uses run_number + run_id path', async () => {
    respondWith({ ...run, status: 'cancelled' });
    await apiClient.runs.update('e1', 'd1', 3, 'run1', { status: 'cancelled' });
    expect(lastCall()).toEqual({
      url: '/api/events/e1/disciplines/d1/runs/3/run1',
      method: 'PUT',
      body: { status: 'cancelled' },
    });
  });
});
