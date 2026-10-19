// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Shared formatters for result times (Gruppenstafette 2027).
 *
 * Race times are shown at 1/100 s with a German decimal comma:
 * `m:ss,hh` from one minute on, `s,hh` below. Values are rounded half up
 * to 1/100 s — the same rule the backend uses for comparisons
 * (`kido_core::timing::round_to_centis`), so the shown time always
 * matches the ranking.
 */

/** Placeholder for a missing time. */
export const NO_TIME = '—';

/** Round milliseconds to 1/100 s (10 ms), half up. */
export function roundToCentis(ms: number): number {
  return Math.floor((ms + 5) / 10) * 10;
}

/**
 * `45000` → `"45,00"`, `62230` → `"1:02,23"`, `605010` → `"10:05,01"`.
 * `null`/`undefined`/non-finite → `empty` (default `"—"`). Negative
 * values keep a leading minus.
 */
export function formatRaceTime(ms: number | null | undefined, empty: string = NO_TIME): string {
  if (ms == null || !Number.isFinite(ms)) return empty;
  const sign = ms < 0 ? '-' : '';
  const centisTotal = roundToCentis(Math.abs(ms)) / 10;
  const minutes = Math.floor(centisTotal / 6000);
  const seconds = Math.floor((centisTotal % 6000) / 100);
  const hundredths = Math.round(centisTotal % 100);
  const hh = hundredths.toString().padStart(2, '0');
  if (minutes > 0) {
    return `${sign}${minutes}:${seconds.toString().padStart(2, '0')},${hh}`;
  }
  return `${sign}${seconds},${hh}`;
}

/**
 * Fixed decimals with a German comma: `formatDecimalDE(12.5, 1)` →
 * `"12,5"`, `formatDecimalDE(0.89, 4)` → `"0,8900"`. Non-finite → `"—"`.
 */
export function formatDecimalDE(value: number | null | undefined, digits: number): string {
  if (value == null || !Number.isFinite(value)) return NO_TIME;
  return value.toFixed(digits).replace('.', ',');
}

/** `"2027-06-12"` → `"12.06.2027"`; unparsable input is returned as is. */
export function formatDateNumericDE(isoDate: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(isoDate);
  if (!m) return isoDate;
  return `${m[3]}.${m[2]}.${m[1]}`;
}

/**
 * RFC 3339 timestamp → `"12.06.2027, 14:03:11"` in the viewer's local
 * time zone (`de-DE`). Unparsable input is returned as is.
 */
export function formatTimestampDE(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString('de-DE', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}
