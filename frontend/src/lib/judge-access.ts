// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * "Zugang ungültig" state of the judge (Strafrichter) app.
 *
 * Both backends (cloud `backend/src/routes/decider.rs` and the desktop
 * on-prem server `desktop/src-tauri/src/local/judge.rs`) answer a wrong
 * or rotated judge token ("Neuer Zugangscode") with **401**. That is
 * final for this link: polling stops and the judge sees a full-screen
 * hint instead of being redirected to the organizer `/login`.
 *
 * 404 ("no active event" / owner not loaded) is *not* treated as invalid
 * access — it is the normal "Wettkampfleitung hat noch nichts aktiv"
 * state and the app keeps polling until an event becomes active.
 *
 * Penalty submits still queued locally are never dropped on 401: they
 * stay in storage (see `penalty-queue.ts`) and the screen counts them so
 * the judge can report them to the Wettkampfleitung.
 */

import { ApiError } from './api';

/** HTTP status codes that mean "this judge link is no longer valid". */
export const INVALID_ACCESS_STATUSES: readonly number[] = [401];

export const INVALID_ACCESS_TITLE = 'Zugang ungültig';
export const INVALID_ACCESS_TEXT =
  'Dieser Strafrichter-Zugang ist nicht mehr gültig. Bitte hol dir bei der Wettkampfleitung einen neuen QR-Code.';

export function isInvalidAccessStatus(status: number | null | undefined): boolean {
  return typeof status === 'number' && INVALID_ACCESS_STATUSES.includes(status);
}

export function isInvalidAccessError(err: unknown): boolean {
  return err instanceof ApiError && isInvalidAccessStatus(err.status);
}

/** `null` for 0, else "1 Meldung konnte …" / "N Meldungen konnten …". */
export function unsentMessage(count: number): string | null {
  if (!Number.isFinite(count) || count <= 0) return null;
  return count === 1
    ? '1 Meldung konnte nicht gesendet werden.'
    : `${count} Meldungen konnten nicht gesendet werden.`;
}

/** Queued items belonging to this judge link (owner + token). */
export function countUnsentForAccess(
  items: readonly { owner_sub: string; token: string }[],
  ownerSub: string,
  token: string,
): number {
  return items.filter((i) => i.owner_sub === ownerSub && i.token === token).length;
}

export interface JudgePollingOptions {
  /** Loads the judge state; a thrown 401 `ApiError` stops polling. */
  load: () => Promise<unknown>;
  intervalMs: number;
  /** Called once when the access turned out to be invalid. */
  onInvalid: () => void;
  /** Any other failure (network, 404, 5xx) — polling continues. */
  onError?: (err: unknown) => void;
}

/**
 * Loads right away, then every `intervalMs`. Stops for good on an
 * invalid-access error (no retry loop). Returns a stop function.
 */
export function startJudgePolling(opts: JudgePollingOptions): () => void {
  let stopped = false;
  let timer: ReturnType<typeof setInterval> | null = null;

  const stop = () => {
    stopped = true;
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  };

  const tick = async () => {
    if (stopped) return;
    try {
      await opts.load();
    } catch (err) {
      if (stopped) return;
      if (isInvalidAccessError(err)) {
        stop();
        opts.onInvalid();
        return;
      }
      opts.onError?.(err);
    }
  };

  void tick();
  if (!stopped) timer = setInterval(() => void tick(), opts.intervalMs);
  return stop;
}
