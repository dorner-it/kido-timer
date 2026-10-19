// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  HAND_TIME_SPREAD_WARN_MS,
  U32_MAX,
  clearHandTimesBody,
  formatTimeInput,
  handTimeMean,
  handTimeRawText,
  handTimesBody,
  isHandTime,
  parseTimeInput,
  roundToCentis,
  validateHandTimes,
} from '../hand-time';

// Parity with crates/kido-core/src/timing.rs — same examples as the Rust tests.
describe('handTimeMean (parity with kido_core::timing::hand_time_mean)', () => {
  it('doc example', () => {
    expect(handTimeMean(47_230, 47_245)).toBe(47_240);
    expect(handTimeMean(47_245, 47_230)).toBe(47_240);
  });

  it.each([
    [47_230, 47_230, 47_230],
    [47_234, 47_234, 47_230],
    [47_235, 47_235, 47_240],
    [47_230, 47_239, 47_230], // 47_234.5
    [47_230, 47_241, 47_240], // 47_235.5
    [47_230, 47_240, 47_240], // 47_235 exactly → up
    [47_230, 47_238, 47_230], // 47_234
    [0, 0, 0],
    [0, 9, 0], // 4.5
    [0, 10, 10], // 5
    [1, 0, 0], // 0.5
  ])('hand_time_mean(%i, %i) = %i', (a, b, want) => {
    expect(handTimeMean(a, b)).toBe(want);
  });

  it('large values, no overflow', () => {
    const big = 4_000_000_000;
    expect(handTimeMean(big, big)).toBe(big);
    expect(handTimeMean(big, big + 10)).toBe(big + 10);
    expect(handTimeMean(U32_MAX - 5, U32_MAX - 5)).toBe(4_294_967_290);
  });

  it('saturates at u32::MAX', () => {
    expect(handTimeMean(U32_MAX, U32_MAX)).toBe(U32_MAX);
  });

  it('is symmetric', () => {
    for (const [a, b] of [
      [47_231, 47_236],
      [61_002, 60_999],
      [12, 7],
    ]) {
      expect(handTimeMean(a!, b!)).toBe(handTimeMean(b!, a!));
    }
  });

  it('always returns a multiple of 10 (below saturation)', () => {
    for (let a = 47_200; a < 47_260; a += 3) {
      expect(handTimeMean(a, 47_233) % 10).toBe(0);
    }
  });

  it('rejects non-u32 inputs', () => {
    expect(() => handTimeMean(-1, 0)).toThrow(RangeError);
    expect(() => handTimeMean(0, 1.5)).toThrow(RangeError);
    expect(() => handTimeMean(U32_MAX + 1, 0)).toThrow(RangeError);
    expect(() => handTimeMean(Number.NaN, 0)).toThrow(RangeError);
  });
});

describe('roundToCentis (parity with kido_core::timing::round_to_centis)', () => {
  it.each([
    [0, 0],
    [4, 0],
    [5, 10],
    [9, 10],
    [10, 10],
    [47_230, 47_230],
    [47_234, 47_230],
    [47_235, 47_240],
    [47_239, 47_240],
    [99_995, 100_000],
    [U32_MAX, 4_294_967_300],
  ])('round_to_centis(%i) = %i', (ms, want) => {
    expect(roundToCentis(ms)).toBe(want);
  });

  it('rejects negative / fractional values', () => {
    expect(() => roundToCentis(-1)).toThrow(RangeError);
    expect(() => roundToCentis(1.2)).toThrow(RangeError);
  });
});

describe('parseTimeInput', () => {
  it.each([
    ['47,23', 47_230],
    ['47.23', 47_230],
    ['0:47,23', 47_230],
    ['0:47.23', 47_230],
    ['1:02,50', 62_500],
    ['1:02.5', 62_500],
    ['1:2,5', 62_500],
    ['62,5', 62_500],
    ['47', 47_000],
    ['47,2', 47_200],
    ['47,235', 47_235],
    ['  47 , 23 ', 47_230],
    ['10:00,00', 600_000],
    ['0,01', 10],
    ['125,40', 125_400],
  ])('%s → %i ms', (raw, ms) => {
    expect(parseTimeInput(raw)).toEqual({ ok: true, ms });
  });

  it.each(['', '   ', 'abc', '47,', ',23', '47,2345', '1:60,00', '1:123,00', '47;23', '-47,23', '1::2', '47,23,1', '1:02:03'])(
    'rejects %j',
    (raw) => {
      const r = parseTimeInput(raw);
      expect(r.ok).toBe(false);
      if (!r.ok) expect(r.error.length).toBeGreaterThan(0);
    },
  );

  it('rejects zero', () => {
    expect(parseTimeInput('0,00')).toEqual({ ok: false, error: 'Die Zeit muss größer als 0 sein.' });
    expect(parseTimeInput('0:00')).toMatchObject({ ok: false });
  });

  it('handles null / undefined', () => {
    expect(parseTimeInput(null).ok).toBe(false);
    expect(parseTimeInput(undefined).ok).toBe(false);
  });

  it('rejects values above u32', () => {
    expect(parseTimeInput('9999999,00').ok).toBe(false);
  });

  it('round-trips with formatTimeInput', () => {
    for (const ms of [10, 990, 47_230, 47_235, 59_990, 60_000, 62_500, 600_000]) {
      const r = parseTimeInput(formatTimeInput(ms));
      expect(r).toEqual({ ok: true, ms });
    }
  });
});

describe('formatTimeInput', () => {
  it.each([
    [47_230, '47,23'],
    [47_235, '47,235'],
    [7_050, '7,05'],
    [62_500, '1:02,50'],
    [600_000, '10:00,00'],
    [0, '0,00'],
    [-1_500, '-1,50'],
  ])('%i → %s', (ms, s) => {
    expect(formatTimeInput(ms)).toBe(s);
  });

  it('empty for null / NaN', () => {
    expect(formatTimeInput(null)).toBe('');
    expect(formatTimeInput(undefined)).toBe('');
    expect(formatTimeInput(Number.NaN)).toBe('');
  });
});

describe('validateHandTimes', () => {
  it('computes the mean preview', () => {
    const v = validateHandTimes('47,23', '47,245');
    expect(v).toEqual({
      ok: true,
      t1: 47_230,
      t2: 47_245,
      mean: 47_240,
      spreadMs: 15,
      warning: null,
    });
  });

  it('mixed formats', () => {
    const v = validateHandTimes('0:47.23', '47,25');
    expect(v.ok && v.mean).toBe(47_240);
  });

  it('reports per-field errors', () => {
    const v = validateHandTimes('', 'x');
    expect(v.ok).toBe(false);
    if (!v.ok) {
      expect(v.errors.t1).toBeTruthy();
      expect(v.errors.t2).toBeTruthy();
    }
    const v2 = validateHandTimes('47,23', '');
    expect(v2.ok).toBe(false);
    if (!v2.ok) {
      expect(v2.errors.t1).toBeUndefined();
      expect(v2.errors.t2).toBeTruthy();
    }
  });

  it('warns (but allows) on a large spread', () => {
    const v = validateHandTimes('47,00', '48,50');
    expect(v.ok).toBe(true);
    if (v.ok) {
      expect(v.spreadMs).toBe(1_500);
      expect(v.spreadMs).toBeGreaterThan(HAND_TIME_SPREAD_WARN_MS);
      expect(v.warning).toContain('1,50');
    }
  });

  it('no warning exactly at the threshold', () => {
    const v = validateHandTimes('47,00', '48,00');
    expect(v.ok && v.warning).toBeNull();
  });
});

describe('request bodies & display', () => {
  it('handTimesBody / clearHandTimesBody', () => {
    expect(handTimesBody(47_230, 47_245)).toEqual({ hand_time_1_ms: 47_230, hand_time_2_ms: 47_245 });
    expect(clearHandTimesBody()).toEqual({ clear: true });
  });

  it('isHandTime / handTimeRawText', () => {
    const lb = { time_source: 'lichtschranke' as const, hand_time_1_ms: null, hand_time_2_ms: null };
    const hs = { time_source: 'handstopp' as const, hand_time_1_ms: 47_230, hand_time_2_ms: 47_245 };
    expect(isHandTime(lb)).toBe(false);
    expect(isHandTime(hs)).toBe(true);
    expect(handTimeRawText(lb)).toBeNull();
    expect(handTimeRawText(hs)).toBe('47,23 / 47,245');
    expect(handTimeRawText({ ...hs, hand_time_2_ms: null })).toBe('47,23 / —');
  });
});
