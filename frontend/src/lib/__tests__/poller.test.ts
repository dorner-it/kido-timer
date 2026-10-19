// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { startPolling, type VisibilitySource } from '../poller';

class FakeDocument extends EventTarget implements VisibilitySource {
  hidden = false;
  setHidden(hidden: boolean) {
    this.hidden = hidden;
    this.dispatchEvent(new Event('visibilitychange'));
  }
  override addEventListener(type: 'visibilitychange', listener: () => void): void {
    super.addEventListener(type, listener);
  }
  override removeEventListener(type: 'visibilitychange', listener: () => void): void {
    super.removeEventListener(type, listener);
  }
}

async function flush() {
  // Let the awaited task and the finally-block settle.
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

describe('startPolling', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('runs immediately and then every interval', async () => {
    const task = vi.fn();
    const stop = startPolling(task, { intervalMs: 5000, visibility: null });
    await flush();
    expect(task).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(4999);
    expect(task).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(task).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(5000);
    expect(task).toHaveBeenCalledTimes(3);
    stop();
  });

  it('immediate: false waits one interval', async () => {
    const task = vi.fn();
    const stop = startPolling(task, { intervalMs: 1000, visibility: null, immediate: false });
    await flush();
    expect(task).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(1);
    stop();
  });

  it('stop() cancels further runs', async () => {
    const task = vi.fn();
    const stop = startPolling(task, { intervalMs: 1000, visibility: null });
    await flush();
    stop();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(task).toHaveBeenCalledTimes(1);
  });

  it('never overlaps a slow task', async () => {
    let resolve: () => void = () => {};
    const task = vi.fn(
      () =>
        new Promise<void>((r) => {
          resolve = r;
        }),
    );
    const stop = startPolling(task, { intervalMs: 1000, visibility: null });
    await flush();
    await vi.advanceTimersByTimeAsync(5000);
    expect(task).toHaveBeenCalledTimes(1);
    resolve();
    await flush();
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(2);
    stop();
  });

  it('keeps polling after a failing task', async () => {
    const task = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValue(undefined);
    const stop = startPolling(task, { intervalMs: 1000, visibility: null });
    await flush();
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(2);
    stop();
  });

  it('pauses while hidden and refreshes on becoming visible', async () => {
    const doc = new FakeDocument();
    const task = vi.fn();
    const stop = startPolling(task, { intervalMs: 1000, visibility: doc });
    await flush();
    expect(task).toHaveBeenCalledTimes(1);

    doc.setHidden(true);
    await vi.advanceTimersByTimeAsync(10_000);
    expect(task).toHaveBeenCalledTimes(1);

    doc.setHidden(false);
    await flush();
    expect(task).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1000);
    expect(task).toHaveBeenCalledTimes(3);
    stop();
  });

  it('does not start while the page is hidden', async () => {
    const doc = new FakeDocument();
    doc.hidden = true;
    const task = vi.fn();
    const stop = startPolling(task, { intervalMs: 1000, visibility: doc });
    await vi.advanceTimersByTimeAsync(5000);
    expect(task).not.toHaveBeenCalled();
    doc.setHidden(false);
    await flush();
    expect(task).toHaveBeenCalledTimes(1);
    stop();
  });

  it('stop() removes the visibility listener', async () => {
    const doc = new FakeDocument();
    const task = vi.fn();
    const stop = startPolling(task, { intervalMs: 1000, visibility: doc });
    await flush();
    stop();
    doc.setHidden(true);
    doc.setHidden(false);
    await flush();
    expect(task).toHaveBeenCalledTimes(1);
  });
});
