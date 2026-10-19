// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import type { CatalogueEntry, PenaltyStatus } from '../api';
import {
  SUBMISSION_STATUS_LABELS,
  buildSubmissionRows,
  canWithdraw,
  countByStatus,
  isStruck,
  pruneLocal,
  struckByLabel,
  submissionStatusFromPenalty,
  type LocalSubmission,
  type SubmissionStatus,
} from '../judge-submissions';
import { indexCatalogue } from '../penalties';

const catalogue: CatalogueEntry[] = [
  { code: 'knotengestell.ein_knoten_falsch', station: 'knotengestell', kind: 'sz', label_de: '1 Knoten falsch' },
  { code: 'knotengestell.mehrere_knoten_falsch', station: 'knotengestell', kind: 'fv', label_de: 'Mehrere Knoten falsch' },
];
const idx = indexCatalogue(catalogue);

const local = (over: Partial<LocalSubmission> = {}): LocalSubmission => ({
  client_id: 'c1',
  id: 'c1',
  code: 'knotengestell.ein_knoten_falsch',
  kind: 'sz',
  label: '1 Knoten falsch',
  run_id: 'r1',
  station: 'knotengestell',
  at: 1000,
  state: 'sent',
  ...over,
});

const server = (id: string, status: PenaltyStatus, over: Record<string, unknown> = {}) => ({
  id,
  run_id: 'r1',
  station: 'knotengestell' as const,
  status,
  code: 'knotengestell.ein_knoten_falsch',
  reason_code: null,
  created_at: '2027-06-12T10:00:00Z',
  delta_ms: 15000,
  kind: 'sz',
  ...over,
});

describe('status mapping', () => {
  it.each<[PenaltyStatus, SubmissionStatus]>([
    ['pending', 'pending'],
    ['approved', 'approved'],
    ['rejected', 'rejected'],
  ])('%s → %s', (p, s) => {
    expect(submissionStatusFromPenalty(p)).toBe(s);
  });

  it('labels (German)', () => {
    expect(SUBMISSION_STATUS_LABELS).toEqual({
      sending: 'wird gesendet …',
      pending: 'ausstehend',
      approved: 'bestätigt',
      rejected: 'zurückgenommen',
      withdrawn: 'zurückgenommen',
    });
  });

  it.each<[SubmissionStatus, boolean, boolean, string | null]>([
    ['sending', false, true, null],
    ['pending', false, true, null],
    ['approved', false, false, null],
    ['rejected', true, false, 'von der Wettkampfleitung'],
    ['withdrawn', true, false, 'von dir'],
  ])('%s: struck=%s withdrawable=%s by=%s', (s, struck, withdraw, by) => {
    expect(isStruck(s)).toBe(struck);
    expect(canWithdraw(s)).toBe(withdraw);
    expect(struckByLabel(s)).toBe(by);
  });
});

describe('buildSubmissionRows', () => {
  const base = { station: 'knotengestell' as const, runIds: ['r1', 'r2'], catalogue: idx };

  it('backend without penalties: local sent → pending, sending → sending', () => {
    const rows = buildSubmissionRows({
      ...base,
      local: [local(), local({ client_id: 'c2', id: null, state: 'sending', at: 2000 })],
      server: undefined,
    });
    expect(rows.map((r) => [r.client_id, r.status])).toEqual([
      ['c2', 'sending'],
      ['c1', 'pending'],
    ]);
  });

  it('server status overrides local (approved / rejected)', () => {
    const rows = buildSubmissionRows({
      ...base,
      local: [local(), local({ client_id: 'c2', id: 'c2', at: 2000 })],
      server: [server('c1', 'approved'), server('c2', 'rejected')],
    });
    expect(Object.fromEntries(rows.map((r) => [r.client_id, r.status]))).toEqual({
      c1: 'approved',
      c2: 'rejected',
    });
  });

  it('queued (sending) entry becomes server status once the server knows its id', () => {
    const rows = buildSubmissionRows({
      ...base,
      local: [local({ id: null, state: 'sending' })],
      server: [server('c1', 'pending')],
    });
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({ id: 'c1', status: 'pending', label: '1 Knoten falsch' });
  });

  it('local sent missing on the server (refresh pending) stays pending', () => {
    const rows = buildSubmissionRows({ ...base, local: [local()], server: [] });
    expect(rows[0]!.status).toBe('pending');
  });

  it('self-withdrawn stays withdrawn even if a stale server copy is still listed', () => {
    const rows = buildSubmissionRows({
      ...base,
      local: [local({ state: 'withdrawn' })],
      server: [server('c1', 'pending')],
    });
    expect(rows[0]!.status).toBe('withdrawn');
  });

  it('server-only penalties (after reload) are shown with catalogue label', () => {
    const rows = buildSubmissionRows({
      ...base,
      local: [],
      server: [server('p9', 'pending', { code: 'knotengestell.mehrere_knoten_falsch', kind: 'fv', delta_ms: 0 })],
    });
    expect(rows[0]).toMatchObject({ id: 'p9', kind: 'fv', label: 'Mehrere Knoten falsch', status: 'pending' });
  });

  it('server label fallbacks: unknown code, reason_code, kind', () => {
    const rows = buildSubmissionRows({
      ...base,
      local: [],
      server: [
        server('a', 'pending', { code: 'x.unknown', created_at: '2027-06-12T10:00:03Z' }),
        server('b', 'pending', { code: null, reason_code: 'legacy', created_at: '2027-06-12T10:00:02Z' }),
        server('c', 'pending', { code: null, kind: undefined, is_disqualification: true, created_at: '2027-06-12T10:00:01Z' }),
      ],
    });
    expect(rows.map((r) => r.label)).toEqual(['x.unknown', 'legacy', 'Fehlversuch']);
    expect(rows[2]!.kind).toBe('fv');
  });

  it('filters other stations and other runs', () => {
    const rows = buildSubmissionRows({
      ...base,
      local: [local({ client_id: 'old', id: 'old', run_id: 'r-old' })],
      server: [
        server('s1', 'pending', { station: 'startlinie' }),
        server('s2', 'pending', { run_id: 'r-old' }),
        server('s3', 'pending', { run_id: 'r2' }),
      ],
    });
    expect(rows.map((r) => r.id)).toEqual(['s3']);
  });

  it('newest first', () => {
    const rows = buildSubmissionRows({
      ...base,
      local: [local({ client_id: 'a', id: 'a', at: 1 }), local({ client_id: 'b', id: 'b', at: 3 }), local({ client_id: 'c', id: 'c', at: 2 })],
      server: undefined,
    });
    expect(rows.map((r) => r.client_id)).toEqual(['b', 'c', 'a']);
  });

  it('without station filter keeps all stations', () => {
    const rows = buildSubmissionRows({
      ...base,
      station: null,
      local: [],
      server: [server('s1', 'pending', { station: 'startlinie' }), server('s2', 'approved')],
    });
    expect(rows).toHaveLength(2);
  });
});

describe('countByStatus / pruneLocal', () => {
  it('counts', () => {
    const rows = buildSubmissionRows({
      station: 'knotengestell',
      runIds: ['r1'],
      catalogue: idx,
      local: [local({ client_id: 'q', id: null, state: 'sending' })],
      server: [server('a', 'approved'), server('b', 'rejected'), server('c', 'pending')],
    });
    expect(countByStatus(rows)).toEqual({ sending: 1, pending: 1, approved: 1, rejected: 1, withdrawn: 0 });
  });

  it('prune keeps sending entries and current runs only', () => {
    const l = [
      local({ client_id: 'a', run_id: 'r-old' }),
      local({ client_id: 'b', run_id: 'r-old', state: 'sending' }),
      local({ client_id: 'c', run_id: 'r1' }),
    ];
    expect(pruneLocal(l, ['r1']).map((x) => x.client_id)).toEqual(['b', 'c']);
  });

  it('prune caps the length', () => {
    const l = Array.from({ length: 60 }, (_, i) => local({ client_id: `c${i}` }));
    expect(pruneLocal(l, ['r1'], 50)).toHaveLength(50);
  });
});
