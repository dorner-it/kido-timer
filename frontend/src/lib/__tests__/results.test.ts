// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  FV_LABEL,
  INVALID_LABEL,
  LINEUP_FALLBACK_LABEL,
  NOT_STARTED_LABEL,
  TAGESSIEGER_LABEL,
  bestTimeLabel,
  buildRunCell,
  buildTagessiegerRows,
  buildWgTableRows,
  isProvisional,
  normalizeResults,
  placeLabel,
  resolveTab,
  resultsTabs,
  runColumnCount,
  sortRankRows,
  sortTagessieger,
  sortWgRankings,
  szLabel,
  teamCount,
  type DisciplineResults,
  type ResultRow,
  type ResultRun,
  type TagessiegerResult,
  type WgRanking,
} from '../results';

// ===== fixtures =====

function mkRun(over: Partial<ResultRun> = {}): ResultRun {
  return {
    run_id: `run-${over.run_number ?? 1}`,
    run_number: 1,
    lane: 1,
    time_source: 'lichtschranke',
    hand_time_1_ms: null,
    hand_time_2_ms: null,
    measured_ms: 45_000,
    sz_count: 0,
    fv_count: 0,
    total_ms: 45_000,
    valid: true,
    invalid_reason: null,
    provisional: false,
    is_best: false,
    ...over,
  };
}

function mkRow(over: Partial<ResultRow> = {}): ResultRow {
  return {
    team_id: `team-${over.start_number ?? 1}`,
    team_name: 'FF Musterdorf',
    start_number: 1,
    place: 1,
    status: 'ranked',
    best_run_id: 'run-1',
    best_total_ms: 45_000,
    provisional: false,
    runs: [mkRun({ is_best: true })],
    ...over,
  };
}

function mkTs(over: Partial<TagessiegerResult> = {}): TagessiegerResult {
  return {
    place: 1,
    team_id: `team-${over.start_number ?? 1}`,
    team_name: 'FF Musterdorf',
    start_number: 1,
    wg: 'wg2',
    wg_label: 'WG2',
    run_id: 'run-1',
    run_number: 1,
    total_ms: 45_000,
    ages: [12, 13],
    avg_age: 12.5,
    factor: 0.89,
    gw_ms: 40_050,
    lineup_fallback: false,
    provisional: false,
    ...over,
  };
}

function mkRanking(wg: WgRanking['wg'], rows: ResultRow[]): WgRanking {
  return { wg, wg_label: wg.toUpperCase(), wg_label_de: 'x', rows };
}

function mkResults(over: Partial<DisciplineResults> = {}): DisciplineResults {
  return {
    event: { id: 'e1', name: 'Kreisausscheid', date: '2027-06-12', location: 'Musterstadt' },
    discipline: { id: 'd1', name: 'Gruppenstafette', mode: 'gruppenstaffette' },
    generated_at: '2027-06-12T10:00:00Z',
    provisional: false,
    wg_rankings: [mkRanking('wg2', [mkRow()])],
    tagessieger: [mkTs()],
    missing_wg: [],
    ...over,
  };
}

/** Raw backend response as documented in docs/api.md § Results. */
const apiResponse = {
  event: { id: 'e1', name: 'Kreisausscheid', date: '2027-06-12', location: null },
  discipline: { id: 'd1', name: 'Gruppenstafette', mode: 'gruppenstaffette' },
  generated_at: '2027-06-12T14:03:11.512Z',
  provisional: true,
  wg_rankings: [
    {
      wg: 'wg2',
      wg_label: 'WG2',
      wg_label_de: 'Jungen bis 14',
      rows: [
        {
          team_id: 't1',
          team_name: 'FF A',
          start_number: 1,
          place: 1,
          status: 'ranked',
          best_run_id: 'r2',
          best_total_ms: 45_000,
          provisional: false,
          runs: [
            {
              run_id: 'r1',
              run_number: 1,
              lane: 1,
              time_source: 'lichtschranke',
              hand_time_1_ms: null,
              hand_time_2_ms: null,
              measured_ms: 50_000,
              sz_count: 1,
              fv_count: 0,
              total_ms: 65_000,
              valid: true,
              invalid_reason: null,
              provisional: false,
              is_best: false,
            },
            {
              run_id: 'r2',
              run_number: 2,
              lane: 2,
              time_source: 'handstopp',
              hand_time_1_ms: 44_990,
              hand_time_2_ms: 45_010,
              measured_ms: 45_000,
              sz_count: 0,
              fv_count: 0,
              total_ms: 45_000,
              valid: true,
              invalid_reason: null,
              provisional: false,
              is_best: true,
            },
          ],
        },
        {
          team_id: 't2',
          team_name: 'FF B',
          start_number: 2,
          place: 2,
          status: 'all_invalid',
          best_run_id: null,
          best_total_ms: null,
          provisional: true,
          runs: [
            {
              run_id: 'r3',
              run_number: 1,
              lane: 1,
              time_source: 'lichtschranke',
              hand_time_1_ms: null,
              hand_time_2_ms: null,
              measured_ms: 40_000,
              sz_count: 0,
              fv_count: 1,
              total_ms: null,
              valid: false,
              invalid_reason: 'fehlversuch',
              provisional: true,
              is_best: false,
            },
          ],
        },
        {
          team_id: 't3',
          team_name: 'FF C',
          start_number: 3,
          place: null,
          status: 'not_started',
          best_run_id: null,
          best_total_ms: null,
          provisional: false,
          runs: [],
        },
      ],
    },
  ],
  tagessieger: [
    {
      place: 1,
      team_id: 't1',
      team_name: 'FF A',
      start_number: 1,
      wg: 'wg2',
      wg_label: 'WG2',
      run_id: 'r2',
      run_number: 2,
      total_ms: 45_000,
      ages: [12, 13],
      avg_age: 12.5,
      factor: 0.89,
      gw_ms: 40_050,
      lineup_fallback: false,
      provisional: false,
    },
  ],
  missing_wg: [{ team_id: 't9', team_name: 'FF Z', start_number: 9 }],
};

// ===== normalizeResults =====

describe('normalizeResults', () => {
  it('parses the documented response', () => {
    const r = normalizeResults(apiResponse);
    expect(r.event.name).toBe('Kreisausscheid');
    expect(r.event.location).toBeNull();
    expect(r.discipline.mode).toBe('gruppenstaffette');
    expect(r.provisional).toBe(true);
    expect(r.wg_rankings).toHaveLength(1);
    expect(r.wg_rankings[0]!.rows).toHaveLength(3);
    expect(r.wg_rankings[0]!.rows[0]!.runs[1]!.time_source).toBe('handstopp');
    expect(r.wg_rankings[0]!.rows[1]!.runs[0]!.invalid_reason).toBe('fehlversuch');
    expect(r.tagessieger[0]!.gw_ms).toBe(40_050);
    expect(r.missing_wg).toEqual([{ team_id: 't9', team_name: 'FF Z', start_number: 9 }]);
  });

  it('fills WG labels when missing', () => {
    const raw = structuredClone(apiResponse) as Record<string, unknown>;
    const wgs = raw.wg_rankings as Array<Record<string, unknown>>;
    delete wgs[0]!.wg_label;
    delete wgs[0]!.wg_label_de;
    const ts = raw.tagessieger as Array<Record<string, unknown>>;
    delete ts[0]!.wg_label;
    const r = normalizeResults(raw);
    expect(r.wg_rankings[0]!.wg_label).toBe('WG2');
    expect(r.wg_rankings[0]!.wg_label_de).toBe('Jungen bis 14');
    expect(r.tagessieger[0]!.wg_label).toBe('WG2');
  });

  it('defaults missing lists and optional run fields', () => {
    const raw = {
      event: apiResponse.event,
      discipline: apiResponse.discipline,
      generated_at: apiResponse.generated_at,
      wg_rankings: [
        {
          wg: 'wg1',
          rows: [
            {
              team_id: 't1',
              team_name: 'A',
              start_number: 1,
              place: 1,
              status: 'ranked',
              best_run_id: 'r1',
              best_total_ms: 1000,
              provisional: false,
              runs: [{ run_id: 'r1', run_number: 1, total_ms: 1000, valid: true }],
            },
          ],
        },
      ],
    };
    const r = normalizeResults(raw);
    expect(r.provisional).toBe(false);
    expect(r.tagessieger).toEqual([]);
    expect(r.missing_wg).toEqual([]);
    const run = r.wg_rankings[0]!.rows[0]!.runs[0]!;
    expect(run.time_source).toBe('lichtschranke');
    expect(run.hand_time_1_ms).toBeNull();
    expect(run.measured_ms).toBeNull();
    expect(run.sz_count).toBe(0);
    expect(run.is_best).toBe(false);
  });

  it('rejects a response without header', () => {
    expect(() => normalizeResults({})).toThrow();
    expect(() => normalizeResults(null)).toThrow();
  });
});

// ===== labels =====

describe('labels', () => {
  it('szLabel', () => {
    expect(szLabel(0)).toBeNull();
    expect(szLabel(1)).toBe('+1 SZ');
    expect(szLabel(3)).toBe('+3 SZ');
  });

  it('placeLabel', () => {
    expect(placeLabel(1)).toBe('1.');
    expect(placeLabel(12)).toBe('12.');
    expect(placeLabel(null)).toBe(NOT_STARTED_LABEL);
    expect(NOT_STARTED_LABEL).toBe('n.a.');
  });
});

// ===== sorting / grouping =====

describe('sortRankRows', () => {
  it('ranked by place, then all_invalid, then not_started', () => {
    const rows = [
      mkRow({ team_id: 'ns', status: 'not_started', place: null, start_number: 1 }),
      mkRow({ team_id: 'inv', status: 'all_invalid', place: 3, start_number: 2 }),
      mkRow({ team_id: 'p2', place: 2, start_number: 3 }),
      mkRow({ team_id: 'p1', place: 1, start_number: 4 }),
    ];
    expect(sortRankRows(rows).map((r) => r.team_id)).toEqual(['p1', 'p2', 'inv', 'ns']);
  });

  it('equal places ordered by start number (1, 1, 3)', () => {
    const rows = [
      mkRow({ team_id: 'c', place: 3, start_number: 1 }),
      mkRow({ team_id: 'b', place: 1, start_number: 7 }),
      mkRow({ team_id: 'a', place: 1, start_number: 2 }),
    ];
    expect(sortRankRows(rows).map((r) => r.team_id)).toEqual(['a', 'b', 'c']);
  });

  it('groups ordered by start number', () => {
    const rows = [
      mkRow({ team_id: 'ns9', status: 'not_started', place: null, start_number: 9 }),
      mkRow({ team_id: 'ns4', status: 'not_started', place: null, start_number: 4 }),
      mkRow({ team_id: 'inv8', status: 'all_invalid', place: 1, start_number: 8 }),
      mkRow({ team_id: 'inv5', status: 'all_invalid', place: 1, start_number: 5 }),
    ];
    expect(sortRankRows(rows).map((r) => r.team_id)).toEqual(['inv5', 'inv8', 'ns4', 'ns9']);
  });

  it('does not mutate the input', () => {
    const rows = [mkRow({ team_id: 'b', place: 2 }), mkRow({ team_id: 'a', place: 1 })];
    sortRankRows(rows);
    expect(rows.map((r) => r.team_id)).toEqual(['b', 'a']);
  });
});

describe('sortWgRankings', () => {
  it('orders WG1 … WG5', () => {
    const sorted = sortWgRankings([
      mkRanking('wg5', []),
      mkRanking('wg1', []),
      mkRanking('wg3', []),
    ]);
    expect(sorted.map((w) => w.wg)).toEqual(['wg1', 'wg3', 'wg5']);
  });
});

describe('sortTagessieger', () => {
  it('by place, then start number', () => {
    const sorted = sortTagessieger([
      mkTs({ team_id: 'c', place: 3, start_number: 1 }),
      mkTs({ team_id: 'b', place: 1, start_number: 5 }),
      mkTs({ team_id: 'a', place: 1, start_number: 2 }),
    ]);
    expect(sorted.map((t) => t.team_id)).toEqual(['a', 'b', 'c']);
  });
});

describe('runColumnCount', () => {
  it('at least two columns', () => {
    expect(runColumnCount([])).toBe(2);
    expect(runColumnCount([mkRow({ runs: [mkRun({ run_number: 1 })] })])).toBe(2);
  });

  it('grows with the highest run number', () => {
    expect(
      runColumnCount([
        mkRow({ runs: [mkRun({ run_number: 1 })] }),
        mkRow({ runs: [mkRun({ run_number: 3 })] }),
      ]),
    ).toBe(3);
  });
});

describe('teamCount', () => {
  it('sums rows over all WGs', () => {
    const r = mkResults({
      wg_rankings: [mkRanking('wg1', [mkRow(), mkRow()]), mkRanking('wg2', [mkRow()])],
    });
    expect(teamCount(r)).toBe(3);
  });
});

// ===== provisional =====

describe('isProvisional', () => {
  it('false for null and fully confirmed results', () => {
    expect(isProvisional(null)).toBe(false);
    expect(isProvisional(undefined)).toBe(false);
    expect(isProvisional(mkResults())).toBe(false);
  });

  it('top-level flag', () => {
    expect(isProvisional(mkResults({ provisional: true }))).toBe(true);
  });

  it('a provisional row', () => {
    const r = mkResults({ wg_rankings: [mkRanking('wg1', [mkRow({ provisional: true })])] });
    expect(isProvisional(r)).toBe(true);
  });

  it('a provisional run inside a non-provisional row', () => {
    const r = mkResults({
      wg_rankings: [mkRanking('wg1', [mkRow({ runs: [mkRun({ provisional: true })] })])],
    });
    expect(isProvisional(r)).toBe(true);
  });

  it('a provisional Tagessieger row', () => {
    expect(isProvisional(mkResults({ tagessieger: [mkTs({ provisional: true })] }))).toBe(true);
  });

  it('a provisional all_invalid team (pending FV)', () => {
    const r = mkResults({
      wg_rankings: [
        mkRanking('wg1', [
          mkRow({
            status: 'all_invalid',
            provisional: true,
            runs: [mkRun({ valid: false, invalid_reason: 'fehlversuch', provisional: true })],
          }),
        ]),
      ],
    });
    expect(isProvisional(r)).toBe(true);
  });
});

// ===== run cells =====

describe('buildRunCell', () => {
  it('missing run → dash', () => {
    const c = buildRunCell(undefined);
    expect(c.text).toBe('—');
    expect(c.sz).toBeNull();
    expect(c.hand).toBe(false);
    expect(c.invalid).toBe(false);
    expect(c.provisional).toBe(false);
    expect(c.title).toBe('');
  });

  it('valid run shows the total', () => {
    const c = buildRunCell(mkRun({ total_ms: 47_230, measured_ms: 47_230 }));
    expect(c.text).toBe('47,23');
    expect(c.sz).toBeNull();
    expect(c.invalid).toBe(false);
    expect(c.title).toContain('Gemessen 47,23');
  });

  it('SZ: total incl. penalty plus "+n SZ"', () => {
    const c = buildRunCell(mkRun({ measured_ms: 50_000, sz_count: 2, total_ms: 80_000 }));
    expect(c.text).toBe('1:20,00');
    expect(c.sz).toBe('+2 SZ');
    expect(c.title).toContain('Gemessen 50,00');
    expect(c.title).toContain('2 × 15 s Strafzeit');
  });

  it('FV → "FV", invalid', () => {
    const c = buildRunCell(
      mkRun({ valid: false, invalid_reason: 'fehlversuch', fv_count: 1, total_ms: null }),
    );
    expect(c.text).toBe(FV_LABEL);
    expect(c.invalid).toBe(true);
    expect(c.title).toContain('Fehlversuch');
  });

  it('FV with SZ keeps the SZ marker', () => {
    const c = buildRunCell(
      mkRun({ valid: false, invalid_reason: 'fehlversuch', fv_count: 1, sz_count: 1, total_ms: null }),
    );
    expect(c.text).toBe('FV');
    expect(c.sz).toBe('+1 SZ');
  });

  it('no time → "ungültig"', () => {
    const c = buildRunCell(
      mkRun({ valid: false, invalid_reason: 'no_time', measured_ms: null, total_ms: null }),
    );
    expect(c.text).toBe(INVALID_LABEL);
    expect(c.invalid).toBe(true);
    expect(c.title).toContain('Keine Zeit erfasst');
  });

  it('invalid without a reason falls back to "ungültig"', () => {
    const c = buildRunCell(mkRun({ valid: false, invalid_reason: null, total_ms: null }));
    expect(c.text).toBe(INVALID_LABEL);
  });

  it('hand time marker and tooltip', () => {
    const c = buildRunCell(
      mkRun({ time_source: 'handstopp', hand_time_1_ms: 44_990, hand_time_2_ms: 45_010 }),
    );
    expect(c.hand).toBe(true);
    expect(c.title).toContain('Handzeit (Mittel aus 44,99 / 45,01)');
  });

  it('light barrier has no hand marker', () => {
    expect(buildRunCell(mkRun()).hand).toBe(false);
  });

  it('provisional marker', () => {
    const c = buildRunCell(mkRun({ provisional: true }));
    expect(c.provisional).toBe(true);
    expect(c.title).toContain('Vorläufig');
  });

  it('best flag', () => {
    expect(buildRunCell(mkRun({ is_best: true })).best).toBe(true);
    expect(buildRunCell(mkRun({ is_best: false })).best).toBe(false);
  });
});

describe('bestTimeLabel', () => {
  it('ranked → best total', () => {
    expect(bestTimeLabel(mkRow({ best_total_ms: 62_230 }))).toBe('1:02,23');
  });
  it('all_invalid → ungültig', () => {
    expect(bestTimeLabel(mkRow({ status: 'all_invalid', best_total_ms: null }))).toBe(INVALID_LABEL);
  });
  it('not_started → n.a.', () => {
    expect(bestTimeLabel(mkRow({ status: 'not_started', best_total_ms: null }))).toBe('n.a.');
  });
});

// ===== WG table rows =====

describe('buildWgTableRows', () => {
  const rows = normalizeResults(apiResponse).wg_rankings[0]!.rows;

  it('one display row per team, sorted', () => {
    const out = buildWgTableRows(rows);
    expect(out.map((r) => r.teamName)).toEqual(['FF A', 'FF B', 'FF C']);
    expect(out.map((r) => r.key)).toEqual(['t1', 't2', 't3']);
  });

  it('place display: ranked, shared last place, n.a.', () => {
    const out = buildWgTableRows(rows);
    expect(out.map((r) => r.place)).toEqual(['1.', '2.', 'n.a.']);
  });

  it('not_started shows n.a. even if a place were sent', () => {
    const out = buildWgTableRows([mkRow({ status: 'not_started', place: 5, runs: [] })]);
    expect(out[0]!.place).toBe('n.a.');
  });

  it('two invalid-only teams share the same last place', () => {
    const out = buildWgTableRows([
      mkRow({ team_id: 'a', place: 1, start_number: 1 }),
      mkRow({ team_id: 'b', status: 'all_invalid', place: 2, start_number: 3, runs: [] }),
      mkRow({ team_id: 'c', status: 'all_invalid', place: 2, start_number: 2, runs: [] }),
    ]);
    expect(out.map((r) => [r.key, r.place])).toEqual([
      ['a', '1.'],
      ['c', '2.'],
      ['b', '2.'],
    ]);
  });

  it('run cells by run number with a dash for missing runs', () => {
    const out = buildWgTableRows(rows);
    const a = out[0]!;
    expect(a.runs).toHaveLength(2);
    expect(a.runs[0]!.text).toBe('1:05,00');
    expect(a.runs[0]!.sz).toBe('+1 SZ');
    expect(a.runs[0]!.best).toBe(false);
    expect(a.runs[1]!.text).toBe('45,00');
    expect(a.runs[1]!.hand).toBe(true);
    expect(a.runs[1]!.best).toBe(true);
    const b = out[1]!;
    expect(b.runs[0]!.text).toBe('FV');
    expect(b.runs[1]!.text).toBe('—');
    const c = out[2]!;
    expect(c.runs.map((x) => x.text)).toEqual(['—', '—']);
  });

  it('run 2 only (run 1 not completed) keeps column positions', () => {
    const out = buildWgTableRows([
      mkRow({ runs: [mkRun({ run_number: 2, total_ms: 50_000, is_best: true })] }),
    ]);
    expect(out[0]!.runs.map((x) => x.text)).toEqual(['—', '50,00']);
  });

  it('Bestzeit column', () => {
    const out = buildWgTableRows(rows);
    expect(out.map((r) => r.best)).toEqual(['45,00', 'ungültig', 'n.a.']);
  });

  it('provisional row marker from row or any run', () => {
    const out = buildWgTableRows([
      mkRow({ team_id: 'a', provisional: true }),
      mkRow({ team_id: 'b', place: 2, runs: [mkRun({ provisional: true })] }),
      mkRow({ team_id: 'c', place: 3 }),
    ]);
    expect(out.map((r) => r.provisional)).toEqual([true, true, false]);
  });

  it('marks the first row of each non-ranked group', () => {
    const out = buildWgTableRows([
      mkRow({ team_id: 'a', place: 1 }),
      mkRow({ team_id: 'b', place: 2 }),
      mkRow({ team_id: 'c', status: 'all_invalid', place: 3, start_number: 3 }),
      mkRow({ team_id: 'd', status: 'all_invalid', place: 3, start_number: 4 }),
      mkRow({ team_id: 'e', status: 'not_started', place: null, start_number: 5 }),
      mkRow({ team_id: 'f', status: 'not_started', place: null, start_number: 6 }),
    ]);
    expect(out.map((r) => r.groupStart)).toEqual([false, false, true, false, true, false]);
  });

  it('a WG with only not-started teams', () => {
    const out = buildWgTableRows([
      mkRow({ team_id: 'a', status: 'not_started', place: null, runs: [], start_number: 2 }),
      mkRow({ team_id: 'b', status: 'not_started', place: null, runs: [], start_number: 1 }),
    ]);
    expect(out.map((r) => [r.key, r.place, r.best])).toEqual([
      ['b', 'n.a.', 'n.a.'],
      ['a', 'n.a.', 'n.a.'],
    ]);
    expect(out[0]!.groupStart).toBe(true);
  });

  it('explicit column count pads extra columns', () => {
    const out = buildWgTableRows([mkRow()], 3);
    expect(out[0]!.runs).toHaveLength(3);
    expect(out[0]!.runs[2]!.text).toBe('—');
  });

  it('start number 0 (unknown) shows a dash', () => {
    expect(buildWgTableRows([mkRow({ start_number: 0 })])[0]!.startNumber).toBe('—');
    expect(buildWgTableRows([mkRow({ start_number: 7 })])[0]!.startNumber).toBe('7');
  });

  it('empty WG', () => {
    expect(buildWgTableRows([])).toEqual([]);
  });
});

// ===== Tagessieger rows =====

describe('buildTagessiegerRows', () => {
  it('formats place, ages, factor and times', () => {
    const [row] = buildTagessiegerRows([mkTs()]);
    expect(row).toMatchObject({
      key: 'team-1',
      place: '1.',
      startNumber: '1',
      teamName: 'FF Musterdorf',
      wg: 'WG2',
      avgAge: '12,5',
      factor: '0,8900',
      best: '45,00',
      gw: '40,05',
      fallback: null,
      provisional: false,
    });
    expect(row!.title).toBe('Alter: 12, 13 · Lauf 1');
  });

  it('"Kader statt Besetzung" when lineup_fallback', () => {
    const [row] = buildTagessiegerRows([mkTs({ lineup_fallback: true })]);
    expect(row!.fallback).toBe(LINEUP_FALLBACK_LABEL);
    expect(row!.fallback).toBe('Kader statt Besetzung');
    expect(row!.title).toContain('ganzen Kader');
  });

  it('sorted by place with ties 1, 1, 3', () => {
    const rows = buildTagessiegerRows([
      mkTs({ team_id: 'c', place: 3, start_number: 3, gw_ms: 41_000 }),
      mkTs({ team_id: 'b', place: 1, start_number: 2, gw_ms: 40_000 }),
      mkTs({ team_id: 'a', place: 1, start_number: 1, gw_ms: 40_000 }),
    ]);
    expect(rows.map((r) => [r.key, r.place])).toEqual([
      ['a', '1.'],
      ['b', '1.'],
      ['c', '3.'],
    ]);
  });

  it('times over a minute and factor > 1', () => {
    const [row] = buildTagessiegerRows([
      mkTs({ total_ms: 75_000, gw_ms: 61_500, factor: 0.82, avg_age: 9 }),
    ]);
    expect(row!.best).toBe('1:15,00');
    expect(row!.gw).toBe('1:01,50');
    expect(row!.factor).toBe('0,8200');
    expect(row!.avgAge).toBe('9,0');
  });

  it('provisional and unknown run number', () => {
    const [row] = buildTagessiegerRows([mkTs({ provisional: true, run_number: null, ages: [] })]);
    expect(row!.provisional).toBe(true);
    expect(row!.title).toBe('Alter: —');
  });

  it('empty list', () => {
    expect(buildTagessiegerRows([])).toEqual([]);
  });
});

// ===== tabs =====

describe('resultsTabs / resolveTab', () => {
  const r = mkResults({
    wg_rankings: [
      { wg: 'wg4', wg_label: 'WG4', wg_label_de: '', rows: [mkRow()] },
      { wg: 'wg1', wg_label: 'WG1', wg_label_de: '', rows: [mkRow(), mkRow()] },
    ],
    tagessieger: [mkTs(), mkTs(), mkTs()],
  });

  it('one tab per WG in order, then Tagessieger', () => {
    expect(resultsTabs(r)).toEqual([
      { key: 'wg1', label: 'WG1', count: 2 },
      { key: 'wg4', label: 'WG4', count: 1 },
      { key: 'tagessieger', label: TAGESSIEGER_LABEL, count: 3 },
    ]);
  });

  it('keeps an existing tab', () => {
    expect(resolveTab(r, 'wg4')).toBe('wg4');
    expect(resolveTab(r, 'tagessieger')).toBe('tagessieger');
  });

  it('falls back to the first WG', () => {
    expect(resolveTab(r, null)).toBe('wg1');
    expect(resolveTab(r, 'wg3')).toBe('wg1');
  });

  it('only Tagessieger without WGs', () => {
    const empty = mkResults({ wg_rankings: [], tagessieger: [] });
    expect(resultsTabs(empty)).toEqual([{ key: 'tagessieger', label: 'Tagessieger', count: 0 }]);
    expect(resolveTab(empty, null)).toBe('tagessieger');
  });
});
