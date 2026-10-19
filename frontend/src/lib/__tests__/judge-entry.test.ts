// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import { resolveSubmitTarget } from '../decider';
import {
  FV_CONFIRM_TITLE,
  IDLE,
  cancelEntry,
  choiceButtonLabel,
  chooseRun,
  flowTitle,
  isDialogOpen,
  reconcileFlow,
  runChoice,
  startEntry,
  type EntryFlow,
  type EntryLike,
} from '../judge-entry';

const cr = (id: string, lane: number | null, name: string) => ({
  run: { id, lane, run_number: 1 },
  team: { name },
});
const L1 = cr('r1', 1, 'FF Nord');
const L2 = cr('r2', 2, 'FF Süd');

const SZ: EntryLike = { code: 'knotengestell.ein_knoten_falsch', kind: 'sz', label_de: '1 Knoten falsch' };
const FV: EntryLike = { code: 'knotengestell.mehrere_knoten_falsch', kind: 'fv', label_de: 'Mehrere Knoten falsch' };

describe('runChoice', () => {
  it('labels with lane and team', () => {
    expect(runChoice(L1)).toEqual({ run_id: 'r1', lane: 1, label: 'Bahn 1 · FF Nord' });
    expect(runChoice(cr('x', null, 'Solo'))).toEqual({ run_id: 'x', lane: null, label: 'Solo' });
  });
});

describe('startEntry', () => {
  it('SZ with one run → submit immediately (one tap)', () => {
    expect(startEntry(SZ, resolveSubmitTarget([L1]))).toEqual({
      step: 'submit',
      entry: SZ,
      run_id: 'r1',
    });
  });

  it('SZ with two runs → choose_run', () => {
    const f = startEntry(SZ, resolveSubmitTarget([L1, L2]));
    expect(f.step).toBe('choose_run');
    if (f.step === 'choose_run') {
      expect(f.choices.map((c) => c.label)).toEqual(['Bahn 1 · FF Nord', 'Bahn 2 · FF Süd']);
    }
  });

  it('FV with one run → confirm with a single choice', () => {
    const f = startEntry(FV, resolveSubmitTarget([L1]));
    expect(f.step).toBe('confirm_fv');
    if (f.step === 'confirm_fv') expect(f.choices.map((c) => c.run_id)).toEqual(['r1']);
  });

  it('FV with two runs → confirm with both lanes (lane tap = confirmation)', () => {
    const f = startEntry(FV, resolveSubmitTarget([L1, L2]));
    expect(f.step).toBe('confirm_fv');
    if (f.step === 'confirm_fv') expect(f.choices).toHaveLength(2);
  });

  it('FV never submits on the first tap', () => {
    for (const runs of [[L1], [L1, L2]]) {
      expect(startEntry(FV, resolveSubmitTarget(runs)).step).not.toBe('submit');
    }
  });

  it('no run → blocked with message', () => {
    expect(startEntry(SZ, resolveSubmitTarget([]))).toEqual({
      step: 'blocked',
      message: 'Aktuell läuft kein Lauf.',
    });
  });

  it('lane judge without run on the lane → blocked', () => {
    const t = resolveSubmitTarget([L1], {
      judgeMode: 'per_station_and_lane',
      laneCount: 2,
      myLane: 2,
    });
    expect(startEntry(FV, t).step).toBe('blocked');
  });

  it('lane judge with run on own lane → SZ direct', () => {
    const t = resolveSubmitTarget([L1, L2], {
      judgeMode: 'per_station_and_lane',
      laneCount: 2,
      myLane: 2,
    });
    expect(startEntry(SZ, t)).toMatchObject({ step: 'submit', run_id: 'r2' });
  });
});

describe('chooseRun', () => {
  it('choose_run + valid run → submit', () => {
    const f = startEntry(SZ, resolveSubmitTarget([L1, L2]));
    expect(chooseRun(f, 'r2')).toEqual({ step: 'submit', entry: SZ, run_id: 'r2' });
  });

  it('confirm_fv + confirm → submit', () => {
    const f = startEntry(FV, resolveSubmitTarget([L1]));
    expect(chooseRun(f, 'r1')).toEqual({ step: 'submit', entry: FV, run_id: 'r1' });
  });

  it('unknown run id → unchanged', () => {
    const f = startEntry(FV, resolveSubmitTarget([L1]));
    expect(chooseRun(f, 'nope')).toBe(f);
  });

  it('outside a dialog → unchanged', () => {
    expect(chooseRun(IDLE, 'r1')).toBe(IDLE);
  });
});

describe('cancel / dialog helpers', () => {
  it('cancelEntry → idle', () => {
    expect(cancelEntry()).toEqual({ step: 'idle' });
  });

  it('isDialogOpen only for choose_run / confirm_fv', () => {
    expect(isDialogOpen(IDLE)).toBe(false);
    expect(isDialogOpen(startEntry(SZ, resolveSubmitTarget([L1])))).toBe(false);
    expect(isDialogOpen(startEntry(SZ, resolveSubmitTarget([L1, L2])))).toBe(true);
    expect(isDialogOpen(startEntry(FV, resolveSubmitTarget([L1])))).toBe(true);
    expect(isDialogOpen({ step: 'blocked', message: 'x' })).toBe(false);
  });

  it('titles', () => {
    expect(flowTitle(startEntry(FV, resolveSubmitTarget([L1])))).toBe(FV_CONFIRM_TITLE);
    expect(FV_CONFIRM_TITLE).toBe('Fehlversuch – Lauf ungültig?');
    expect(flowTitle(startEntry(SZ, resolveSubmitTarget([L1, L2])))).toBe('Welche Bahn?');
    expect(flowTitle(IDLE)).toBeNull();
  });

  it('button labels', () => {
    const fv1 = startEntry(FV, resolveSubmitTarget([L1]));
    const fv2 = startEntry(FV, resolveSubmitTarget([L1, L2]));
    const sz2 = startEntry(SZ, resolveSubmitTarget([L1, L2]));
    if (fv1.step !== 'confirm_fv' || fv2.step !== 'confirm_fv' || sz2.step !== 'choose_run') {
      throw new Error('unexpected flow');
    }
    expect(choiceButtonLabel(fv1, fv1.choices[0]!)).toBe('Ja, Fehlversuch melden');
    expect(choiceButtonLabel(fv2, fv2.choices[1]!)).toBe('Ja, Bahn 2 · FF Süd');
    expect(choiceButtonLabel(sz2, sz2.choices[0]!)).toBe('Bahn 1 · FF Nord');
  });
});

describe('reconcileFlow', () => {
  const sz2 = startEntry(SZ, resolveSubmitTarget([L1, L2])) as EntryFlow;

  it('unchanged when all choices are still current', () => {
    expect(reconcileFlow(sz2, ['r1', 'r2'])).toBe(sz2);
  });

  it('drops finished runs', () => {
    const f = reconcileFlow(sz2, ['r2']);
    expect(f.step).toBe('choose_run');
    if (f.step === 'choose_run') expect(f.choices.map((c) => c.run_id)).toEqual(['r2']);
  });

  it('closes when nothing is left', () => {
    expect(reconcileFlow(sz2, [])).toEqual({ step: 'idle' });
  });

  it('ignores non-dialog flows', () => {
    expect(reconcileFlow(IDLE, [])).toBe(IDLE);
  });
});
