// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import type { CatalogueEntry, Penalty, PenaltyKind, PenaltyStatus } from '../api';
import { indexCatalogue } from '../penalties';
import {
  REVIEW_STATUS_LABELS,
  canReview,
  canUndoReview,
  compareInstants,
  countsTowardsResult,
  groupByStation,
  markRunApproved,
  needsFvConfirmation,
  penaltiesByRun,
  penaltyLine,
  penaltyStation,
  replacePenalty,
  resultTimeMs,
  summarizeRun,
  summaryText,
  summaryTone,
  takeOverHeadline,
  takeOverPlan,
} from '../run-review';

let seq = 0;
function pen(
  kind: PenaltyKind,
  status: PenaltyStatus,
  extra: Partial<Penalty> = {},
): Penalty {
  seq += 1;
  return {
    id: `p${seq}`,
    event_id: 'e1',
    discipline_id: 'd1',
    run_id: 'r1',
    team_id: 't1',
    runner_id: null,
    station: 'knotengestell',
    kind,
    code: null,
    is_disqualification: kind === 'fv',
    decider_slot: 0,
    delta_ms: kind === 'sz' ? 15_000 : 0,
    reason_code: null,
    status,
    created_at: `2027-05-01T10:00:${String(seq % 60).padStart(2, '0')}Z`,
    reviewed_at: null,
    reviewed_by: null,
    ...extra,
  };
}

const catalogue: CatalogueEntry[] = [
  { code: 'knotengestell.ein_knoten_falsch', station: 'knotengestell', kind: 'sz', label_de: '1 Knoten falsch' },
  { code: 'knotengestell.mehrere_knoten_falsch', station: 'knotengestell', kind: 'fv', label_de: 'Mehrere Knoten falsch' },
  { code: 'startlinie.fruehstart', station: 'startlinie', kind: 'sz', label_de: 'Ein WK startet vor dem Abschlagen' },
];
const idx = indexCatalogue(catalogue);

describe('labels & predicates', () => {
  it('FBL wording', () => {
    expect(REVIEW_STATUS_LABELS).toEqual({
      pending: 'Offen',
      approved: 'Bestätigt',
      rejected: 'Zurückgenommen',
    });
  });

  it('counts mirrors kido_core PenaltyStatus::counts', () => {
    expect(countsTowardsResult('pending')).toBe(true);
    expect(countsTowardsResult('approved')).toBe(true);
    expect(countsTowardsResult('rejected')).toBe(false);
  });

  it('canReview only pending', () => {
    expect(canReview({ status: 'pending' })).toBe(true);
    expect(canReview({ status: 'approved' })).toBe(false);
    expect(canReview({ status: 'rejected' })).toBe(false);
  });

  it('needsFvConfirmation for FV incl. legacy is_disqualification', () => {
    expect(needsFvConfirmation({ kind: 'fv', is_disqualification: true })).toBe(true);
    expect(needsFvConfirmation({ kind: 'sz', is_disqualification: false })).toBe(false);
    expect(
      needsFvConfirmation({ kind: undefined as unknown as PenaltyKind, is_disqualification: true }),
    ).toBe(true);
  });
});

describe('summarizeRun', () => {
  it('no penalties → clean & valid', () => {
    const s = summarizeRun([]);
    expect(s).toMatchObject({ total: 0, sz: 0, fv: 0, valid: true, provisional: false, penaltyMs: 0 });
    expect(summaryText(s)).toBe('fehlerfrei · gültig');
    expect(summaryTone(s)).toBe('clean');
  });

  it('two approved SZ → +30 s, valid, final', () => {
    const s = summarizeRun([pen('sz', 'approved'), pen('sz', 'approved')]);
    expect(s).toMatchObject({ sz: 2, confirmedSz: 2, pendingSz: 0, penaltyMs: 30_000, valid: true, provisional: false });
    expect(summaryText(s)).toBe('2 SZ (+30 s) · gültig');
    expect(summaryTone(s)).toBe('penalty');
  });

  it('pending SZ count but make it provisional', () => {
    const s = summarizeRun([pen('sz', 'approved'), pen('sz', 'pending')]);
    expect(s).toMatchObject({ sz: 2, pendingSz: 1, confirmedSz: 1, pending: 1, provisional: true });
    expect(summaryText(s)).toBe('2 SZ (+30 s) · gültig · vorläufig');
    expect(summaryTone(s)).toBe('provisional');
  });

  it('rejected SZ do not count', () => {
    const s = summarizeRun([pen('sz', 'rejected'), pen('sz', 'approved'), pen('sz', 'rejected')]);
    expect(s).toMatchObject({ sz: 1, rejected: 2, approved: 1, penaltyMs: 15_000 });
    expect(summaryText(s)).toBe('1 SZ (+15 s) · gültig');
  });

  it('only rejected → clean', () => {
    const s = summarizeRun([pen('fv', 'rejected'), pen('sz', 'rejected')]);
    expect(s.valid).toBe(true);
    expect(summaryText(s)).toBe('fehlerfrei · gültig');
  });

  it('approved FV → invalid', () => {
    const s = summarizeRun([pen('fv', 'approved'), pen('sz', 'approved')]);
    expect(s).toMatchObject({ fv: 1, confirmedFv: 1, valid: false, provisional: false });
    expect(summaryText(s)).toBe('FV · ungültig');
    expect(summaryTone(s)).toBe('invalid');
  });

  it('pending FV → invalid but provisional', () => {
    const s = summarizeRun([pen('fv', 'pending')]);
    expect(s).toMatchObject({ fv: 1, pendingFv: 1, valid: false, provisional: true });
    expect(summaryText(s)).toBe('FV · ungültig · vorläufig');
    expect(summaryTone(s)).toBe('invalid');
  });

  it('rejected FV keeps the run valid', () => {
    const s = summarizeRun([pen('fv', 'rejected'), pen('sz', 'pending')]);
    expect(s.valid).toBe(true);
    expect(s.fv).toBe(0);
    expect(summaryText(s)).toBe('1 SZ (+15 s) · gültig · vorläufig');
  });

  it('legacy SZ delta_ms is ignored — always 15 s per SZ', () => {
    const s = summarizeRun([pen('sz', 'approved', { delta_ms: 5_000 }), pen('sz', 'approved', { delta_ms: 999_000 })]);
    expect(s.penaltyMs).toBe(30_000);
  });

  it('legacy row without kind uses is_disqualification', () => {
    const legacy = { ...pen('sz', 'approved'), kind: undefined as unknown as PenaltyKind, is_disqualification: true };
    expect(summarizeRun([legacy]).valid).toBe(false);
  });

  it('many SZ', () => {
    const s = summarizeRun(Array.from({ length: 7 }, () => pen('sz', 'approved')));
    expect(summaryText(s)).toBe('7 SZ (+105 s) · gültig');
  });
});

describe('resultTimeMs', () => {
  it('measured + 15 s × SZ', () => {
    const s = summarizeRun([pen('sz', 'approved'), pen('sz', 'pending'), pen('sz', 'rejected')]);
    expect(resultTimeMs({ original_time_ms: 47_240 }, s)).toBe(77_240);
  });
  it('null when invalid', () => {
    expect(resultTimeMs({ original_time_ms: 47_240 }, summarizeRun([pen('fv', 'pending')]))).toBeNull();
  });
  it('null without time', () => {
    expect(resultTimeMs({ original_time_ms: null }, summarizeRun([]))).toBeNull();
  });
  it('no penalty → measured', () => {
    expect(resultTimeMs({ original_time_ms: 50_000 }, summarizeRun([]))).toBe(50_000);
  });
});

describe('grouping', () => {
  it('penaltiesByRun', () => {
    const a = pen('sz', 'pending', { run_id: 'a' });
    const b = pen('sz', 'pending', { run_id: 'b' });
    const c = pen('fv', 'pending', { run_id: 'a' });
    const m = penaltiesByRun([a, b, c]);
    expect(m.get('a')).toEqual([a, c]);
    expect(m.get('b')).toEqual([b]);
    expect(m.get('x')).toBeUndefined();
  });

  it('penaltyStation falls back to the catalogue', () => {
    expect(penaltyStation({ station: null, code: 'startlinie.fruehstart' }, idx)).toBe('startlinie');
    expect(penaltyStation({ station: 'podest', code: 'startlinie.fruehstart' }, idx)).toBe('podest');
    expect(penaltyStation({ station: null, code: 'unknown.code' }, idx)).toBeNull();
    expect(penaltyStation({ station: null, code: null }, idx)).toBeNull();
  });

  it('groupByStation: canonical order, unknown last, items by time', () => {
    const late = pen('sz', 'pending', { station: 'knotengestell', created_at: '2027-05-01T10:05:00Z' });
    const early = pen('sz', 'pending', { station: 'knotengestell', created_at: '2027-05-01T10:01:00Z' });
    const start = pen('sz', 'pending', { station: 'startlinie' });
    const none = pen('sz', 'pending', { station: null });
    const viaCode = pen('sz', 'pending', { station: null, code: 'startlinie.fruehstart' });
    const groups = groupByStation([late, none, early, start, viaCode], idx);
    expect(groups.map((g) => g.station)).toEqual(['startlinie', 'knotengestell', null]);
    expect(groups.map((g) => g.label)).toEqual(['Startlinie/Zeitnehmer', 'Knotengestell', 'Ohne Station']);
    expect(groups[0]!.items).toHaveLength(2);
    expect(groups[1]!.items).toEqual([early, late]);
    expect(groups[2]!.items).toEqual([none]);
  });

  it('groupByStation of nothing', () => {
    expect(groupByStation([], idx)).toEqual([]);
  });
});

describe('takeOverPlan (Übernehmen = bulk-approve)', () => {
  it('lists pending, counts SZ/FV, needs FV ack', () => {
    const list = [pen('sz', 'pending'), pen('fv', 'pending'), pen('sz', 'approved'), pen('sz', 'rejected')];
    const plan = takeOverPlan(list);
    expect(plan.pending).toHaveLength(2);
    expect(plan.pendingSz).toBe(1);
    expect(plan.pendingFv).toBe(1);
    expect(plan.needsFvAck).toBe(true);
    expect(plan.after).toMatchObject({ provisional: false, valid: false, pending: 0, approved: 3, rejected: 1 });
    expect(takeOverHeadline(plan)).toBe('2 offene Fehler (1 SZ, 1 FV) werden bestätigt.');
  });

  it('only SZ pending → no FV ack', () => {
    const plan = takeOverPlan([pen('sz', 'pending')]);
    expect(plan.needsFvAck).toBe(false);
    expect(summaryText(plan.after)).toBe('1 SZ (+15 s) · gültig');
    expect(takeOverHeadline(plan)).toBe('1 offener Fehler (1 SZ) wird bestätigt.');
  });

  it('nothing pending', () => {
    const plan = takeOverPlan([pen('sz', 'approved'), pen('fv', 'rejected')]);
    expect(plan.pending).toEqual([]);
    expect(plan.needsFvAck).toBe(false);
    expect(takeOverHeadline(plan)).toBe('Keine offenen Fehler – der Lauf wird so übernommen.');
  });

  it('after-summary equals provisional summary minus the provisional flag', () => {
    const list = [pen('sz', 'pending'), pen('sz', 'pending'), pen('sz', 'rejected')];
    const before = summarizeRun(list);
    const { after } = takeOverPlan(list);
    expect(after.sz).toBe(before.sz);
    expect(after.penaltyMs).toBe(before.penaltyMs);
    expect(after.valid).toBe(before.valid);
    expect(before.provisional).toBe(true);
    expect(after.provisional).toBe(false);
  });

  it('does not mutate input', () => {
    const p = pen('sz', 'pending');
    takeOverPlan([p]);
    expect(p.status).toBe('pending');
  });
});

describe('penaltyLine', () => {
  it('catalogue label with station and kind', () => {
    const p = pen('fv', 'pending', { code: 'knotengestell.mehrere_knoten_falsch' });
    expect(penaltyLine(p, idx)).toBe('Knotengestell: FV – Mehrere Knoten falsch');
  });
  it('legacy row', () => {
    const p = pen('sz', 'pending', { station: null, reason_code: 'Schlauch' });
    expect(penaltyLine(p, idx)).toBe('Ohne Station: SZ – Schlauch');
  });
});

describe('list updates', () => {
  it('replacePenalty replaces by id or appends', () => {
    const a = pen('sz', 'pending');
    const b = pen('sz', 'pending');
    const a2 = { ...a, status: 'approved' as const };
    expect(replacePenalty([a, b], a2)).toEqual([a2, b]);
    const c = pen('fv', 'pending');
    expect(replacePenalty([a], c)).toEqual([a, c]);
  });

  it('markRunApproved only touches pending of that run', () => {
    const a = pen('sz', 'pending', { run_id: 'r1' });
    const b = pen('sz', 'rejected', { run_id: 'r1' });
    const c = pen('sz', 'pending', { run_id: 'r2' });
    const out = markRunApproved([a, b, c], 'r1');
    expect(out.map((p) => p.status)).toEqual(['approved', 'rejected', 'pending']);
    expect(a.status).toBe('pending');
  });
});

describe('canUndoReview (mirror of kido_core::penalty::can_undo_review)', () => {
  const acc = '2027-06-12T10:00:00Z';
  const run = (penalties_accepted_at: string | null) => ({ penalties_accepted_at });

  it('pending → never', () => {
    expect(canUndoReview(pen('sz', 'pending'), run(null))).toBe(false);
    expect(canUndoReview(pen('sz', 'pending', { reviewed_at: '2027-06-12T11:00:00Z' }), run(acc))).toBe(false);
  });

  it('no Übernehmen yet → every decision', () => {
    expect(canUndoReview(pen('sz', 'approved', { reviewed_at: acc }), run(null))).toBe(true);
    expect(canUndoReview(pen('fv', 'rejected', { reviewed_at: acc }), run(null))).toBe(true);
    expect(canUndoReview(pen('sz', 'approved', { reviewed_at: null }), run(null))).toBe(true);
    expect(canUndoReview(pen('sz', 'approved', { reviewed_at: acc }), undefined)).toBe(true);
    // Older backend: field missing on the run.
    expect(canUndoReview(pen('sz', 'approved', { reviewed_at: acc }), {} as never)).toBe(true);
  });

  it('decided before Übernehmen → frozen', () => {
    expect(canUndoReview(pen('sz', 'approved', { reviewed_at: '2027-06-12T09:59:59Z' }), run(acc))).toBe(false);
    expect(canUndoReview(pen('sz', 'rejected', { reviewed_at: '2027-06-12T09:00:00Z' }), run(acc))).toBe(false);
  });

  it('decided at the Übernehmen instant (bulk-approved) → frozen', () => {
    const at = '2027-06-12T10:00:00.123456789Z';
    expect(canUndoReview(pen('sz', 'approved', { reviewed_at: at }), run(at))).toBe(false);
  });

  it('decided after Übernehmen → undoable, with sub-ms precision', () => {
    expect(canUndoReview(pen('sz', 'approved', { reviewed_at: '2027-06-12T10:00:01Z' }), run(acc))).toBe(true);
    expect(
      canUndoReview(
        pen('fv', 'rejected', { reviewed_at: '2027-06-12T10:00:00.123456790Z' }),
        run('2027-06-12T10:00:00.123456789Z'),
      ),
    ).toBe(true);
  });

  it('decided without reviewed_at on a taken-over run → frozen', () => {
    expect(canUndoReview(pen('sz', 'approved', { reviewed_at: null }), run(acc))).toBe(false);
  });

  it('compareInstants handles offsets and fractions', () => {
    expect(compareInstants('2027-06-12T10:00:00Z', '2027-06-12T12:00:00+02:00')).toBe(0);
    expect(compareInstants('2027-06-12T10:00:00.5Z', '2027-06-12T10:00:00.4999Z')).toBeGreaterThan(0);
    expect(compareInstants('2027-06-12T10:00:00Z', '2027-06-12T10:00:00.000000001Z')).toBeLessThan(0);
    expect(Number.isNaN(compareInstants('kaputt', '2027-06-12T10:00:00Z'))).toBe(true);
  });
});
