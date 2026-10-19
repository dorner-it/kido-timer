// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ApiError, apiClient } from '../api';
import {
  INVALID_ACCESS_TEXT,
  INVALID_ACCESS_TITLE,
  countUnsentForAccess,
  isInvalidAccessError,
  isInvalidAccessStatus,
  unsentMessage,
} from '../judge-access';
import { resolveJudgeParams, standaloneApiBase } from '../judge-spa-params';
import { drainOnce, droppedLog, penaltyQueue, stationQueueItem } from '../penalty-queue';

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

type FetchMock = ReturnType<typeof vi.fn>;
let fetchMock: FetchMock;
let replace: ReturnType<typeof vi.fn>;

function errorResponse(status: number): Response {
  return new Response(JSON.stringify({ error: { code: 'unauthorized', message: 'unauthorized' } }), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

function stubWindow(pathname: string, extra: Record<string, unknown> = {}) {
  replace = vi.fn();
  vi.stubGlobal('window', {
    location: { pathname, search: '', hash: '', replace },
    ...extra,
  });
}

/** Fresh api + judge-access modules (resets the sticky redirect flag of
 *  `api.ts`, so a wrong redirect in an earlier test can't hide one here). */
async function fresh() {
  vi.resetModules();
  const api = await import('../api');
  const access = await import('../judge-access');
  return { apiClient: api.apiClient, startJudgePolling: access.startJudgePolling };
}

const item = (client_id: string, token = 'tok') =>
  stationQueueItem({
    owner_sub: 'o',
    token,
    body: { station: 'knotengestell', run_id: 'r1', code: 'knotengestell.ein_knoten_falsch' },
    label: '1 Knoten falsch',
    client_id,
    now: 1,
  });

beforeEach(() => {
  fetchMock = vi.fn();
  vi.stubGlobal('fetch', fetchMock);
  vi.stubGlobal('localStorage', new MemoryStorage());
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('invalid-access classification', () => {
  it('401 is invalid access; 404 / 409 / 5xx / network are not', () => {
    expect(isInvalidAccessStatus(401)).toBe(true);
    for (const s of [400, 403, 404, 409, 500, 503]) expect(isInvalidAccessStatus(s)).toBe(false);
    expect(isInvalidAccessStatus(null)).toBe(false);
    expect(isInvalidAccessError(new ApiError(401, 'unauthorized', 'x'))).toBe(true);
    expect(isInvalidAccessError(new ApiError(404, 'not_found', 'x'))).toBe(false);
    expect(isInvalidAccessError(new TypeError('Failed to fetch'))).toBe(false);
  });

  it('German texts', () => {
    expect(INVALID_ACCESS_TITLE).toBe('Zugang ungültig');
    expect(INVALID_ACCESS_TEXT).toBe(
      'Dieser Strafrichter-Zugang ist nicht mehr gültig. Bitte hol dir bei der Wettkampfleitung einen neuen QR-Code.',
    );
    expect(unsentMessage(0)).toBeNull();
    expect(unsentMessage(1)).toBe('1 Meldung konnte nicht gesendet werden.');
    expect(unsentMessage(3)).toBe('3 Meldungen konnten nicht gesendet werden.');
  });
});

describe('judge requests: 401 never redirects to /login', () => {
  beforeEach(() => stubWindow('/decider/o/tok'));

  it('state / submit / withdraw / lineup / legacy slot state reject with 401, no redirect', async () => {
    fetchMock.mockImplementation(async () => errorResponse(401));
    const calls: Promise<unknown>[] = [
      apiClient.deciderStation.state('o', 'tok', 'knotengestell'),
      apiClient.deciderStation.submit('o', 'tok', {
        station: 'knotengestell',
        run_id: 'r1',
        code: 'knotengestell.ein_knoten_falsch',
        client_id: 'c1',
      }),
      apiClient.deciderStation.withdraw('o', 'tok', 'p1', 'knotengestell'),
      apiClient.deciderStation.setLineup('o', 'tok', 'r1', ['a']),
      apiClient.decider.state('o', 1, 'tok'),
      apiClient.decider.submit('o', 1, 'tok', { delta_ms: 5000 }),
      apiClient.decider.withdraw('o', 1, 'tok', 'p1'),
    ];
    for (const c of calls) {
      await expect(c).rejects.toMatchObject({ status: 401 });
    }
    expect(replace).not.toHaveBeenCalled();
  });

  it('polling stops on 401 and reports invalid access once', async () => {
    const { apiClient, startJudgePolling } = await fresh();
    vi.useFakeTimers();
    fetchMock.mockImplementation(async () => errorResponse(401));
    const onInvalid = vi.fn();
    const onError = vi.fn();
    const stop = startJudgePolling({
      load: () => apiClient.deciderStation.state('o', 'tok', null),
      intervalMs: 4000,
      onInvalid,
      onError,
    });
    await vi.advanceTimersByTimeAsync(20_000);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(onInvalid).toHaveBeenCalledTimes(1);
    expect(onError).not.toHaveBeenCalled();
    expect(replace).not.toHaveBeenCalled();
    stop();
  });

  it('404 / network errors keep polling (no invalid screen)', async () => {
    const { apiClient, startJudgePolling } = await fresh();
    vi.useFakeTimers();
    fetchMock
      .mockImplementationOnce(async () => errorResponse(404))
      .mockImplementationOnce(async () => {
        throw new TypeError('Failed to fetch');
      })
      .mockImplementation(async () => errorResponse(404));
    const onInvalid = vi.fn();
    const onError = vi.fn();
    const stop = startJudgePolling({
      load: () => apiClient.deciderStation.state('o', 'tok', null),
      intervalMs: 4000,
      onInvalid,
      onError,
    });
    await vi.advanceTimersByTimeAsync(8_500);
    stop();
    expect(fetchMock).toHaveBeenCalledTimes(3);
    expect(onInvalid).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledTimes(3);
    await vi.advanceTimersByTimeAsync(20_000);
    expect(fetchMock).toHaveBeenCalledTimes(3);
  });
});

describe('unsent queue on 401', () => {
  beforeEach(() => stubWindow('/decider/o/tok'));

  it('keeps 401 items (blocked), counts them, does not drop or retry them', async () => {
    penaltyQueue.enqueue(item('c1'));
    penaltyQueue.enqueue(item('c2'));
    fetchMock.mockImplementation(async () => errorResponse(401));

    const res = await drainOnce();
    expect(res.unauthorized?.map((i) => i.client_id)).toEqual(['c1', 'c2']);
    expect(res.dropped).toEqual([]);
    expect(res.remaining).toBe(2);
    expect(droppedLog.list()).toEqual([]);
    expect(penaltyQueue.list().every((i) => i.blocked === 'unauthorized')).toBe(true);
    expect(countUnsentForAccess(penaltyQueue.list(), 'o', 'tok')).toBe(2);
    expect(replace).not.toHaveBeenCalled();

    // Blocked items are not hammered on the next drain.
    fetchMock.mockClear();
    await drainOnce();
    expect(fetchMock).not.toHaveBeenCalled();
    expect(penaltyQueue.size()).toBe(2);
  });

  it('a 401 item does not stop items of another (valid) link; unblock retries', async () => {
    penaltyQueue.enqueue(item('old', 'old-tok'));
    penaltyQueue.enqueue(item('new', 'new-tok'));
    fetchMock.mockImplementation(async (url: string) =>
      url.includes('/old-tok/')
        ? errorResponse(401)
        : new Response(JSON.stringify({ id: 'new' }), {
            status: 201,
            headers: { 'Content-Type': 'application/json' },
          }),
    );
    const res = await drainOnce();
    expect(res.confirmed).toEqual([{ client_id: 'new', id: 'new' }]);
    expect(res.unauthorized?.map((i) => i.client_id)).toEqual(['old']);
    expect(countUnsentForAccess(penaltyQueue.list(), 'o', 'old-tok')).toBe(1);
    expect(countUnsentForAccess(penaltyQueue.list(), 'o', 'new-tok')).toBe(0);

    penaltyQueue.unblock('o', 'old-tok');
    expect(penaltyQueue.list()[0]!.blocked).toBeUndefined();
    fetchMock.mockClear();
    await drainOnce();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('block() adds a not-yet-queued item', () => {
    penaltyQueue.block(item('x'));
    expect(penaltyQueue.list()).toMatchObject([{ client_id: 'x', blocked: 'unauthorized' }]);
    penaltyQueue.block(item('x'));
    expect(penaltyQueue.size()).toBe(1);
  });
});

describe('static on-prem SPA build (same JudgeApp + api client)', () => {
  it('401 from the desktop server: no redirect, LAN base used, polling stops', async () => {
    const { apiClient, startJudgePolling } = await fresh();
    vi.useFakeTimers();
    stubWindow('/judge/', { __KIDO_API_BASE__: standaloneApiBase(undefined) });
    const params = resolveJudgeParams('?t=old-code&o=sub-1');
    expect(params.ok).toBe(true);
    if (!params.ok) return;

    fetchMock.mockImplementation(async () => errorResponse(401));
    const onInvalid = vi.fn();
    const stop = startJudgePolling({
      load: () =>
        apiClient.deciderStation.state(params.params.ownerSub, params.params.token, null),
      intervalMs: 4000,
      onInvalid,
    });
    await vi.advanceTimersByTimeAsync(20_000);
    stop();
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(fetchMock.mock.calls[0]![0]).toBe('/api/decider/sub-1/old-code/state');
    expect(onInvalid).toHaveBeenCalledTimes(1);
    expect(replace).not.toHaveBeenCalled();
  });

  it('explicit LAN api base override is used as well', async () => {
    const { apiClient } = await fresh();
    stubWindow('/judge/', { __KIDO_API_BASE__: 'http://192.168.1.20:8080/' });
    fetchMock.mockImplementation(async () => errorResponse(401));
    await expect(apiClient.deciderStation.state('sub-1', 'old', null)).rejects.toMatchObject({ status: 401 });
    expect(fetchMock.mock.calls[0]![0]).toBe('http://192.168.1.20:8080/api/decider/sub-1/old/state');
    expect(replace).not.toHaveBeenCalled();
  });
});

describe('organizer requests: 401 still redirects to /login (regression)', () => {
  it('redirects with return_to + expired', async () => {
    const { apiClient: organizer } = await fresh();
    stubWindow('/events');
    fetchMock.mockImplementation(async () => errorResponse(401));
    await expect(organizer.events.list()).rejects.toMatchObject({ status: 401 });
    expect(replace).toHaveBeenCalledTimes(1);
    expect(replace).toHaveBeenCalledWith('/login?return_to=%2Fevents&expired=1');
  });

  it('opt-out probes (auth.me) do not redirect', async () => {
    const { apiClient: organizer } = await fresh();
    stubWindow('/');
    fetchMock.mockImplementation(async () => errorResponse(401));
    await expect(organizer.auth.me()).rejects.toMatchObject({ status: 401 });
    expect(replace).not.toHaveBeenCalled();
  });
});

describe('legacy slot pad (/decider/:owner/:slot/:token) on 401', () => {
  beforeEach(() => stubWindow('/decider/o/2/slot-tok'));

  const slotItem = (client_id: string) => ({
    client_id,
    owner_sub: 'o',
    slot: 2,
    token: 'slot-tok',
    delta_ms: 15_000,
    reason_code: null,
    label: '+15 s',
    queued_at: 1,
  });

  it('slot state 401 is an invalid-access error and never redirects', async () => {
    fetchMock.mockImplementation(async () => errorResponse(401));
    const { apiClient: fresh_ } = await fresh();
    const err = await fresh_.decider.state('o', 2, 'slot-tok').catch((e: unknown) => e);
    const { isInvalidAccessError: isInvalid } = await import('../judge-access');
    expect(isInvalid(err)).toBe(true);
    expect(replace).not.toHaveBeenCalled();
  });

  it('slot submits with 401 stay queued (blocked) and are counted', async () => {
    penaltyQueue.enqueue(slotItem('s1'));
    fetchMock.mockImplementation(async () => errorResponse(401));
    const res = await drainOnce();
    expect(res.unauthorized?.map((i) => i.client_id)).toEqual(['s1']);
    expect(countUnsentForAccess(penaltyQueue.list(), 'o', 'slot-tok')).toBe(1);
    expect(replace).not.toHaveBeenCalled();
  });
});
