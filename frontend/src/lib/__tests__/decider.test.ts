// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  catalogueEntryLabel,
  catalogueSubmitBody,
  currentRunLabel,
  legacySubmitBody,
  resolveSubmitTarget,
} from '../decider';

const cr = (id: string, lane: number | null, name: string) => ({
  run: { id, lane, run_number: 1 },
  team: { name },
});

describe('resolveSubmitTarget', () => {
  it('no run → error', () => {
    const t = resolveSubmitTarget([]);
    expect(t.ok).toBe(false);
    if (!t.ok) {
      expect(t.reason).toBe('no_run');
      expect(t.message).toBe('Aktuell läuft kein Lauf.');
    }
  });

  it('single run → its id', () => {
    expect(resolveSubmitTarget([cr('run1', 1, 'A')])).toEqual({ ok: true, run_id: 'run1' });
  });

  it('multiple runs → refused until the lane selection exists', () => {
    const t = resolveSubmitTarget([cr('run1', 1, 'A'), cr('run2', 2, 'B')]);
    expect(t.ok).toBe(false);
    if (!t.ok) expect(t.reason).toBe('multiple_runs');
  });
});

describe('currentRunLabel', () => {
  it('with and without lane', () => {
    expect(currentRunLabel(cr('r', 2, 'FF Musterstadt'))).toBe('Bahn 2 · FF Musterstadt');
    expect(currentRunLabel(cr('r', null, 'FF Musterstadt'))).toBe('FF Musterstadt');
  });
});

describe('submit bodies', () => {
  it('catalogue body carries station, code, run_id and client_id only', () => {
    expect(
      catalogueSubmitBody('knotengestell', { code: 'knotengestell.ein_knoten_falsch' }, 'run1', 'c1'),
    ).toEqual({
      station: 'knotengestell',
      code: 'knotengestell.ein_knoten_falsch',
      run_id: 'run1',
      client_id: 'c1',
    });
  });

  it('catalogue body omits run_id when unknown', () => {
    expect(catalogueSubmitBody('podest', { code: 'x' }, undefined, 'c1')).toEqual({
      station: 'podest',
      code: 'x',
      client_id: 'c1',
    });
  });

  it('legacy time penalty', () => {
    expect(
      legacySubmitBody('startlinie', { deltaMs: 5000, reasonCode: 'kleiner_fehler' }, 'run1', 'c1'),
    ).toEqual({
      station: 'startlinie',
      client_id: 'c1',
      reason_code: 'kleiner_fehler',
      run_id: 'run1',
      delta_ms: 5000,
    });
  });

  it('legacy DQ sends the flag instead of a delta', () => {
    const b = legacySubmitBody('angriffslinie', { deltaMs: 0, reasonCode: 'dsq', dq: true }, undefined, 'c1');
    expect(b.is_disqualification).toBe(true);
    expect(b.delta_ms).toBeUndefined();
    expect(b.run_id).toBeUndefined();
  });

  it('catalogueEntryLabel', () => {
    expect(
      catalogueEntryLabel({ code: 'a', station: 'knotengestell', kind: 'fv', label_de: 'Mehrere Knoten falsch' }),
    ).toBe('FV · Mehrere Knoten falsch');
  });
});
