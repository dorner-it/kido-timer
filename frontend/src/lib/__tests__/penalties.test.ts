// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import type { CatalogueEntry } from '../api';
import {
  PENALTY_KIND_LABELS,
  PENALTY_KIND_SHORT,
  SZ_DELTA_MS,
  catalogueForStation,
  formatDelta,
  indexCatalogue,
  isPenaltyKind,
  penaltyDeltaText,
  penaltyDescription,
  penaltyKind,
  penaltyKindBadgeClasses,
  penaltySummaryLine,
} from '../penalties';
import { stationLabel } from '../stations';

const CATALOGUE: CatalogueEntry[] = [
  {
    code: 'knotengestell.ein_knoten_falsch',
    station: 'knotengestell',
    kind: 'sz',
    label_de: '1 Knoten falsch',
  },
  {
    code: 'knotengestell.mehrere_knoten_falsch',
    station: 'knotengestell',
    kind: 'fv',
    label_de: 'Mehrere Knoten falsch',
  },
  {
    code: 'angriffslinie.nicht_bekaempfte_ziele',
    station: 'angriffslinie',
    kind: 'fv',
    label_de: 'Nicht bekämpfte Ziele',
  },
];
const idx = indexCatalogue(CATALOGUE);

describe('kind', () => {
  it('labels', () => {
    expect(PENALTY_KIND_LABELS).toEqual({ sz: 'SZ (+15 s)', fv: 'FV (ungültig)' });
    expect(PENALTY_KIND_SHORT).toEqual({ sz: 'SZ', fv: 'FV' });
    expect(SZ_DELTA_MS).toBe(15_000);
  });

  it('uses an explicit kind', () => {
    expect(penaltyKind({ kind: 'fv', is_disqualification: false })).toBe('fv');
    expect(penaltyKind({ kind: 'sz', is_disqualification: true })).toBe('sz');
  });

  it('migrates missing kind from is_disqualification', () => {
    expect(penaltyKind({ is_disqualification: true })).toBe('fv');
    expect(penaltyKind({ is_disqualification: false })).toBe('sz');
    expect(penaltyKind({})).toBe('sz');
    expect(penaltyKind({ kind: 'bogus', is_disqualification: true })).toBe('fv');
  });

  it('isPenaltyKind', () => {
    expect(isPenaltyKind('sz')).toBe(true);
    expect(isPenaltyKind('dq')).toBe(false);
  });

  it('badge classes differ per kind', () => {
    expect(penaltyKindBadgeClasses('fv')).toContain('status-unknown');
    expect(penaltyKindBadgeClasses('sz')).toContain('status-armed');
  });
});

describe('catalogue helpers', () => {
  it('indexes by code', () => {
    expect(idx.get('knotengestell.ein_knoten_falsch')?.label_de).toBe('1 Knoten falsch');
    expect(indexCatalogue(null).size).toBe(0);
  });

  it('filters by station in order', () => {
    expect(catalogueForStation(CATALOGUE, 'knotengestell').map((e) => e.code)).toEqual([
      'knotengestell.ein_knoten_falsch',
      'knotengestell.mehrere_knoten_falsch',
    ]);
    expect(catalogueForStation(CATALOGUE, 'podest')).toEqual([]);
    expect(catalogueForStation(CATALOGUE, null)).toEqual([]);
    expect(catalogueForStation(null, 'knotengestell')).toEqual([]);
  });
});

describe('penaltyDescription', () => {
  it('shows the catalogue label when the code is known', () => {
    expect(
      penaltyDescription(
        { code: 'knotengestell.ein_knoten_falsch', reason_code: null, delta_ms: 15000 },
        idx,
      ),
    ).toBe('1 Knoten falsch');
  });

  it('shows the raw code when the catalogue does not know it', () => {
    expect(
      penaltyDescription({ code: 'startlinie.neu', reason_code: 'x', delta_ms: 15000 }, idx),
    ).toBe('startlinie.neu');
  });

  it('falls back to reason_code on legacy rows', () => {
    expect(penaltyDescription({ code: null, reason_code: 'uebergabe', delta_ms: 10000 }, idx)).toBe(
      'uebergabe',
    );
  });

  it('falls back to the delta, or FV label', () => {
    expect(penaltyDescription({ code: null, reason_code: null, delta_ms: 5000 }, idx)).toBe(
      '+5.00',
    );
    expect(
      penaltyDescription(
        { code: null, reason_code: null, delta_ms: 0, is_disqualification: true },
        idx,
      ),
    ).toBe('FV (ungültig)');
  });
});

describe('delta text', () => {
  it('formatDelta', () => {
    expect(formatDelta(15000)).toBe('+15.00');
    expect(formatDelta(-2500)).toBe('-2.50');
    expect(formatDelta(60000)).toBe('+1:00.00');
    expect(formatDelta(0)).toBe('+0.00');
  });

  it('FV has no delta, SZ shows its delta', () => {
    expect(penaltyDeltaText({ kind: 'fv', code: null, reason_code: null, delta_ms: 0 })).toBe('—');
    expect(penaltyDeltaText({ kind: 'sz', code: null, reason_code: null, delta_ms: 15000 })).toBe(
      '+15.00',
    );
    expect(
      penaltyDeltaText({ code: null, reason_code: null, delta_ms: 999000, is_disqualification: true }),
    ).toBe('—');
  });
});

describe('penaltySummaryLine', () => {
  const label = (s: Parameters<typeof stationLabel>[0]) => stationLabel(s, 'ohne Station');

  it('SZ with catalogue label', () => {
    expect(
      penaltySummaryLine(
        {
          kind: 'sz',
          code: 'knotengestell.ein_knoten_falsch',
          reason_code: null,
          delta_ms: 15000,
          station: 'knotengestell',
        },
        idx,
        label,
      ),
    ).toBe('• Knotengestell: SZ +15.00 – 1 Knoten falsch');
  });

  it('FV without delta', () => {
    expect(
      penaltySummaryLine(
        {
          kind: 'fv',
          code: 'angriffslinie.nicht_bekaempfte_ziele',
          reason_code: null,
          delta_ms: 0,
          station: 'angriffslinie',
        },
        idx,
        label,
      ),
    ).toBe('• Angriffslinie: FV – Nicht bekämpfte Ziele');
  });

  it('legacy row without station', () => {
    expect(
      penaltySummaryLine(
        { code: null, reason_code: 'kleiner_fehler', delta_ms: 5000, station: null },
        idx,
        label,
      ),
    ).toBe('• ohne Station: SZ +5.00 – kleiner_fehler');
  });
});
