// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError, apiClient, getCatalogue, stationPenaltyBody } from '../api';

type FetchMock = ReturnType<typeof vi.fn>;
let fetchMock: FetchMock;

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(status === 204 ? null : JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

function respondWith(body: unknown, status = 200) {
  fetchMock.mockResolvedValueOnce(jsonResponse(body, status));
}

function lastCall(): { url: string; init: RequestInit; body: unknown } {
  const call = fetchMock.mock.calls[fetchMock.mock.calls.length - 1]!;
  const init = (call[1] ?? {}) as RequestInit;
  return {
    url: call[0] as string,
    init,
    body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined,
  };
}

const ts = '2026-10-01T10:00:00Z';
const run = {
  id: 'run1',
  event_id: 'e1',
  discipline_id: 'd1',
  team_id: 't1',
  runner_id: null,
  run_number: 1,
  lane: 1,
  status: 'active',
  original_time_ms: null,
  correction_ms: 0,
  started_at: null,
  ended_at: null,
  created_at: ts,
  updated_at: ts,
};
const entry = {
  team_id: 't1',
  discipline_id: 'd1',
  event_id: 'e1',
  start_number: 1,
  lane: null,
  wg: 'wg2',
  created_at: ts,
  updated_at: ts,
};
const discipline = {
  id: 'd1',
  event_id: 'e1',
  name: 'GS',
  mode: 'gruppenstaffette',
  sync_mode: 'live',
  position: 1,
  created_at: ts,
  updated_at: ts,
};

beforeEach(() => {
  fetchMock = vi.fn();
  vi.stubGlobal('fetch', fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('catalogue', () => {
  it('GET /api/catalogue and normalizes rows', async () => {
    respondWith([
      { code: 'knotengestell.ein_knoten_falsch', station: 'knotengestell', kind: 'sz', label_de: '1 Knoten falsch' },
      { code: 'x', station: 'knoten', kind: 'fv', label_de: 'X' },
    ]);
    const list = await getCatalogue();
    const { url, init } = lastCall();
    expect(url).toBe('/api/catalogue');
    expect(init.method).toBeUndefined();
    expect(init.credentials).toBe('include');
    expect(list).toHaveLength(2);
    expect(list[1]?.station).toBe('knotengestell');
  });

  it('apiClient.catalogue.list is the same endpoint', async () => {
    respondWith([]);
    await apiClient.catalogue.list();
    expect(lastCall().url).toBe('/api/catalogue');
  });
});

describe('disciplines', () => {
  it('create sends lane_count and judge_mode', async () => {
    respondWith({ ...discipline, lane_count: 1, judge_mode: 'per_station_and_lane' });
    const d = await apiClient.disciplines.create('e1', {
      name: 'GS',
      mode: 'gruppenstaffette',
      position: 1,
      lane_count: 1,
      judge_mode: 'per_station_and_lane',
    });
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/events/e1/disciplines');
    expect(init.method).toBe('POST');
    expect((init.headers as Record<string, string>)['Content-Type']).toBe('application/json');
    expect(body).toEqual({
      name: 'GS',
      mode: 'gruppenstaffette',
      position: 1,
      lane_count: 1,
      judge_mode: 'per_station_and_lane',
    });
    expect(d.lane_count).toBe(1);
  });

  it('update returns lane_conflicts', async () => {
    respondWith({ ...discipline, lane_count: 1, judge_mode: 'per_station', lane_conflicts: ['t2'] });
    const d = await apiClient.disciplines.update('e1', 'd1', {
      name: 'GS',
      mode: 'gruppenstaffette',
      position: 1,
      lane_count: 1,
      judge_mode: 'per_station',
    });
    const { url, init } = lastCall();
    expect(url).toBe('/api/events/e1/disciplines/d1');
    expect(init.method).toBe('PUT');
    expect(d.lane_conflicts).toEqual(['t2']);
  });

  it('legacy response without new fields gets defaults', async () => {
    respondWith(discipline);
    const d = await apiClient.disciplines.create('e1', {
      name: 'GS',
      mode: 'gruppenstaffette',
      position: 1,
    });
    expect(d.lane_count).toBe(2);
    expect(d.judge_mode).toBe('per_station');
  });
});

describe('events', () => {
  it('setCurrent legacy body', async () => {
    respondWith({ id: 'e1', current_run_id: 'run1' });
    const e = await apiClient.events.setCurrent('e1', { discipline_id: 'd1', run_id: 'run1' });
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/events/e1/current');
    expect(init.method).toBe('POST');
    expect(body).toEqual({ discipline_id: 'd1', run_id: 'run1' });
    expect(e.current_run_ids).toEqual(['run1']);
  });

  it('setCurrent parallel body with run_ids', async () => {
    respondWith({ id: 'e1', current_run_id: 'run1', current_run_ids: ['run1', 'run2'] });
    const e = await apiClient.events.setCurrent('e1', {
      discipline_id: 'd1',
      run_ids: ['run1', 'run2'],
    });
    expect(lastCall().body).toEqual({ discipline_id: 'd1', run_ids: ['run1', 'run2'] });
    expect(e.current_run_ids).toEqual(['run1', 'run2']);
  });

  it('detail normalizes nested rows', async () => {
    respondWith({
      event: { id: 'e1', current_run_id: null },
      disciplines: [discipline],
      teams: [],
      runners: [],
      team_entries: [{ ...entry, wg: null }],
      runs: [run],
    });
    const d = await apiClient.events.detail('e1');
    expect(lastCall().url).toBe('/api/events/e1');
    expect(d.event.current_run_ids).toEqual([]);
    expect(d.team_entries[0]?.wg_history).toEqual([]);
    expect(d.runs[0]?.lineup).toEqual([]);
  });
});

describe('runners', () => {
  const input = { first_name: 'Max', last_name: 'Muster', birth_date: '2012-03-04', position: 1 };

  it('create never sends gender', async () => {
    respondWith({ id: 'r1', ...input, gender: null });
    await apiClient.runners.create('e1', 't1', { ...input, gender: 'male' } as typeof input);
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/events/e1/teams/t1/runners');
    expect(init.method).toBe('POST');
    expect(body).toEqual(input);
    expect(body).not.toHaveProperty('gender');
  });

  it('update never sends gender', async () => {
    respondWith({ id: 'r1', ...input });
    await apiClient.runners.update('e1', 't1', 'r1', { ...input, gender: null } as typeof input);
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/events/e1/teams/t1/runners/r1');
    expect(init.method).toBe('PUT');
    expect(body).not.toHaveProperty('gender');
  });
});

describe('team entries', () => {
  it('upsert sends wg and only the input fields', async () => {
    respondWith({ ...entry, wg_warnings: [{ runner_id: 'r1', age: 15, wg: 'wg2', reason: 'zu alt' }] });
    const e = await apiClient.teamEntries.upsert('e1', 'd1', {
      team_id: 't1',
      start_number: 1,
      lane: null,
      wg: 'wg2',
      // stray fields from a spread entry must not leak
      ...({ wg_history: [], created_at: ts } as object),
    } as Parameters<typeof apiClient.teamEntries.upsert>[2]);
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/events/e1/disciplines/d1/entries');
    expect(init.method).toBe('POST');
    expect(body).toEqual({ team_id: 't1', start_number: 1, lane: null, wg: 'wg2' });
    expect(e.wg_warnings).toHaveLength(1);
  });

  it('moveWg posts wg + note', async () => {
    respondWith({
      ...entry,
      wg: 'wg1',
      wg_history: [{ from: 'wg2', to: 'wg1', by: 'orga', at: ts, note: '2 Mädchen' }],
    });
    const e = await apiClient.teamEntries.moveWg('e1', 'd1', 't1', {
      wg: 'wg1',
      note: ' 2 Mädchen ',
    });
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/events/e1/disciplines/d1/entries/t1/move-wg');
    expect(init.method).toBe('POST');
    expect(body).toEqual({ wg: 'wg1', note: '2 Mädchen' });
    expect(e.wg).toBe('wg1');
    expect(e.wg_history).toHaveLength(1);
  });

  it('moveWg omits an empty note', async () => {
    respondWith({ ...entry, wg: 'wg3' });
    await apiClient.teamEntries.moveWg('e1', 'd1', 't1', { wg: 'wg3', note: '  ' });
    expect(lastCall().body).toEqual({ wg: 'wg3' });
    respondWith({ ...entry, wg: 'wg3' });
    await apiClient.teamEntries.moveWg('e1', 'd1', 't1', { wg: 'wg3' });
    expect(lastCall().body).toEqual({ wg: 'wg3' });
  });

  it('moveWg surfaces API errors', async () => {
    respondWith({ error: { code: 'forbidden', message: 'Nur Organisator' } }, 403);
    await expect(
      apiClient.teamEntries.moveWg('e1', 'd1', 't1', { wg: 'wg1' }),
    ).rejects.toMatchObject({ status: 403, code: 'forbidden', message: 'Nur Organisator' });
  });
});

describe('runs', () => {
  it('setLineup PUTs runner_ids', async () => {
    respondWith({ ...run, lineup: ['r1', 'r2'], lineup_confirmed_by: 'organizer', lineup_confirmed_at: ts });
    const r = await apiClient.runs.setLineup('e1', 'd1', 'run1', ['r1', 'r2']);
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/events/e1/disciplines/d1/runs/run1/lineup');
    expect(init.method).toBe('PUT');
    expect(body).toEqual({ runner_ids: ['r1', 'r2'] });
    expect(r.lineup).toEqual(['r1', 'r2']);
    expect(r.lineup_confirmed_by).toBe('organizer');
  });

  it('setLineup with empty list clears', async () => {
    respondWith(run);
    await apiClient.runs.setLineup('e1', 'd1', 'run1', []);
    expect(lastCall().body).toEqual({ runner_ids: [] });
  });

  it('setHandTimes PUTs both times', async () => {
    respondWith({
      ...run,
      time_source: 'handstopp',
      hand_time_1_ms: 47230,
      hand_time_2_ms: 47245,
      original_time_ms: 47240,
    });
    const r = await apiClient.runs.setHandTimes('e1', 'd1', 'run1', {
      hand_time_1_ms: 47230,
      hand_time_2_ms: 47245,
    });
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/events/e1/disciplines/d1/runs/run1/hand-times');
    expect(init.method).toBe('PUT');
    expect(body).toEqual({ hand_time_1_ms: 47230, hand_time_2_ms: 47245 });
    expect(r.time_source).toBe('handstopp');
  });

  it('clearHandTimes sends { clear: true }', async () => {
    respondWith(run);
    const r = await apiClient.runs.clearHandTimes('e1', 'd1', 'run1');
    const { url, body } = lastCall();
    expect(url).toBe('/api/events/e1/disciplines/d1/runs/run1/hand-times');
    expect(body).toEqual({ clear: true });
    expect(r.time_source).toBe('lichtschranke');
  });

  it('list normalizes legacy runs', async () => {
    respondWith([run]);
    const list = await apiClient.runs.list('e1', 'd1');
    expect(lastCall().url).toBe('/api/events/e1/disciplines/d1/runs');
    expect(list[0]?.hand_time_1_ms).toBeNull();
  });
});

describe('penalties', () => {
  it('list maps legacy station keys and kind', async () => {
    respondWith([
      { id: 'p1', station: 'kuebelspritze', is_disqualification: true, delta_ms: 999000, code: undefined },
    ]);
    const list = await apiClient.penalties.list('e1', { status: 'pending', discipline_id: 'd1' });
    expect(lastCall().url).toBe('/api/events/e1/penalties?status=pending&discipline_id=d1');
    expect(list[0]?.station).toBe('angriffslinie');
    expect(list[0]?.kind).toBe('fv');
    expect(list[0]?.code).toBeNull();
  });

  it('bulkApprove endpoint unchanged', async () => {
    respondWith({ approved: 2, rejected: 0, dq: 1 });
    await apiClient.penalties.bulkApprove('e1', 'run1');
    const { url, init } = lastCall();
    expect(url).toBe('/api/events/e1/runs/run1/penalties/bulk-approve');
    expect(init.method).toBe('POST');
  });
});

describe('judge station', () => {
  it('state normalizes current_runs', async () => {
    respondWith({
      event: { id: 'e1', name: 'X', date: '2027-05-10', location: null },
      discipline: { id: 'd1', name: 'GS', mode: 'gruppenstaffette', lane_count: 2, judge_mode: 'per_station' },
      current_runs: [
        { run: { ...run, id: 'run2', lane: 2 }, team: { id: 't2', name: 'B' }, runners: [] },
        { run, team: { id: 't1', name: 'A' }, runners: [] },
      ],
      current_run: { run, team: { id: 't1', name: 'A' }, runners: [] },
      disabled: false,
      allowed_stations: ['startlinie', 'knotengestell'],
    });
    const s = await apiClient.deciderStation.state('owner', 'tok');
    expect(lastCall().url).toBe('/api/decider/owner/tok/state');
    expect(s.current_runs.map((c) => c.team.name)).toEqual(['A', 'B']);
    expect(s.current_run?.run.id).toBe('run1');
  });

  it('submit with catalogue code', async () => {
    respondWith({ id: 'p1', station: 'knotengestell', kind: 'sz', code: 'knotengestell.ein_knoten_falsch', delta_ms: 15000 });
    const p = await apiClient.deciderStation.submit('owner', 'tok', {
      station: 'knotengestell',
      run_id: 'run1',
      code: 'knotengestell.ein_knoten_falsch',
      client_id: 'c1',
      delta_ms: 999,
      is_disqualification: true,
    });
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/decider/owner/tok/penalties');
    expect(init.method).toBe('POST');
    expect(body).toEqual({
      station: 'knotengestell',
      run_id: 'run1',
      code: 'knotengestell.ein_knoten_falsch',
      client_id: 'c1',
    });
    expect(p.kind).toBe('sz');
  });

  it('judge lineup PUT', async () => {
    respondWith({ ...run, lineup: ['r1'], lineup_confirmed_by: 'startlinie' });
    const r = await apiClient.deciderStation.setLineup('owner', 'tok', 'run1', ['r1']);
    const { url, init, body } = lastCall();
    expect(url).toBe('/api/decider/owner/tok/runs/run1/lineup');
    expect(init.method).toBe('PUT');
    expect(body).toEqual({ runner_ids: ['r1'] });
    expect(r.lineup_confirmed_by).toBe('startlinie');
  });

  it('withdraw passes the canonical station as query', async () => {
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 }));
    await apiClient.deciderStation.withdraw('owner', 'tok', 'p1', 'angriffslinie');
    const { url, init } = lastCall();
    expect(url).toBe('/api/decider/owner/tok/penalties/p1?station=angriffslinie');
    expect(init.method).toBe('DELETE');
  });

  it('errors become ApiError', async () => {
    respondWith({ error: { code: 'validation', message: 'Code passt nicht zur Station' } }, 400);
    const err = await apiClient.deciderStation
      .submit('owner', 'tok', { station: 'podest', code: 'knotengestell.x' })
      .catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).code).toBe('validation');
  });
});

describe('stationPenaltyBody', () => {
  it('code wins over legacy fields', () => {
    expect(
      stationPenaltyBody({ station: 'podest', code: 'c', delta_ms: 5, reason_code: 'r' }),
    ).toEqual({ station: 'podest', code: 'c' });
  });

  it('legacy fields when no code', () => {
    expect(
      stationPenaltyBody({ station: 'podest', delta_ms: 5000, reason_code: null, client_id: 'c1' }),
    ).toEqual({ station: 'podest', delta_ms: 5000, reason_code: null, client_id: 'c1' });
    expect(stationPenaltyBody({ station: 'podest', is_disqualification: true })).toEqual({
      station: 'podest',
      is_disqualification: true,
    });
  });
});
