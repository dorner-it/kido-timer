// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError, apiClient } from '../api';

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  fetchMock = vi.fn();
  vi.stubGlobal('fetch', fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

const body = {
  event: { id: 'e1', name: 'Kreisausscheid', date: '2027-06-12', location: null },
  discipline: { id: 'd1', name: 'Gruppenstafette', mode: 'gruppenstaffette' },
  generated_at: '2027-06-12T10:00:00Z',
  provisional: false,
  wg_rankings: [{ wg: 'wg3', rows: [] }],
  tagessieger: [],
  missing_wg: [],
};

describe('apiClient.results.get', () => {
  it('GETs the nested organizer route with credentials and normalizes', async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(JSON.stringify(body), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    );
    const res = await apiClient.results.get('e1', 'd1');
    const [url, init] = fetchMock.mock.calls[0]! as [string, RequestInit];
    expect(url).toBe('/api/events/e1/disciplines/d1/results');
    expect(init.credentials).toBe('include');
    expect(init.method).toBeUndefined();
    expect(res.wg_rankings[0]!.wg_label).toBe('WG3');
    expect(res.wg_rankings[0]!.wg_label_de).toBe('Mädchen bis 14');
  });

  it('maps 404 to ApiError', async () => {
    fetchMock.mockResolvedValueOnce(
      new Response(JSON.stringify({ error: { code: 'not_found', message: 'not found: discipline d1' } }), {
        status: 404,
        headers: { 'Content-Type': 'application/json' },
      }),
    );
    await expect(apiClient.results.get('e1', 'd1')).rejects.toMatchObject({
      status: 404,
      code: 'not_found',
    });
    fetchMock.mockResolvedValueOnce(new Response('x', { status: 500 }));
    await expect(apiClient.results.get('e1', 'd1')).rejects.toBeInstanceOf(ApiError);
  });
});
