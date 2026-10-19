// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it, vi } from 'vitest';

import {
  catalogueEntrySchema,
  disciplineFormSchema,
  disciplineReadSchema,
  eventReadSchema,
  fieldErrors,
  handTimesSchema,
  lineupSchema,
  moveWgFormSchema,
  normalizeCatalogue,
  normalizeDiscipline,
  normalizeEvent,
  normalizeEventDetail,
  normalizePenalty,
  normalizeRun,
  normalizeRunner,
  normalizeStationState,
  normalizeTeamEntry,
  penaltyReadSchema,
  runReadSchema,
  runnerFormSchema,
  teamEntryFormSchema,
  teamEntryReadSchema,
} from '../schemas';

const ts = '2026-10-01T10:00:00Z';

const legacyDiscipline = {
  id: 'd1',
  event_id: 'e1',
  name: 'GS Jugend',
  mode: 'gruppenstaffette',
  sync_mode: 'live',
  position: 1,
  created_at: ts,
  updated_at: ts,
};

const legacyEvent = {
  id: 'e1',
  owner_sub: 'sub',
  name: 'Kreisausscheid',
  date: '2027-05-10',
  location: null,
  is_active: true,
  current_discipline_id: 'd1',
  current_run_id: 'run1',
  decider_tokens: ['a', 'b', 'c', 'd'],
  station_token: 'tok',
  created_at: ts,
  updated_at: ts,
};

const legacyEntry = {
  team_id: 't1',
  discipline_id: 'd1',
  event_id: 'e1',
  start_number: 3,
  lane: 2,
  wg: null,
  created_at: ts,
  updated_at: ts,
};

const legacyRun = {
  id: 'run1',
  event_id: 'e1',
  discipline_id: 'd1',
  team_id: 't1',
  runner_id: null,
  run_number: 1,
  lane: 1,
  status: 'active',
  original_time_ms: 47230,
  correction_ms: 0,
  started_at: null,
  ended_at: null,
  created_at: ts,
  updated_at: ts,
};

const legacyPenalty = {
  id: 'p1',
  event_id: 'e1',
  discipline_id: 'd1',
  run_id: 'run1',
  team_id: 't1',
  runner_id: null,
  station: 'knoten',
  is_disqualification: false,
  decider_slot: 0,
  delta_ms: 10000,
  reason_code: 'knoten_bund',
  status: 'pending',
  created_at: ts,
  reviewed_at: null,
  reviewed_by: null,
};

const legacyRunner = {
  id: 'r1',
  team_id: 't1',
  event_id: 'e1',
  first_name: 'Max',
  last_name: 'Muster',
  birth_date: '2012-03-04',
  position: 1,
  gender: 'male',
  created_at: ts,
  updated_at: ts,
};

describe('discipline read schema', () => {
  it('accepts a legacy row and defaults lane_count + judge_mode', () => {
    const d = normalizeDiscipline(legacyDiscipline);
    expect(d.lane_count).toBe(2);
    expect(d.judge_mode).toBe('per_station');
    expect(d.name).toBe('GS Jugend');
  });

  it('defaults Löschangriff to 4 lanes', () => {
    expect(normalizeDiscipline({ ...legacyDiscipline, mode: 'loeschangriff' }).lane_count).toBe(4);
  });

  it('keeps valid new fields', () => {
    const d = normalizeDiscipline({
      ...legacyDiscipline,
      lane_count: 1,
      judge_mode: 'per_station_and_lane',
    });
    expect(d.lane_count).toBe(1);
    expect(d.judge_mode).toBe('per_station_and_lane');
  });

  it('replaces an invalid judge mode / lane count with defaults', () => {
    const d = normalizeDiscipline({ ...legacyDiscipline, lane_count: 3, judge_mode: 'x' });
    expect(d.lane_count).toBe(2);
    expect(d.judge_mode).toBe('per_station');
  });

  it('passes lane_conflicts through', () => {
    const parsed = disciplineReadSchema.parse({ ...legacyDiscipline, lane_conflicts: ['t1'] });
    expect(parsed.lane_conflicts).toEqual(['t1']);
  });

  it('rejects an unknown mode in the schema but normalize falls back to the raw row', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const raw = { ...legacyDiscipline, mode: 'single_lane' };
    expect(disciplineReadSchema.safeParse(raw).success).toBe(false);
    expect(normalizeDiscipline(raw)).toBe(raw);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});

describe('event read schema', () => {
  it('derives current_run_ids from current_run_id', () => {
    expect(normalizeEvent(legacyEvent).current_run_ids).toEqual(['run1']);
  });

  it('empty when no current run', () => {
    expect(normalizeEvent({ ...legacyEvent, current_run_id: null }).current_run_ids).toEqual([]);
  });

  it('keeps an explicit list', () => {
    expect(
      eventReadSchema.parse({ ...legacyEvent, current_run_ids: ['run1', 'run2'] }).current_run_ids,
    ).toEqual(['run1', 'run2']);
  });
});

describe('runner read schema', () => {
  it('tolerates legacy gender and missing gender', () => {
    expect(normalizeRunner(legacyRunner).gender).toBe('male');
    const { gender: _g, ...noGender } = legacyRunner;
    expect(normalizeRunner(noGender).gender).toBeNull();
    expect(normalizeRunner({ ...legacyRunner, gender: 'divers' }).gender).toBeNull();
  });
});

describe('team entry read schema', () => {
  it('accepts a legacy row (wg null, no history/warnings)', () => {
    const e = normalizeTeamEntry(legacyEntry);
    expect(e.wg).toBeNull();
    expect(e.wg_history).toEqual([]);
    expect(e.wg_warnings).toEqual([]);
    expect(e.start_number).toBe(3);
  });

  it('parses history and warnings', () => {
    const e = teamEntryReadSchema.parse({
      ...legacyEntry,
      wg: 'wg1',
      wg_history: [{ from: 'wg2', to: 'wg1', by: 'orga', at: ts, note: '2 Mädchen' }],
      wg_warnings: [{ runner_id: 'r1', age: 15, wg: 'wg1', reason: 'zu alt' }],
    });
    expect(e.wg).toBe('wg1');
    expect(e.wg_history[0]?.note).toBe('2 Mädchen');
    expect(e.wg_warnings[0]?.age).toBe(15);
  });

  it('maps an invalid wg to null', () => {
    expect(normalizeTeamEntry({ ...legacyEntry, wg: 'wg7' }).wg).toBeNull();
  });
});

describe('run read schema', () => {
  it('defaults the new fields on a legacy row', () => {
    const r = normalizeRun(legacyRun);
    expect(r.lineup).toEqual([]);
    expect(r.lineup_confirmed_at).toBeNull();
    expect(r.lineup_confirmed_by).toBeNull();
    expect(r.time_source).toBe('lichtschranke');
    expect(r.hand_time_1_ms).toBeNull();
    expect(r.hand_time_2_ms).toBeNull();
    expect(r.penalties_accepted_at).toBeNull();
    expect(r.original_time_ms).toBe(47230);
  });

  it('keeps penalties_accepted_at', () => {
    const r = normalizeRun({ ...legacyRun, penalties_accepted_at: ts });
    expect(r.penalties_accepted_at).toBe(ts);
  });

  it('keeps hand times and lineup', () => {
    const r = runReadSchema.parse({
      ...legacyRun,
      lineup: ['r1', 'r2'],
      lineup_confirmed_at: ts,
      lineup_confirmed_by: 'startlinie',
      time_source: 'handstopp',
      hand_time_1_ms: 47230,
      hand_time_2_ms: 47245,
    });
    expect(r.time_source).toBe('handstopp');
    expect(r.lineup_confirmed_by).toBe('startlinie');
    expect(r.hand_time_2_ms).toBe(47245);
  });
});

describe('penalty read schema', () => {
  it('migrates legacy station key and kind', () => {
    const p = normalizePenalty(legacyPenalty);
    expect(p.station).toBe('knotengestell');
    expect(p.kind).toBe('sz');
    expect(p.code).toBeNull();
    expect(p.delta_ms).toBe(10000);
  });

  it('DQ rows become FV', () => {
    expect(normalizePenalty({ ...legacyPenalty, is_disqualification: true }).kind).toBe('fv');
  });

  it('maps every legacy station key', () => {
    const map = {
      start: 'startlinie',
      verteiler: 'verteilerlinie',
      strahlrohr: 'strahlrohrlinie',
      knoten: 'knotengestell',
      kuebelspritze: 'angriffslinie',
    };
    for (const [legacy, canonical] of Object.entries(map)) {
      expect(normalizePenalty({ ...legacyPenalty, station: legacy }).station).toBe(canonical);
    }
  });

  it('null / unknown station → null', () => {
    expect(normalizePenalty({ ...legacyPenalty, station: null }).station).toBeNull();
    expect(normalizePenalty({ ...legacyPenalty, station: 'mond' }).station).toBeNull();
  });

  it('keeps kind and code from the 2027 backend', () => {
    const p = penaltyReadSchema.parse({
      ...legacyPenalty,
      station: 'knotengestell',
      kind: 'fv',
      code: 'knotengestell.mehrere_knoten_falsch',
      delta_ms: 0,
      is_disqualification: true,
    });
    expect(p.kind).toBe('fv');
    expect(p.code).toBe('knotengestell.mehrere_knoten_falsch');
  });
});

describe('catalogue', () => {
  it('normalizes entries and legacy station keys', () => {
    expect(
      normalizeCatalogue([
        { code: 'a', station: 'knoten', kind: 'sz', label_de: 'A' },
        { code: 'b', station: 'angriffslinie', kind: 'fv', label_de: 'B' },
      ]),
    ).toEqual([
      { code: 'a', station: 'knotengestell', kind: 'sz', label_de: 'A' },
      { code: 'b', station: 'angriffslinie', kind: 'fv', label_de: 'B' },
    ]);
  });

  it('drops invalid rows', () => {
    expect(
      normalizeCatalogue([
        { code: 'a', station: 'mond', kind: 'sz', label_de: 'A' },
        { code: '', station: 'podest', kind: 'sz', label_de: 'B' },
        { code: 'c', station: 'podest', kind: 'dq', label_de: 'C' },
        null,
      ]),
    ).toEqual([]);
    expect(normalizeCatalogue({})).toEqual([]);
  });

  it('schema requires label_de', () => {
    expect(
      catalogueEntrySchema.safeParse({ code: 'a', station: 'podest', kind: 'sz' }).success,
    ).toBe(false);
  });
});

describe('normalizeEventDetail', () => {
  it('normalizes all nested legacy rows', () => {
    const d = normalizeEventDetail({
      event: legacyEvent,
      disciplines: [legacyDiscipline],
      teams: [{ id: 't1' }],
      runners: [legacyRunner],
      team_entries: [legacyEntry],
      runs: [legacyRun],
    });
    expect(d.event.current_run_ids).toEqual(['run1']);
    expect(d.disciplines[0]?.lane_count).toBe(2);
    expect(d.team_entries[0]?.wg_history).toEqual([]);
    expect(d.runs[0]?.time_source).toBe('lichtschranke');
    expect(d.teams).toHaveLength(1);
  });

  it('tolerates missing arrays', () => {
    const d = normalizeEventDetail({ event: legacyEvent });
    expect(d.disciplines).toEqual([]);
    expect(d.runs).toEqual([]);
  });
});

describe('normalizeStationState', () => {
  const team1 = { id: 't1', name: 'FF A' };
  const team2 = { id: 't2', name: 'FF B' };

  it('falls back to [current_run] for an older backend', () => {
    const s = normalizeStationState({
      event: { id: 'e1' },
      discipline: { id: 'd1', name: 'GS', mode: 'gruppenstaffette' },
      current_run: { run: legacyRun, team: team1, runner: null },
      disabled: false,
      allowed_stations: ['start', 'knoten'],
    });
    expect(s.current_runs).toHaveLength(1);
    expect(s.current_run?.team.name).toBe('FF A');
    expect(s.current_runs[0]?.runners).toEqual([]);
    expect(s.allowed_stations).toEqual(['startlinie', 'knotengestell']);
    expect(s.discipline?.lane_count).toBe(2);
    expect(s.discipline?.judge_mode).toBe('per_station');
  });

  it('orders current_runs by lane and sets current_run = first', () => {
    const s = normalizeStationState({
      event: { id: 'e1' },
      discipline: { id: 'd1', name: 'GS', mode: 'gruppenstaffette', lane_count: 2 },
      current_runs: [
        { run: { ...legacyRun, id: 'run2', lane: 2 }, team: team2, runners: [legacyRunner] },
        { run: { ...legacyRun, id: 'run1', lane: 1 }, team: team1, runners: [] },
      ],
      disabled: false,
    });
    expect(s.current_runs.map((c) => c.run.id)).toEqual(['run1', 'run2']);
    expect(s.current_run?.run.id).toBe('run1');
    expect(s.current_runs[1]?.runners[0]?.first_name).toBe('Max');
    expect(s.allowed_stations).toEqual([]);
  });

  it('no runs, no discipline', () => {
    const s = normalizeStationState({ event: { id: 'e1' }, discipline: null, disabled: true });
    expect(s.current_runs).toEqual([]);
    expect(s.current_run).toBeNull();
    expect(s.discipline).toBeNull();
    expect(s.disabled).toBe(true);
  });
});

// ===== form schemas =====

describe('disciplineFormSchema', () => {
  const base = {
    name: 'GS Jugend',
    mode: 'gruppenstaffette' as const,
    position: 1,
    lane_count: 2,
    judge_mode: 'per_station' as const,
  };

  it('accepts Gruppenstaffette with 1 or 2 lanes', () => {
    expect(disciplineFormSchema.safeParse(base).success).toBe(true);
    expect(disciplineFormSchema.safeParse({ ...base, lane_count: 1 }).success).toBe(true);
  });

  it('rejects an invalid lane_count for Gruppenstaffette', () => {
    const r = disciplineFormSchema.safeParse({ ...base, lane_count: 3 });
    expect(r.success).toBe(false);
    if (!r.success) {
      expect(fieldErrors(r.error).lane_count).toBe('Gruppenstaffette hat 1 oder 2 Bahnen.');
    }
  });

  it('Löschangriff requires exactly 4', () => {
    expect(
      disciplineFormSchema.safeParse({ ...base, mode: 'loeschangriff', lane_count: 4 }).success,
    ).toBe(true);
    const r = disciplineFormSchema.safeParse({ ...base, mode: 'loeschangriff', lane_count: 2 });
    expect(r.success).toBe(false);
    if (!r.success) expect(fieldErrors(r.error).lane_count).toBe('Löschangriff hat immer 4 Bahnen.');
  });

  it('rejects a non-integer lane count, empty name and unknown judge mode', () => {
    expect(disciplineFormSchema.safeParse({ ...base, lane_count: 1.5 }).success).toBe(false);
    expect(disciplineFormSchema.safeParse({ ...base, name: '   ' }).success).toBe(false);
    expect(disciplineFormSchema.safeParse({ ...base, judge_mode: 'per_lane' }).success).toBe(false);
  });

  it('trims the name', () => {
    const r = disciplineFormSchema.parse({ ...base, name: '  GS  ' });
    expect(r.name).toBe('GS');
  });
});

describe('teamEntryFormSchema', () => {
  const base = { team_id: 't1', start_number: 1, lane: null, wg: 'wg2' };

  it('accepts a complete entry', () => {
    expect(teamEntryFormSchema.parse(base)).toEqual(base);
  });

  it('requires wg', () => {
    const r = teamEntryFormSchema.safeParse({ ...base, wg: undefined });
    expect(r.success).toBe(false);
    if (!r.success) expect(fieldErrors(r.error).wg).toBe('Bitte Wertungsgruppe wählen.');
  });

  it('rejects empty-string and unknown wg', () => {
    expect(teamEntryFormSchema.safeParse({ ...base, wg: '' }).success).toBe(false);
    expect(teamEntryFormSchema.safeParse({ ...base, wg: 'wg6' }).success).toBe(false);
  });

  it('validates team, start number and lane', () => {
    expect(teamEntryFormSchema.safeParse({ ...base, team_id: '' }).success).toBe(false);
    expect(teamEntryFormSchema.safeParse({ ...base, start_number: 0 }).success).toBe(false);
    expect(teamEntryFormSchema.safeParse({ ...base, lane: 0 }).success).toBe(false);
    expect(teamEntryFormSchema.safeParse({ ...base, lane: 2 }).success).toBe(true);
  });
});

describe('moveWgFormSchema', () => {
  it('requires a target WG', () => {
    const r = moveWgFormSchema.safeParse({ note: 'x' });
    expect(r.success).toBe(false);
    if (!r.success) expect(fieldErrors(r.error).wg).toBe('Bitte Ziel-WG wählen.');
  });

  it('trims the note and drops empty notes', () => {
    expect(moveWgFormSchema.parse({ wg: 'wg1', note: '  2 Mädchen ' })).toEqual({
      wg: 'wg1',
      note: '2 Mädchen',
    });
    expect(moveWgFormSchema.parse({ wg: 'wg1', note: '   ' }).note).toBeUndefined();
    expect(moveWgFormSchema.parse({ wg: 'wg1' }).note).toBeUndefined();
  });

  it('limits the note length', () => {
    expect(moveWgFormSchema.safeParse({ wg: 'wg1', note: 'x'.repeat(501) }).success).toBe(false);
  });
});

describe('runnerFormSchema', () => {
  it('strips gender', () => {
    const r = runnerFormSchema.parse({
      first_name: 'Max',
      last_name: 'Muster',
      birth_date: '2012-03-04',
      position: 1,
      gender: 'male',
    });
    expect(r).toEqual({
      first_name: 'Max',
      last_name: 'Muster',
      birth_date: '2012-03-04',
      position: 1,
    });
  });

  it('rejects bad dates', () => {
    expect(
      runnerFormSchema.safeParse({
        first_name: 'Max',
        last_name: 'Muster',
        birth_date: '04.03.2012',
        position: 1,
      }).success,
    ).toBe(false);
  });
});

describe('lineup + hand times', () => {
  it('lineup rejects duplicates, allows empty', () => {
    expect(lineupSchema.safeParse([]).success).toBe(true);
    expect(lineupSchema.safeParse(['a', 'b']).success).toBe(true);
    expect(lineupSchema.safeParse(['a', 'a']).success).toBe(false);
  });

  it('hand times must be positive integers', () => {
    expect(handTimesSchema.safeParse({ hand_time_1_ms: 47230, hand_time_2_ms: 47245 }).success).toBe(
      true,
    );
    expect(handTimesSchema.safeParse({ hand_time_1_ms: 0, hand_time_2_ms: 1 }).success).toBe(false);
    expect(handTimesSchema.safeParse({ hand_time_1_ms: 1.5, hand_time_2_ms: 1 }).success).toBe(
      false,
    );
  });
});
