// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Visibility-aware polling (auto-refresh of the results page).
 *
 * - Runs `task`, waits `intervalMs` after it settles, runs it again — so
 *   slow requests never overlap.
 * - Pauses while the page is hidden (background tab, minimized window)
 *   and refreshes immediately when it becomes visible again.
 * - Errors thrown by `task` are swallowed (the caller shows its own
 *   error state); polling continues.
 */

export interface VisibilitySource {
  readonly hidden: boolean;
  addEventListener(type: 'visibilitychange', listener: () => void): void;
  removeEventListener(type: 'visibilitychange', listener: () => void): void;
}

export interface PollingOptions {
  intervalMs: number;
  /** Defaults to `document` in the browser; `null` = always visible. */
  visibility?: VisibilitySource | null;
  /** Run once right away (default `true`). */
  immediate?: boolean;
}

/** Start polling; returns a stop function. */
export function startPolling(
  task: () => unknown | Promise<unknown>,
  options: PollingOptions,
): () => void {
  const visibility =
    options.visibility !== undefined
      ? options.visibility
      : typeof document !== 'undefined'
        ? (document as VisibilitySource)
        : null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let running = false;
  let stopped = false;

  const isHidden = () => visibility?.hidden === true;

  const clear = () => {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  };

  const schedule = () => {
    clear();
    if (stopped || isHidden()) return;
    timer = setTimeout(() => {
      timer = null;
      void run();
    }, options.intervalMs);
  };

  const run = async () => {
    if (stopped || running || isHidden()) return;
    running = true;
    try {
      await task();
    } catch {
      // Caller handles errors; keep polling.
    } finally {
      running = false;
    }
    schedule();
  };

  const onVisibility = () => {
    if (isHidden()) {
      clear();
    } else {
      void run();
    }
  };

  visibility?.addEventListener('visibilitychange', onVisibility);
  if (options.immediate ?? true) {
    void run();
  } else {
    schedule();
  }

  return () => {
    stopped = true;
    clear();
    visibility?.removeEventListener('visibilitychange', onVisibility);
  };
}
