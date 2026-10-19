// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { apiClient } from '../api';
import { getApiBase, normalizeApiBase, resolveApiBase } from '../api-base';
import { lineupRequestBody } from '../lineup';

type FetchMock = ReturnType<typeof vi.fn>;
let fetchMock: FetchMock;

function respondWith(body: unknown, status = 200) {
  fetchMock.mockResolvedValueOnce(
    new Response(status === 204 ? null : JSON.stringify(body), {
      status,
      headers: { 'Content-Type': 'application/json' },
    }),
  );
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

const ts = '2027-06-12T10:00:00Z';
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
const stateBody = {
  event: { id: 'e1', name: 'X', date: '2027-06-12', location: null },
  discipline: { id: 'd1', name: 'GS', mode: 'gruppenstaffette', lane_count: 2, judge_mode: 'per_station' },
  current_runs: [{ run, team: { id: 't1', name: 'A' }, runners: [] }],
  disabled: false,
  allowed_stations: ['startlinie', 'knoten'],
};

beforeEach(() => {
  fetchMock = vi.fn();
  vi.stubGlobal('fetch', fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('resolveApiBase / normalizeApiBase', () => {
  it('override string wins over env', () => {
    expect(resolveApiBase('http://192.168.1.20:8080', 'https://api.ki-do.it')).toBe(
      'http://192.168.1.20:8080',
    );
  });

  it('empty override = same origin (wins over env)', () => {
    expect(resolveApiBase('', 'https://api.ki-do.it')).toBe('');
  });

  it('non-string override falls back to env', () => {
    expect(resolveApiBase(undefined, 'https://api.ki-do.it')).toBe('https://api.ki-do.it');
    expect(resolveApiBase(null, 'https://api.ki-do.it')).toBe('https://api.ki-do.it');
    expect(resolveApiBase(42, 'https://api.ki-do.it')).toBe('https://api.ki-do.it');
  });

  it('nothing set → empty string', () => {
    expect(resolveApiBase(undefined, undefined)).toBe('');
  });

  it('strips trailing slashes and whitespace', () => {
    expect(normalizeApiBase(' http://10.0.0.5:3000/// ')).toBe('http://10.0.0.5:3000');
    expect(resolveApiBase('http://x/', undefined)).toBe('http://x');
  });
});

describe('getApiBase runtime override', () => {
  it('reads window.__KIDO_API_BASE__ on every call', async () => {
    vi.stubGlobal('window', { __KIDO_API_BASE__: 'http://192.168.1.20:8080/' });
    expect(getApiBase()).toBe('http://192.168.1.20:8080');
    respondWith(stateBody);
    await apiClient.deciderStation.state('owner', 'tok');
    expect(lastCall().url).toBe('http://192.168.1.20:8080/api/decider/owner/tok/state');

    (globalThis as unknown as { window: { __KIDO_API_BASE__?: string } }).window.__KIDO_API_BASE__ =
      'http://10.0.0.9';
    respondWith(stateBody);
    await apiClient.deciderStation.state('owner', 'tok');
    expect(lastCall().url).toBe('http://10.0.0.9/api/decider/owner/tok/state');
  });

  it('without override the build-time default (empty in tests) is used', async () => {
    vi.stubGlobal('window', {});
    expect(getApiBase()).toBe(resolveApiBase(undefined, import.meta.env.PUBLIC_API_BASE_URL));
    respondWith(stateBody);
    await apiClient.deciderStation.state('owner', 'tok');
    expect(lastCall().url).toBe(`${getApiBase()}/api/decider/owner/tok/state`);
  });
});

describe('station state for the judge app', () => {
  it('passes the station as query', async () => {
    respondWith(stateBody);
    await apiClient.deciderStation.state('owner', 'tok', 'knotengestell');
    expect(lastCall().url).toBe('/api/decider/owner/tok/state?station=knotengestell');
  });

  it('no station → no query', async () => {
    respondWith(stateBody);
    await apiClient.deciderStation.state('owner', 'tok', null);
    expect(lastCall().url).toBe('/api/decider/owner/tok/state');
  });

  it('normalizes penalties (legacy rows get kind + canonical station)', async () => {
    respondWith({
      ...stateBody,
      penalties: [
        { id: 'p1', run_id: 'run1', station: 'knoten', status: 'pending', is_disqualification: true, delta_ms: 0 },
        { id: 'p2', run_id: 'run1', station: 'knotengestell', kind: 'sz', code: 'knotengestell.ein_knoten_falsch', status: 'approved', delta_ms: 15000 },
      ],
    });
    const s = await apiClient.deciderStation.state('owner', 'tok', 'knotengestell');
    expect(s.penalties).toHaveLength(2);
    expect(s.penalties![0]!.kind).toBe('fv');
    expect(s.penalties![0]!.station).toBe('knotengestell');
    expect(s.penalties![1]!.status).toBe('approved');
  });

  it('missing penalties field stays undefined (older backend)', async () => {
    respondWith(stateBody);
    const s = await apiClient.deciderStation.state('owner', 'tok');
    expect(s.penalties).toBeUndefined();
    expect(s.allowed_stations).toEqual(['startlinie', 'knotengestell']);
  });
});

describe('judge lineup PUT with station', () => {
  it('sends runner_ids + station from lineupRequestBody', async () => {
    respondWith({ ...run, lineup: ['a', 'b'], lineup_confirmed_by: 'startlinie', lineup_confirmed_at: ts });
    const roster = [
      { id: 'b', first_name: 'B', last_name: 'B', position: 2 },
      { id: 'a', first_name: 'A', last_name: 'A', position: 1 },
    ];
    const body = lineupRequestBody(new Set(['b', 'a']), roster);
    const r = await apiClient.deciderStation.setLineup('owner', 'tok', 'run1', body.runner_ids, body.station);
    const { url, init, body: sent } = lastCall();
    expect(url).toBe('/api/decider/owner/tok/runs/run1/lineup');
    expect(init.method).toBe('PUT');
    expect(sent).toEqual({ runner_ids: ['a', 'b'], station: 'startlinie' });
    expect(r.lineup_confirmed_at).toBe(ts);
  });

  it('without station keeps the PR 1 body', async () => {
    respondWith({ ...run, lineup: [] });
    await apiClient.deciderStation.setLineup('owner', 'tok', 'run1', []);
    expect(lastCall().body).toEqual({ runner_ids: [] });
  });
});
