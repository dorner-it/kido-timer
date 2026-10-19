// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * "Eigene Eingaben" of a judge station: merges
 * - locally queued submits (offline queue, not yet on the server),
 * - locally known sent submits (incl. the ones the judge withdrew), and
 * - the server's penalties of the current runs (`state.penalties`,
 *   filtered to the judge's station) which carry the review status.
 *
 * Status shown to the judge:
 * - `sending`        – in the offline queue ("wird gesendet …")
 * - `pending`        – on the server, not reviewed ("ausstehend")
 * - `approved`       – confirmed by the FBL ("bestätigt")
 * - `rejected`       – withdrawn by the FBL ("zurückgenommen", crossed out)
 * - `withdrawn`      – withdrawn by the judge ("zurückgenommen", crossed out)
 */

import type { Penalty, PenaltyKind, PenaltyStatus } from './api';
import { penaltyKind, type CatalogueIndex } from './penalties';
import type { StationKind } from './stations';

export type SubmissionStatus = 'sending' | 'pending' | 'approved' | 'rejected' | 'withdrawn';

export const SUBMISSION_STATUS_LABELS: Record<SubmissionStatus, string> = {
  sending: 'wird gesendet …',
  pending: 'ausstehend',
  approved: 'bestätigt',
  rejected: 'zurückgenommen',
  withdrawn: 'zurückgenommen',
};

/** Server status → judge view. */
export function submissionStatusFromPenalty(status: PenaltyStatus): SubmissionStatus {
  switch (status) {
    case 'approved':
      return 'approved';
    case 'rejected':
      return 'rejected';
    default:
      return 'pending';
  }
}

/** Crossed out in the list. */
export function isStruck(status: SubmissionStatus): boolean {
  return status === 'rejected' || status === 'withdrawn';
}

/** Only unreviewed entries can be withdrawn (queued ones are discarded). */
export function canWithdraw(status: SubmissionStatus): boolean {
  return status === 'sending' || status === 'pending';
}

/** Who took it back (for the sub-line). */
export function struckByLabel(status: SubmissionStatus): string | null {
  if (status === 'rejected') return 'von der Wettkampfleitung';
  if (status === 'withdrawn') return 'von dir';
  return null;
}

/** Locally remembered submit (React state of the judge app). */
export interface LocalSubmission {
  client_id: string;
  /** Server id once sent (= client_id with the 2027 backend). */
  id: string | null;
  code: string | null;
  kind: PenaltyKind;
  label: string;
  run_id: string;
  station: StationKind;
  at: number;
  state: 'sending' | 'sent' | 'withdrawn';
}

export interface SubmissionRow {
  key: string;
  /** Server id for withdraw; `null` while only queued. */
  id: string | null;
  client_id: string;
  kind: PenaltyKind;
  label: string;
  run_id: string;
  status: SubmissionStatus;
  at: number;
}

type ServerPenalty = Pick<
  Penalty,
  'id' | 'run_id' | 'station' | 'status' | 'code' | 'reason_code' | 'created_at' | 'delta_ms'
> & { kind?: unknown; is_disqualification?: boolean | null };

function parseTime(iso: string | null | undefined): number {
  const t = iso ? Date.parse(iso) : NaN;
  return Number.isFinite(t) ? t : 0;
}

function serverLabel(p: ServerPenalty, catalogue: CatalogueIndex): string {
  if (p.code) return catalogue.get(p.code)?.label_de ?? p.code;
  if (p.reason_code) return p.reason_code;
  return penaltyKind(p) === 'fv' ? 'Fehlversuch' : 'Strafzeit';
}

/**
 * Builds the list (newest first) for the given runs.
 *
 * - `server === undefined` → backend without `penalties`; local `sent`
 *   entries are shown as `pending`.
 * - `server` given → it is the truth for everything it contains. A local
 *   `sent` entry missing from it stays `pending` (state not refreshed
 *   yet); a local `withdrawn` entry stays `withdrawn`.
 * - Server penalties of other stations or other runs are ignored.
 */
export function buildSubmissionRows(opts: {
  local: readonly LocalSubmission[];
  server: readonly ServerPenalty[] | undefined;
  station: StationKind | null;
  runIds: readonly string[];
  catalogue: CatalogueIndex;
}): SubmissionRow[] {
  const { local, server, station, runIds, catalogue } = opts;
  const inScope = (runId: string) => runIds.includes(runId);
  const rows = new Map<string, SubmissionRow>();

  for (const l of local) {
    if (!inScope(l.run_id) || (station && l.station !== station)) continue;
    const status: SubmissionStatus =
      l.state === 'sending' ? 'sending' : l.state === 'withdrawn' ? 'withdrawn' : 'pending';
    rows.set(l.id ?? l.client_id, {
      key: l.client_id,
      id: l.id,
      client_id: l.client_id,
      kind: l.kind,
      label: l.label,
      run_id: l.run_id,
      status,
      at: l.at,
    });
  }

  for (const p of server ?? []) {
    if (!inScope(p.run_id)) continue;
    if (station && p.station !== station) continue;
    const existing = rows.get(p.id);
    if (existing?.status === 'withdrawn') continue;
    rows.set(p.id, {
      key: existing?.key ?? p.id,
      id: p.id,
      client_id: existing?.client_id ?? p.id,
      kind: penaltyKind(p),
      label: existing?.label ?? serverLabel(p, catalogue),
      run_id: p.run_id,
      status: submissionStatusFromPenalty(p.status),
      at: existing?.at ?? parseTime(p.created_at),
    });
  }

  return [...rows.values()].sort((a, b) => b.at - a.at);
}

/** Counts for the section header, e.g. `{ pending: 2, approved: 1 }`. */
export function countByStatus(rows: readonly SubmissionRow[]): Record<SubmissionStatus, number> {
  const out: Record<SubmissionStatus, number> = {
    sending: 0,
    pending: 0,
    approved: 0,
    rejected: 0,
    withdrawn: 0,
  };
  for (const r of rows) out[r.status] += 1;
  return out;
}

/** Keeps the local list small: drop entries of runs no longer current
 *  (the server list covers anything still relevant). */
export function pruneLocal(
  local: readonly LocalSubmission[],
  runIds: readonly string[],
  max = 50,
): LocalSubmission[] {
  return local.filter((l) => l.state === 'sending' || runIds.includes(l.run_id)).slice(0, max);
}
