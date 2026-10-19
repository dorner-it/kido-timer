// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  WERTUNGSGRUPPEN,
  WERTUNGSGRUPPE_DESCRIPTIONS,
  WERTUNGSGRUPPE_LABELS,
  WG_MISSING_LABEL,
  formatWgHistoryEntry,
  formatWgWarning,
  formatWgWarnings,
  isWertungsgruppe,
  runnerDisplayName,
  wgLabel,
  wgOptionLabel,
  wgWarningTooltip,
} from '../wg';

const runners = new Map([
  ['r1', { id: 'r1', first_name: 'Max', last_name: 'Muster' }],
  ['r2', { id: 'r2', first_name: 'Erika', last_name: 'Beispiel' }],
]);

describe('WG labels', () => {
  it('lists the five WGs in order', () => {
    expect(WERTUNGSGRUPPEN).toEqual(['wg1', 'wg2', 'wg3', 'wg4', 'wg5']);
  });

  it('short labels', () => {
    expect(Object.values(WERTUNGSGRUPPE_LABELS)).toEqual(['WG1', 'WG2', 'WG3', 'WG4', 'WG5']);
  });

  it('long labels match the FBL wording', () => {
    expect(WERTUNGSGRUPPE_DESCRIPTIONS).toEqual({
      wg1: 'Jungen + Mädchen 8–10',
      wg2: 'Jungen bis 14',
      wg3: 'Mädchen bis 14',
      wg4: 'Jungen bis 18',
      wg5: 'Mädchen bis 18',
    });
  });

  it('wgOptionLabel combines both', () => {
    expect(wgOptionLabel('wg1')).toBe('WG1 – Jungen + Mädchen 8–10');
    expect(wgOptionLabel('wg5')).toBe('WG5 – Mädchen bis 18');
  });

  it('wgLabel falls back for null / unknown', () => {
    expect(wgLabel('wg3')).toBe('WG3');
    expect(wgLabel(null)).toBe('—');
    expect(wgLabel('wg9', WG_MISSING_LABEL)).toBe('WG fehlt');
  });

  it('isWertungsgruppe', () => {
    expect(isWertungsgruppe('wg2')).toBe(true);
    expect(isWertungsgruppe('WG2')).toBe(false);
    expect(isWertungsgruppe(null)).toBe(false);
  });
});

describe('runnerDisplayName', () => {
  it('joins first and last name', () => {
    expect(runnerDisplayName(runners.get('r1'))).toBe('Max Muster');
  });
  it('falls back for unknown runners', () => {
    expect(runnerDisplayName(undefined)).toBe('Unbekannter Läufer');
    expect(runnerDisplayName({ id: 'x', first_name: ' ', last_name: '' })).toBe(
      'Unbekannter Läufer',
    );
  });
});

describe('WG warnings', () => {
  it('formats a single warning', () => {
    expect(
      formatWgWarning({ runner_id: 'r1', age: 15, wg: 'wg2', reason: 'zu alt' }, runners),
    ).toBe('Max Muster: 15 J. – zu alt für WG2');
  });

  it('formats "zu jung"', () => {
    expect(
      formatWgWarning({ runner_id: 'r2', age: 7, wg: 'wg1', reason: 'zu jung' }, runners),
    ).toBe('Erika Beispiel: 7 J. – zu jung für WG1');
  });

  it('uses a fallback name and reason', () => {
    expect(formatWgWarning({ runner_id: 'zz', age: 19, wg: 'wg4', reason: '' }, runners)).toBe(
      'Unbekannter Läufer: 19 J. – passt nicht für WG4',
    );
  });

  it('formats lists and tolerates null', () => {
    expect(
      formatWgWarnings(
        [
          { runner_id: 'r1', age: 15, wg: 'wg2', reason: 'zu alt' },
          { runner_id: 'r2', age: 16, wg: 'wg2', reason: 'zu alt' },
        ],
        runners,
      ),
    ).toEqual(['Max Muster: 15 J. – zu alt für WG2', 'Erika Beispiel: 16 J. – zu alt für WG2']);
    expect(formatWgWarnings(null, runners)).toEqual([]);
    expect(formatWgWarnings(undefined, runners)).toEqual([]);
  });

  it('tooltip has a header line and is empty without warnings', () => {
    expect(
      wgWarningTooltip([{ runner_id: 'r1', age: 15, wg: 'wg2', reason: 'zu alt' }], runners),
    ).toBe('Alter passt nicht zur Wertungsgruppe:\nMax Muster: 15 J. – zu alt für WG2');
    expect(wgWarningTooltip([], runners)).toBe('');
  });
});

describe('formatWgHistoryEntry', () => {
  it('includes from → to, who, when and note', () => {
    const line = formatWgHistoryEntry({
      from: 'wg2',
      to: 'wg1',
      by: 'orga@example.de',
      at: '2027-05-10T09:15:00Z',
      note: '2 Mädchen',
    });
    expect(line.startsWith('WG2 → WG1 · von orga@example.de · ')).toBe(true);
    expect(line).toContain('2027');
    expect(line.endsWith(' · 2 Mädchen')).toBe(true);
  });

  it('omits empty parts and shows "WG fehlt" for a null origin', () => {
    expect(formatWgHistoryEntry({ from: null, to: 'wg3', by: '', at: '', note: null })).toBe(
      'WG fehlt → WG3',
    );
  });

  it('keeps an unparsable timestamp verbatim', () => {
    expect(
      formatWgHistoryEntry({ from: 'wg4', to: 'wg5', by: 'sub-1', at: 'gestern', note: '  ' }),
    ).toBe('WG4 → WG5 · von sub-1 · gestern');
  });
});
