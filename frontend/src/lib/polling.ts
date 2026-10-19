// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Small polling scheduler for the organizer review: refreshes every few
 * seconds while a run is current, pauses while the tab is hidden and
 * refreshes immediately when it becomes visible again. Never runs two
 * ticks at once. Dependencies are injectable for tests.
 */

/** While at least one run is current (review is live). */
export const POLL_LIVE_MS = 2500;
/** Nothing current: slow background refresh. */
export const POLL_IDLE_MS = 15000;

export function pollIntervalMs(hasCurrentRuns: boolean): number {
  return hasCurrentRuns ? POLL_LIVE_MS : POLL_IDLE_MS;
}

export interface PollerDeps {
  setTimeout: (fn: () => void, ms: number) => unknown;
  clearTimeout: (handle: unknown) => void;
  isHidden: () => boolean;
  /** Subscribe to visibility changes; returns an unsubscribe function. */
  onVisibilityChange: (cb: () => void) => () => void;
}

export interface Poller {
  start(): void;
  stop(): void;
  /** Run now (e.g. after an action) and restart the interval. */
  trigger(): Promise<void>;
  setInterval(ms: number): void;
  readonly running: boolean;
}

export function browserPollerDeps(): PollerDeps {
  return {
    setTimeout: (fn, ms) => globalThis.setTimeout(fn, ms),
    clearTimeout: (h) => globalThis.clearTimeout(h as ReturnType<typeof setTimeout>),
    isHidden: () => typeof document !== 'undefined' && document.visibilityState === 'hidden',
    onVisibilityChange: (cb) => {
      if (typeof document === 'undefined') return () => {};
      document.addEventListener('visibilitychange', cb);
      return () => document.removeEventListener('visibilitychange', cb);
    },
  };
}

export function createPoller(
  task: () => Promise<void> | void,
  intervalMs: number,
  deps: PollerDeps = browserPollerDeps(),
  onError: (e: unknown) => void = () => {},
): Poller {
  let interval = intervalMs;
  let timer: unknown = null;
  let running = false;
  let inFlight: Promise<void> | null = null;
  let unsubscribe: (() => void) | null = null;

  const clear = () => {
    if (timer != null) {
      deps.clearTimeout(timer);
      timer = null;
    }
  };

  const schedule = () => {
    clear();
    if (!running || deps.isHidden()) return;
    timer = deps.setTimeout(() => {
      timer = null;
      void tick();
    }, interval);
  };

  const tick = async (): Promise<void> => {
    if (!running) return;
    if (deps.isHidden()) return; // resumes on visibilitychange
    if (inFlight) return inFlight;
    inFlight = (async () => {
      try {
        await task();
      } catch (e) {
        onError(e);
      } finally {
        inFlight = null;
      }
    })();
    await inFlight;
    schedule();
  };

  const onVisibility = () => {
    if (!running) return;
    if (deps.isHidden()) clear();
    else void tick();
  };

  return {
    start() {
      if (running) return;
      running = true;
      unsubscribe = deps.onVisibilityChange(onVisibility);
      schedule();
    },
    stop() {
      running = false;
      clear();
      unsubscribe?.();
      unsubscribe = null;
    },
    async trigger() {
      if (!running) {
        try {
          await task();
        } catch (e) {
          onError(e);
        }
        return;
      }
      clear();
      await tick();
    },
    setInterval(ms: number) {
      if (ms === interval) return;
      interval = ms;
      if (running && timer != null) schedule();
    },
    get running() {
      return running;
    },
  };
}
