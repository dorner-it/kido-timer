// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  NO_TIME,
  formatDateNumericDE,
  formatDecimalDE,
  formatRaceTime,
  formatTimestampDE,
  roundToCentis,
} from '../time-format';

describe('roundToCentis', () => {
  it.each([
    [0, 0],
    [4, 0],
    [5, 10],
    [9, 10],
    [47_234, 47_230],
    [47_235, 47_240],
    [47_239, 47_240],
    [59_995, 60_000],
  ])('%i ms → %i ms', (input, expected) => {
    expect(roundToCentis(input)).toBe(expected);
  });
});

describe('formatRaceTime', () => {
  it.each([
    [0, '0,00'],
    [10, '0,01'],
    [990, '0,99'],
    [5_000, '5,00'],
    [9_870, '9,87'],
    [45_000, '45,00'],
    [47_230, '47,23'],
    [59_990, '59,99'],
    [60_000, '1:00,00'],
    [62_230, '1:02,23'],
    [75_000, '1:15,00'],
    [119_990, '1:59,99'],
    [605_010, '10:05,01'],
    [3_600_000, '60:00,00'],
  ])('%i ms → %s', (ms, expected) => {
    expect(formatRaceTime(ms)).toBe(expected);
  });

  it('rounds half up to 1/100 s like the backend', () => {
    expect(formatRaceTime(47_234)).toBe('47,23');
    expect(formatRaceTime(47_235)).toBe('47,24');
    expect(formatRaceTime(47_239)).toBe('47,24');
  });

  it('carries rounding into seconds and minutes', () => {
    expect(formatRaceTime(9_995)).toBe('10,00');
    expect(formatRaceTime(59_995)).toBe('1:00,00');
    expect(formatRaceTime(119_996)).toBe('2:00,00');
  });

  it('pads seconds only from one minute on', () => {
    expect(formatRaceTime(61_050)).toBe('1:01,05');
    expect(formatRaceTime(1_050)).toBe('1,05');
  });

  it('uses a decimal comma, never a dot', () => {
    expect(formatRaceTime(47_230)).not.toContain('.');
  });

  it('missing values', () => {
    expect(formatRaceTime(null)).toBe(NO_TIME);
    expect(formatRaceTime(undefined)).toBe(NO_TIME);
    expect(formatRaceTime(Number.NaN)).toBe(NO_TIME);
    expect(formatRaceTime(Number.POSITIVE_INFINITY)).toBe(NO_TIME);
    expect(formatRaceTime(null, 'n.a.')).toBe('n.a.');
  });

  it('negative values keep a minus', () => {
    expect(formatRaceTime(-1_500)).toBe('-1,50');
    expect(formatRaceTime(-61_000)).toBe('-1:01,00');
  });

  it('fractional milliseconds', () => {
    expect(formatRaceTime(47_234.9)).toBe('47,23');
    expect(formatRaceTime(47_235.2)).toBe('47,24');
  });
});

describe('formatDecimalDE', () => {
  it.each([
    [12.5, 1, '12,5'],
    [12, 1, '12,0'],
    [11.25, 1, '11,3'],
    [0.89, 4, '0,8900'],
    [1, 4, '1,0000'],
    [0.8867, 4, '0,8867'],
  ])('%d with %i digits → %s', (value, digits, expected) => {
    expect(formatDecimalDE(value, digits)).toBe(expected);
  });

  it('missing values', () => {
    expect(formatDecimalDE(null, 1)).toBe(NO_TIME);
    expect(formatDecimalDE(undefined, 1)).toBe(NO_TIME);
    expect(formatDecimalDE(Number.NaN, 1)).toBe(NO_TIME);
  });
});

describe('formatDateNumericDE', () => {
  it('formats ISO dates', () => {
    expect(formatDateNumericDE('2027-06-12')).toBe('12.06.2027');
    expect(formatDateNumericDE('2027-12-01')).toBe('01.12.2027');
  });

  it('returns unparsable input unchanged', () => {
    expect(formatDateNumericDE('12.06.2027')).toBe('12.06.2027');
    expect(formatDateNumericDE('')).toBe('');
  });
});

describe('formatTimestampDE', () => {
  it('formats RFC 3339 timestamps with date and seconds', () => {
    const out = formatTimestampDE('2027-06-12T10:03:11Z');
    expect(out).toMatch(/^\d{2}\.\d{2}\.\d{4}, \d{2}:\d{2}:\d{2}$/);
    expect(out.startsWith('12.06.2027') || out.startsWith('11.06.2027') || out.startsWith('13.06.2027')).toBe(true);
    expect(out.endsWith(':11')).toBe(true);
  });

  it('returns unparsable input unchanged', () => {
    expect(formatTimestampDE('kein Datum')).toBe('kein Datum');
  });
});
