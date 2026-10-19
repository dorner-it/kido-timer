// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Hand times (two stopwatches) — only entered when the light barrier
 * failed. Mirrors `kido_core::timing` exactly so the preview in the
 * organizer UI equals what the backend stores.
 */

import type { HandTimesInput, Run } from './api';

/** Largest value of a Rust `u32` (wire type of the hand times). */
export const U32_MAX = 4_294_967_295;

function assertU32(name: string, v: number): void {
  if (!Number.isInteger(v) || v < 0 || v > U32_MAX) {
    throw new RangeError(`${name} muss eine ganze Zahl zwischen 0 und ${U32_MAX} sein`);
  }
}

/**
 * Mean of the two hand times, rounded to 1/100 s (10 ms), half up.
 * Exact mirror of `kido_core::timing::hand_time_mean`:
 * `centis = (t1 + t2 + 10) / 20` (integer division), result saturates
 * at `u32::MAX`.
 *
 * Example: 47 230 and 47 245 → 47 240 (47 237.5 → 47 240).
 */
export function handTimeMean(t1Ms: number, t2Ms: number): number {
  assertU32('t1', t1Ms);
  assertU32('t2', t2Ms);
  // sum ≤ 2^33, safely representable as a JS number.
  const sum = t1Ms + t2Ms;
  const centis = Math.floor((sum + 10) / 20);
  return Math.min(centis * 10, U32_MAX);
}

/**
 * Round a millisecond value to 1/100 s, half up. Mirror of
 * `kido_core::timing::round_to_centis` for the range of JS safe integers.
 */
export function roundToCentis(ms: number): number {
  if (!Number.isInteger(ms) || ms < 0) {
    throw new RangeError('ms muss eine nicht-negative ganze Zahl sein');
  }
  const centis = Math.floor(ms / 10) + (ms % 10 >= 5 ? 1 : 0);
  return centis * 10;
}

export type ParseTimeResult = { ok: true; ms: number } | { ok: false; error: string };

// [m:]ss[,hh] — minutes optional, separator comma or dot, 1–3 fraction digits.
const TIME_RE = /^(?:(\d{1,3}):)?(\d+)(?:[.,](\d{1,3}))?$/;

/**
 * Parse a stopwatch reading typed by the organizer.
 *
 * Accepted: `47,23` · `47.23` · `0:47,23` · `1:02,50` · `62,5` (= 62.50 s)
 * · `47` (= 47.00 s) · `47,235` (ms). With minutes the seconds must be
 * 0–59. Whitespace is ignored. Zero is rejected (the backend requires
 * hand times > 0).
 */
export function parseTimeInput(raw: string | null | undefined): ParseTimeResult {
  const s = (raw ?? '').replace(/\s+/g, '');
  if (s === '') return { ok: false, error: 'Bitte eine Zeit eingeben.' };
  const m = TIME_RE.exec(s);
  if (!m) {
    return { ok: false, error: 'Format: m:ss,hh oder ss,hh (z.B. 47,23 oder 1:02,50).' };
  }
  const [, minStr, secStr, fracStr] = m;
  const minutes = minStr != null ? parseInt(minStr, 10) : 0;
  const seconds = parseInt(secStr!, 10);
  if (minStr != null && (secStr!.length > 2 || seconds > 59)) {
    return { ok: false, error: 'Mit Minuten müssen die Sekunden zwischen 0 und 59 liegen.' };
  }
  let fracMs = 0;
  if (fracStr != null) {
    fracMs = parseInt(fracStr.padEnd(3, '0'), 10);
  }
  const ms = (minutes * 60 + seconds) * 1000 + fracMs;
  if (!Number.isSafeInteger(ms) || ms > U32_MAX) {
    return { ok: false, error: 'Zeit ist zu groß.' };
  }
  if (ms <= 0) return { ok: false, error: 'Die Zeit muss größer als 0 sein.' };
  return { ok: true, ms };
}

/**
 * Input-style formatting with a comma: `47,23`, `1:02,50`. Hundredths
 * are shown; a value with a non-zero ms digit keeps all three digits
 * (`47,235`) so nothing typed is hidden.
 */
export function formatTimeInput(ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return '';
  const abs = Math.abs(Math.trunc(ms));
  const sign = ms < 0 ? '-' : '';
  const minutes = Math.floor(abs / 60_000);
  const seconds = Math.floor((abs % 60_000) / 1000);
  const frac = abs % 1000;
  const fracStr = frac % 10 === 0 ? String(frac / 10).padStart(2, '0') : String(frac).padStart(3, '0');
  if (minutes > 0) {
    return `${sign}${minutes}:${String(seconds).padStart(2, '0')},${fracStr}`;
  }
  return `${sign}${seconds},${fracStr}`;
}

/** Above this spread between the two stopwatches the UI shows a hint. */
export const HAND_TIME_SPREAD_WARN_MS = 1000;

export type HandTimesValidation =
  | {
      ok: true;
      t1: number;
      t2: number;
      mean: number;
      spreadMs: number;
      /** Soft hint (does not block saving). */
      warning: string | null;
    }
  | { ok: false; errors: { t1?: string; t2?: string } };

/** Validate both inputs and compute the preview mean. */
export function validateHandTimes(raw1: string, raw2: string): HandTimesValidation {
  const p1 = parseTimeInput(raw1);
  const p2 = parseTimeInput(raw2);
  if (!p1.ok || !p2.ok) {
    const errors: { t1?: string; t2?: string } = {};
    if (!p1.ok) errors.t1 = p1.error;
    if (!p2.ok) errors.t2 = p2.error;
    return { ok: false, errors };
  }
  const spreadMs = Math.abs(p1.ms - p2.ms);
  return {
    ok: true,
    t1: p1.ms,
    t2: p2.ms,
    mean: handTimeMean(p1.ms, p2.ms),
    spreadMs,
    warning:
      spreadMs > HAND_TIME_SPREAD_WARN_MS
        ? `Die beiden Handzeiten weichen um ${formatTimeInput(spreadMs)} s ab – bitte prüfen.`
        : null,
  };
}

/** Body for `PUT …/runs/{run_id}/hand-times`. */
export function handTimesBody(t1Ms: number, t2Ms: number): HandTimesInput {
  return { hand_time_1_ms: t1Ms, hand_time_2_ms: t2Ms };
}

/** Body for "Handzeit entfernen". */
export function clearHandTimesBody(): HandTimesInput {
  return { clear: true };
}

/** Whether the run's time comes from the two stopwatches. */
export function isHandTime(run: Pick<Run, 'time_source'>): boolean {
  return run.time_source === 'handstopp';
}

/** `"47,23 / 47,25"` — both raw readings, or `null` when not a hand time. */
export function handTimeRawText(
  run: Pick<Run, 'time_source' | 'hand_time_1_ms' | 'hand_time_2_ms'>,
): string | null {
  if (!isHandTime(run)) return null;
  const a = run.hand_time_1_ms != null ? formatTimeInput(run.hand_time_1_ms) : '—';
  const b = run.hand_time_2_ms != null ? formatTimeInput(run.hand_time_2_ms) : '—';
  return `${a} / ${b}`;
}
