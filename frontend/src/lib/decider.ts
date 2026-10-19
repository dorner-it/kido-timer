// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Pure helpers for the judge (Strafrichter) station app.
 */

import type { CatalogueEntry, StationPenaltyInput } from './api';
import { PENALTY_KIND_SHORT } from './penalties';
import type { StationKind } from './stations';

interface CurrentRunLike {
  run: { id: string; lane: number | null; run_number: number };
  team: { name: string };
}

export type SubmitTarget =
  | { ok: true; run_id: string }
  | { ok: false; reason: 'no_run' | 'multiple_runs'; message: string };

/**
 * Which run a station submission goes to.
 *
 * TODO(PR 2 – Bahnauswahl): with more than one current run the judge
 * must pick the lane/team (judge_mode `per_station`) or only sees the
 * run on their own lane (`per_station_and_lane`). That selection plugs
 * in here: accept the selected run id and return it instead of
 * refusing. Until then only the single-run case can be submitted.
 */
export function resolveSubmitTarget(currentRuns: readonly CurrentRunLike[]): SubmitTarget {
  if (currentRuns.length === 0) {
    return { ok: false, reason: 'no_run', message: 'Aktuell läuft kein Lauf.' };
  }
  if (currentRuns.length > 1) {
    return {
      ok: false,
      reason: 'multiple_runs',
      message:
        'Mehrere Läufe gleichzeitig aktiv. Die Auswahl der Bahn folgt in einem ' +
        'späteren Update — bitte den Fehler direkt beim Organisator melden.',
    };
  }
  return { ok: true, run_id: currentRuns[0]!.run.id };
}

/** `"Bahn 1 · FF Musterstadt"`; without lane just the team name. */
export function currentRunLabel(cr: CurrentRunLike): string {
  return cr.run.lane != null ? `Bahn ${cr.run.lane} · ${cr.team.name}` : cr.team.name;
}

/** 2027 submission: the catalogue code decides kind and delta server-side. */
export function catalogueSubmitBody(
  station: StationKind,
  entry: Pick<CatalogueEntry, 'code'>,
  runId: string | undefined,
  clientId: string,
): StationPenaltyInput {
  const body: StationPenaltyInput = { station, code: entry.code, client_id: clientId };
  if (runId) body.run_id = runId;
  return body;
}

/** Pre-2027 fallback (no catalogue available from the backend). */
export function legacySubmitBody(
  station: StationKind,
  opts: { deltaMs: number; reasonCode: string | null; dq?: boolean },
  runId: string | undefined,
  clientId: string,
): StationPenaltyInput {
  const body: StationPenaltyInput = {
    station,
    client_id: clientId,
    reason_code: opts.reasonCode,
  };
  if (runId) body.run_id = runId;
  if (opts.dq) body.is_disqualification = true;
  else body.delta_ms = opts.deltaMs;
  return body;
}

/** Label for the "Eigene Eingaben" list, e.g. `"SZ · 1 Knoten falsch"`. */
export function catalogueEntryLabel(entry: CatalogueEntry): string {
  return `${PENALTY_KIND_SHORT[entry.kind]} · ${entry.label_de}`;
}
