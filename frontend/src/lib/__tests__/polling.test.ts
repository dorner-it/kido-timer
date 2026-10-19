// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { POLL_IDLE_MS, POLL_LIVE_MS, createPoller, pollIntervalMs, type PollerDeps } from '../polling';

let hidden = false;
let visibilityCb: (() => void) | null = null;
let unsubscribed = 0;

function deps(): PollerDeps {
  return {
    setTimeout: (fn, ms) => setTimeout(fn, ms),
    clearTimeout: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
    isHidden: () => hidden,
    onVisibilityChange: (cb) => {
      visibilityCb = cb;
      return () => {
        unsubscribed += 1;
        visibilityCb = null;
      };
    },
  };
}

function setHidden(v: boolean) {
  hidden = v;
  visibilityCb?.();
}

beforeEach(() => {
  vi.useFakeTimers();
  hidden = false;
  visibilityCb = null;
  unsubscribed = 0;
});
afterEach(() => {
  vi.useRealTimers();
});

describe('pollIntervalMs', () => {
  it('2–3 s while a run is current, slower otherwise', () => {
    expect(pollIntervalMs(true)).toBe(POLL_LIVE_MS);
    expect(POLL_LIVE_MS).toBeGreaterThanOrEqual(2000);
    expect(POLL_LIVE_MS).toBeLessThanOrEqual(3000);
    expect(pollIntervalMs(false)).toBe(POLL_IDLE_MS);
    expect(POLL_IDLE_MS).toBeGreaterThan(POLL_LIVE_MS);
  });
});

describe('createPoller', () => {
  it('runs the task every interval after start', async () => {
    const task = vi.fn();
    const p = createPoller(task, 1000, deps());
    p.start();
    expect(task).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(3000);
    expect(task).toHaveBeenCalledTimes(5);
    p.stop();
  });

  it('stop halts polling and unsubscribes', async () => {
    const task = vi.fn();
    const p = createPoller(task, 1000, deps());
    p.start();
    expect(p.running).toBe(true);
    p.stop();
    expect(p.running).toBe(false);
    expect(unsubscribed).toBe(1);
    await vi.advanceTimersByTimeAsync(5000);
    expect(task).not.toHaveBeenCalled();
  });

  it('start is idempotent', async () => {
    const task = vi.fn();
    const p = createPoller(task, 1000, deps());
    p.start();
    p.start();
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(1);
    p.stop();
  });

  it('pauses while hidden and refreshes immediately on return', async () => {
    const task = vi.fn();
    const p = createPoller(task, 1000, deps());
    p.start();
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(1);
    setHidden(true);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(task).toHaveBeenCalledTimes(1);
    setHidden(false);
    await vi.advanceTimersByTimeAsync(0);
    expect(task).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(3);
    p.stop();
  });

  it('does not schedule when started hidden', async () => {
    hidden = true;
    const task = vi.fn();
    const p = createPoller(task, 1000, deps());
    p.start();
    await vi.advanceTimersByTimeAsync(5000);
    expect(task).not.toHaveBeenCalled();
    setHidden(false);
    await vi.advanceTimersByTimeAsync(0);
    expect(task).toHaveBeenCalledTimes(1);
    p.stop();
  });

  it('never overlaps a slow task', async () => {
    let active = 0;
    let maxActive = 0;
    const task = vi.fn(async () => {
      active += 1;
      maxActive = Math.max(maxActive, active);
      await new Promise((r) => setTimeout(r, 2500));
      active -= 1;
    });
    const p = createPoller(task, 1000, deps());
    p.start();
    await vi.advanceTimersByTimeAsync(1000); // tick 1 starts
    void p.trigger(); // joins the in-flight tick
    await vi.advanceTimersByTimeAsync(10_000);
    expect(maxActive).toBe(1);
    p.stop();
  });

  it('next tick is scheduled after the task finished (interval between runs)', async () => {
    const starts: number[] = [];
    const task = vi.fn(async () => {
      starts.push(Date.now());
      await new Promise((r) => setTimeout(r, 500));
    });
    const p = createPoller(task, 1000, deps());
    const t0 = Date.now();
    p.start();
    await vi.advanceTimersByTimeAsync(3900);
    expect(starts.map((s) => s - t0)).toEqual([1000, 2500]);
    p.stop();
  });

  it('trigger runs immediately and restarts the interval', async () => {
    const task = vi.fn();
    const p = createPoller(task, 1000, deps());
    p.start();
    await vi.advanceTimersByTimeAsync(800);
    await p.trigger();
    expect(task).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(800);
    expect(task).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(200);
    expect(task).toHaveBeenCalledTimes(2);
    p.stop();
  });

  it('trigger when stopped runs once without scheduling', async () => {
    const task = vi.fn();
    const p = createPoller(task, 1000, deps());
    await p.trigger();
    expect(task).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(5000);
    expect(task).toHaveBeenCalledTimes(1);
  });

  it('errors are reported and polling continues', async () => {
    const onError = vi.fn();
    const task = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValue(undefined);
    const p = createPoller(task, 1000, deps(), onError);
    p.start();
    await vi.advanceTimersByTimeAsync(1000);
    expect(onError).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(2);
    p.stop();
  });

  it('setInterval changes the cadence', async () => {
    const task = vi.fn();
    const p = createPoller(task, 10_000, deps());
    p.start();
    p.setInterval(1000);
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(1);
    p.setInterval(1000); // no-op
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(2);
    p.stop();
  });
});
