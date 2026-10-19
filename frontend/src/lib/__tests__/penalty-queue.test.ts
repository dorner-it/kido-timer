// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError } from '../api';
import {
  attemptSubmit,
  classifySubmitError,
  defaultSender,
  drainOnce,
  droppedLog,
  isStationSubmit,
  penaltyQueue,
  stationQueueItem,
  type QueuedSlotSubmit,
  type QueuedSubmit,
} from '../penalty-queue';

class MemoryStorage {
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

const slotItem: QueuedSlotSubmit = {
  client_id: 'slot-1',
  owner_sub: 'o',
  slot: 2,
  token: 't',
  delta_ms: 5000,
  reason_code: 'x',
  label: '+5 s',
  queued_at: 1,
};

const stationItem = stationQueueItem({
  owner_sub: 'o',
  token: 'tok',
  body: { station: 'knotengestell', run_id: 'r1', code: 'knotengestell.ein_knoten_falsch' },
  label: '1 Knoten falsch',
  client_id: 'st-1',
  now: 42,
});

describe('stationQueueItem', () => {
  it('copies the client id into the body (idempotent retries)', () => {
    expect(stationItem).toEqual({
      flow: 'station',
      client_id: 'st-1',
      owner_sub: 'o',
      token: 'tok',
      body: {
        station: 'knotengestell',
        run_id: 'r1',
        code: 'knotengestell.ein_knoten_falsch',
        client_id: 'st-1',
      },
      label: '1 Knoten falsch',
      queued_at: 42,
    });
  });

  it('isStationSubmit: legacy items without flow are slot items', () => {
    expect(isStationSubmit(stationItem)).toBe(true);
    expect(isStationSubmit(slotItem)).toBe(false);
    expect(isStationSubmit({ ...slotItem, flow: 'slot' })).toBe(false);
  });
});

describe('classifySubmitError', () => {
  it.each([
    [500, true],
    [502, true],
    [503, true],
    [408, true],
    [429, true],
    [400, false],
    [401, false],
    [403, false],
    [404, false],
    [409, false],
  ])('HTTP %i → retriable %s', (status, retriable) => {
    const r = classifySubmitError(new ApiError(status, 'c', 'm'));
    expect(r).toEqual({ ok: false, retriable, status, message: 'm' });
  });

  it('network error → retriable, no status', () => {
    expect(classifySubmitError(new TypeError('Failed to fetch'))).toEqual({
      ok: false,
      retriable: true,
      status: null,
      message: 'Failed to fetch',
    });
  });

  it('non-Error → generic message', () => {
    expect(classifySubmitError('boom').message).toBe('Netzwerkfehler');
  });
});

describe('attemptSubmit', () => {
  it('ok → id', async () => {
    await expect(attemptSubmit(stationItem, async () => ({ id: 'st-1' }))).resolves.toEqual({
      ok: true,
      id: 'st-1',
    });
  });

  it('failure → classified', async () => {
    const r = await attemptSubmit(stationItem, async () => {
      throw new ApiError(409, 'conflict', 'nicht mehr aktuell');
    });
    expect(r).toEqual({ ok: false, retriable: false, status: 409, message: 'nicht mehr aktuell' });
  });
});

describe('defaultSender routes by flow', () => {
  let fetchMock: ReturnType<typeof vi.fn>;
  beforeEach(() => {
    fetchMock = vi.fn().mockImplementation(
      async () =>
        new Response(JSON.stringify({ id: 'srv' }), {
          status: 201,
          headers: { 'Content-Type': 'application/json' },
        }),
    );
    vi.stubGlobal('fetch', fetchMock);
  });
  afterEach(() => vi.unstubAllGlobals());

  it('station item → station route with catalogue body', async () => {
    await defaultSender(stationItem);
    const [url, init] = fetchMock.mock.calls[0]! as [string, RequestInit];
    expect(url).toBe('/api/decider/o/tok/penalties');
    expect(JSON.parse(init.body as string)).toEqual({
      station: 'knotengestell',
      run_id: 'r1',
      client_id: 'st-1',
      code: 'knotengestell.ein_knoten_falsch',
    });
  });

  it('slot item → slot route', async () => {
    await defaultSender(slotItem);
    const [url, init] = fetchMock.mock.calls[0]! as [string, RequestInit];
    expect(url).toBe('/api/decider/o/2/t/penalties');
    expect(JSON.parse(init.body as string)).toEqual({ delta_ms: 5000, reason_code: 'x', client_id: 'slot-1' });
  });
});

describe('queue persistence + drain (retries)', () => {
  let fetchMock: ReturnType<typeof vi.fn>;
  beforeEach(() => {
    vi.stubGlobal('localStorage', new MemoryStorage());
    fetchMock = vi.fn();
    vi.stubGlobal('fetch', fetchMock);
  });
  afterEach(() => vi.unstubAllGlobals());

  const ok = () =>
    new Response(JSON.stringify({ id: 'srv' }), { status: 201, headers: { 'Content-Type': 'application/json' } });
  const err = (status: number) =>
    new Response(JSON.stringify({ error: { code: 'x', message: `E${status}` } }), {
      status,
      headers: { 'Content-Type': 'application/json' },
    });

  it('enqueue is idempotent per client id; remove works', () => {
    penaltyQueue.enqueue(stationItem);
    penaltyQueue.enqueue(stationItem);
    penaltyQueue.enqueue(slotItem);
    expect(penaltyQueue.size()).toBe(2);
    penaltyQueue.remove('st-1');
    expect(penaltyQueue.list().map((i: QueuedSubmit) => i.client_id)).toEqual(['slot-1']);
  });

  it('survives a reload (stored as JSON incl. station body)', () => {
    penaltyQueue.enqueue(stationItem);
    const raw = (globalThis.localStorage as unknown as MemoryStorage).getItem('kido_penalty_queue_v1')!;
    expect(JSON.parse(raw)[0]).toEqual(stationItem);
  });

  it('corrupt storage reads as empty', () => {
    (globalThis.localStorage as unknown as MemoryStorage).setItem('kido_penalty_queue_v1', '{"not":"array"}');
    expect(penaltyQueue.list()).toEqual([]);
  });

  it('network down: stops at the first retriable failure and keeps everything', async () => {
    penaltyQueue.enqueue(stationItem);
    penaltyQueue.enqueue({ ...stationItem, client_id: 'st-2', body: { ...stationItem.body, client_id: 'st-2' } });
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));
    const res = await drainOnce();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(res).toEqual({ confirmed: [], dropped: [], remaining: 2 });
  });

  it('retry later succeeds with the same client id', async () => {
    penaltyQueue.enqueue(stationItem);
    fetchMock.mockRejectedValueOnce(new TypeError('offline'));
    await drainOnce();
    fetchMock.mockResolvedValueOnce(ok());
    const res = await drainOnce();
    expect(res.confirmed).toEqual([{ client_id: 'st-1', id: 'srv' }]);
    expect(res.remaining).toBe(0);
    const bodies = fetchMock.mock.calls.map((c) => JSON.parse((c[1] as RequestInit).body as string).client_id);
    expect(bodies).toEqual(['st-1', 'st-1']);
  });

  it('5xx keeps the item, 4xx drops it into the dropped log', async () => {
    penaltyQueue.enqueue(stationItem);
    fetchMock.mockResolvedValueOnce(err(503));
    expect((await drainOnce()).remaining).toBe(1);

    fetchMock.mockResolvedValueOnce(err(409));
    const res = await drainOnce();
    expect(res.remaining).toBe(0);
    expect(res.dropped).toHaveLength(1);
    expect(res.dropped[0]).toMatchObject({ client_id: 'st-1', http_status: 409, reason: 'E409', flow: 'station' });
    expect(droppedLog.list()[0]!.client_id).toBe('st-1');
  });

  it('mixed slot + station items drain in order', async () => {
    penaltyQueue.enqueue(slotItem);
    penaltyQueue.enqueue(stationItem);
    fetchMock.mockResolvedValueOnce(ok()).mockResolvedValueOnce(ok());
    const res = await drainOnce();
    expect(res.confirmed.map((c) => c.client_id)).toEqual(['slot-1', 'st-1']);
    expect((fetchMock.mock.calls[0]![0] as string)).toContain('/o/2/t/');
    expect((fetchMock.mock.calls[1]![0] as string)).toContain('/o/tok/penalties');
  });
});
