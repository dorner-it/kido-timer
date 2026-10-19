// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import {
  normalizeCatalogue,
  normalizeDiscipline,
  normalizeEvent,
  normalizeEventDetail,
  normalizePenalty,
  normalizeRun,
  normalizeRunner,
  normalizeStationState,
  normalizeTeamEntry,
} from './schemas';
import { normalizeResults } from './results';
import type { StationKind } from './stations';
import { getApiBase } from './api-base';

export type { StationKind } from './stations';
export { getApiBase } from './api-base';

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

interface ApiErrorBody {
  error: { code: string; message: string };
}

async function parseError(response: Response): Promise<ApiError> {
  let body: Partial<ApiErrorBody> = {};
  try {
    body = (await response.json()) as Partial<ApiErrorBody>;
  } catch {
    // body is not JSON
  }
  const code = body.error?.code ?? 'unknown_error';
  const message = body.error?.message ?? `Request failed with status ${response.status}`;
  return new ApiError(response.status, code, message);
}

interface RequestOptions extends RequestInit {
  /** Set `true` to bypass the global 401 → /login redirect. Use for probes
   *  (e.g. auth.me on a public page) where 401 is a valid value. */
  allowUnauthorized?: boolean;
}

async function request<T>(
  path: string,
  init: RequestOptions = {},
): Promise<T> {
  const { allowUnauthorized, ...fetchInit } = init;
  const res = await fetch(`${getApiBase()}${path}`, {
    credentials: 'include',
    ...fetchInit,
    headers: {
      ...(fetchInit.body ? { 'Content-Type': 'application/json' } : {}),
      ...(fetchInit.headers ?? {}),
    },
  });
  if (res.status === 401 && !allowUnauthorized) {
    redirectToLogin();
    throw await parseError(res);
  }
  if (!res.ok) {
    throw await parseError(res);
  }
  if (res.status === 204) {
    return undefined as T;
  }
  return (await res.json()) as T;
}

/**
 * Options of every judge (`/api/decider/...`) request: a 401 there means
 * "judge link invalid/rotated" — the judge app shows its own
 * "Zugang ungültig" screen (`judge-access.ts`) instead of the organizer
 * login redirect.
 */
const JUDGE: RequestOptions = { allowUnauthorized: true };

let redirecting = false;
function redirectToLogin() {
  if (typeof window === 'undefined' || redirecting) return;
  if (window.location.pathname === '/login') return;
  redirecting = true;
  const here = window.location.pathname + window.location.search + window.location.hash;
  const params = new URLSearchParams();
  params.set('return_to', here);
  params.set('expired', '1');
  window.location.replace(`/login?${params}`);
}

// ===== domain types — mirror the backend (schema v4, PR 1 contract) =====
//
// Wire contract: docs/contract-pr1-2027.md. Every field new in v4 is
// optional/defaulted on read (see `schemas.ts`), so responses from an
// older backend and pre-2027 rows still load.

/**
 * Discipline-specific timer modes (schema >= 3). Lane counts are
 * configured per discipline (`Discipline.lane_count`).
 */
export type TimerMode = 'gruppenstaffette' | 'loeschangriff';
/** `offline` = on-prem (desktop runs a local server); `on_prem` accepted on input. */
export type SyncMode = 'live' | 'offline';
export type JudgeMode = 'per_station' | 'per_station_and_lane';
export type RunStatus = 'pending' | 'active' | 'completed' | 'cancelled';
export type PenaltyStatus = 'pending' | 'approved' | 'rejected';
/** SZ = +15 s, run stays valid; FV = run invalid. */
export type PenaltyKind = 'sz' | 'fv';
export type TimeSource = 'lichtschranke' | 'handstopp';
/** Legacy only: no longer written by the UI, tolerated on read. */
export type Gender = 'male' | 'female';
/** Wertungsgruppe — ticked by the club on registration, never derived. */
export type Wertungsgruppe = 'wg1' | 'wg2' | 'wg3' | 'wg4' | 'wg5';

export interface AuthenticatedUser {
  sub: string;
  email: string;
  display_name: string;
  created_at: string;
  updated_at: string;
}

/** Tagesveranstaltung — parent of disciplines. */
export interface Event {
  id: string;
  owner_sub: string;
  name: string;
  date: string;
  location: string | null;
  is_active: boolean;
  current_discipline_id: string | null;
  /** Kept for old clients = `current_run_ids[0]`. */
  current_run_id: string | null;
  /** One per active lane (max `lane_count`). Legacy rows: `[current_run_id]` or `[]`. */
  current_run_ids: string[];
  /** Deprecated; kept on the wire for legacy QR codes. */
  decider_tokens: [string, string, string, string];
  /** Single shared strafrichter token (URL-safe base64, 32 raw bytes).
   *  Empty string on legacy rows; backfilled on the next save. */
  station_token: string;
  created_at: string;
  updated_at: string;
}

/** Wettkampf — one discipline within an event. */
export interface Discipline {
  id: string;
  event_id: string;
  name: string;
  mode: TimerMode;
  sync_mode: SyncMode;
  position: number;
  /** Gruppenstaffette 1|2 (default 2), Löschangriff 4. Defaulted on read. */
  lane_count: number;
  /** Default `per_station`. */
  judge_mode: JudgeMode;
  created_at: string;
  updated_at: string;
}

/** Create/update response: lists entries whose lane exceeds `lane_count`. */
export interface DisciplineSaveResponse extends Discipline {
  lane_conflicts?: string[];
}

export interface Team {
  id: string;
  event_id: string;
  name: string;
  position: number;
  created_at: string;
  updated_at: string;
}

/** Append-only log entry written by "Umsetzen". */
export interface WgHistoryEntry {
  from: Wertungsgruppe | null;
  to: Wertungsgruppe | null;
  /** User email or sub. */
  by: string;
  /** RFC 3339. */
  at: string;
  note?: string | null;
}

/** Runner whose 31.12.-age does not fit the entry's WG. Computed on read. */
export interface WgWarning {
  runner_id: string;
  age: number;
  wg: Wertungsgruppe;
  /** `"zu alt"` / `"zu jung"`. */
  reason: string;
}

export interface TeamEntry {
  team_id: string;
  discipline_id: string;
  event_id: string;
  start_number: number;
  lane: number | null;
  /** Ticked by the club. `null` only on legacy rows ("WG fehlt"). */
  wg: Wertungsgruppe | null;
  wg_history: WgHistoryEntry[];
  /** Never blocks saving; empty = all runners fit. */
  wg_warnings: WgWarning[];
  created_at: string;
  updated_at: string;
}

export interface Runner {
  id: string;
  team_id: string;
  event_id: string;
  first_name: string;
  last_name: string;
  birth_date: string;
  position: number;
  /** Legacy, read-only. `null` for new rows; never sent by the UI. */
  gender?: Gender | null;
  created_at: string;
  updated_at: string;
}

export interface Run {
  id: string;
  event_id: string;
  discipline_id: string;
  team_id: string;
  runner_id: string | null;
  run_number: number;
  lane: number | null;
  status: RunStatus;
  original_time_ms: number | null;
  correction_ms: number;
  started_at: string | null;
  ended_at: string | null;
  /** Runners who actually run (subset of the team roster). */
  lineup: string[];
  lineup_confirmed_at: string | null;
  lineup_confirmed_by: 'startlinie' | 'organizer' | null;
  time_source: TimeSource;
  hand_time_1_ms: number | null;
  hand_time_2_ms: number | null;
  /** Last "Übernehmen" (bulk-approve). Decisions up to this instant are
   *  frozen; later ones can still be undone. `null` until the first
   *  Übernehmen (and on older backends / rows). */
  penalties_accepted_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface Penalty {
  id: string;
  event_id: string;
  discipline_id: string;
  run_id: string;
  team_id: string;
  runner_id: string | null;
  /** Canonical station key (legacy keys mapped on read); `null` on very old rows. */
  station: StationKind | null;
  /** Read-time migration: missing → `is_disqualification ? 'fv' : 'sz'`. */
  kind: PenaltyKind;
  /** Catalogue code, e.g. `knotengestell.ein_knoten_falsch`; `null` on legacy rows. */
  code: string | null;
  /** Kept for old clients (= kind == fv). */
  is_disqualification: boolean;
  /** Deprecated identity for legacy slot-token submissions. */
  decider_slot: number;
  /** sz → 15000, fv → 0 (legacy rows: as sent). */
  delta_ms: number;
  reason_code: string | null;
  status: PenaltyStatus;
  created_at: string;
  reviewed_at: string | null;
  reviewed_by: string | null;
}

/** Entry of the static error catalogue (`GET /api/catalogue`). */
export interface CatalogueEntry {
  code: string;
  station: StationKind;
  kind: PenaltyKind;
  label_de: string;
}

/** Full event detail returned by `GET /api/events/:id`. One round-trip
 *  gathers every row beneath the event partition. */
export interface EventDetail {
  event: Event;
  disciplines: Discipline[];
  teams: Team[];
  runners: Runner[];
  team_entries: TeamEntry[];
  runs: Run[];
}

export interface ApiKeySummary {
  key_id: string;
  name: string;
  prefix: string;
  created_at: string;
}

export interface ApiKeyCreatedResponse {
  key: ApiKeySummary;
  plaintext: string;
}

export interface DeciderState {
  event: { id: string; name: string; date: string; location: string | null };
  discipline: { id: string; name: string; mode: TimerMode } | null;
  slot: number;
  current_run: {
    run: Run;
    team: Team;
    runner: Runner | null;
  } | null;
  disabled: boolean;
}

/** Discipline as embedded in the station state. */
export interface StationDiscipline {
  id: string;
  name: string;
  mode: TimerMode;
  lane_count: number;
  judge_mode: JudgeMode;
}

/** One current run as seen by a judge station. */
export interface StationCurrentRun {
  run: Run;
  team: Team;
  /** Team roster (for the lineup tick list in PR 2). */
  runners: Runner[];
  /** Legacy single-runner field; `null` with the 2027 backend. */
  runner?: Runner | null;
}

/** Response of the station-token state endpoint. */
export interface StationDeciderState {
  event: { id: string; name: string; date: string; location: string | null };
  discipline: StationDiscipline | null;
  /** One per current run, ordered by lane. */
  current_runs: StationCurrentRun[];
  /** Kept = `current_runs[0]` for old clients. */
  current_run: StationCurrentRun | null;
  disabled: boolean;
  allowed_stations: StationKind[];
  /** PR 2: penalties of the current runs (of `?station=` when given).
   *  `undefined` when the backend doesn't send the field. */
  penalties?: Penalty[];
}

export interface BulkApproveResponse {
  approved: number;
  rejected: number;
  dq: number;
  /** The run's new `penalties_accepted_at` (server clock). Missing on older backends. */
  penalties_accepted_at?: string;
}

export interface ImportResultsResponse {
  runs_imported: number;
  penalties_imported: number;
  runs_skipped: number;
  penalties_skipped: number;
}

export type SignInResponse =
  | { result: 'authenticated'; user: AuthenticatedUser }
  | { result: 'new_password_required'; session: string; email: string };

// ===== request bodies =====

export interface DisciplineInput {
  name: string;
  mode: TimerMode;
  position: number;
  lane_count?: number;
  judge_mode?: JudgeMode;
}

export interface RunnerInput {
  first_name: string;
  last_name: string;
  birth_date: string;
  position: number;
}

export interface TeamEntryInput {
  team_id: string;
  start_number: number;
  lane: number | null;
  /** Required since 2027. */
  wg: Wertungsgruppe;
}

export interface MoveWgInput {
  wg: Wertungsgruppe;
  note?: string;
}

export type SetCurrentInput =
  | { discipline_id: string | null; run_id: string | null }
  | { discipline_id: string | null; run_ids: string[] };

export type HandTimesInput =
  | { hand_time_1_ms: number; hand_time_2_ms: number }
  | { clear: true };

export interface StationPenaltyInput {
  station: StationKind;
  /** Optional only when exactly one run is current. */
  run_id?: string;
  /** Catalogue code (2027). `kind`/`delta_ms` are derived server-side. */
  code?: string;
  client_id?: string;
  /** Legacy (pre-2027 backend) — only sent when no catalogue is available. */
  delta_ms?: number | null;
  /** Legacy; ignored on input by the 2027 backend. */
  is_disqualification?: boolean;
  reason_code?: string | null;
}

// Auth endpoints opt out of the global 401-redirect: a 401 here means
// "bad credentials" or "no session yet", which is a legitimate response
// the LoginForm wants to display, not a trigger to redirect.
export const apiClient = {
  auth: {
    me: () => request<{ user: AuthenticatedUser }>('/api/auth/me', { allowUnauthorized: true }),
    signIn: (input: { email: string; password: string }) =>
      request<SignInResponse>('/api/auth/sign-in', {
        method: 'POST',
        body: JSON.stringify(input),
        allowUnauthorized: true,
      }),
    completeNewPassword: (input: {
      email: string;
      new_password: string;
      session: string;
    }) =>
      request<SignInResponse>('/api/auth/complete-new-password', {
        method: 'POST',
        body: JSON.stringify(input),
        allowUnauthorized: true,
      }),
    forgotPassword: (input: { email: string }) =>
      request<void>('/api/auth/forgot-password', {
        method: 'POST',
        body: JSON.stringify(input),
        allowUnauthorized: true,
      }),
    confirmForgotPassword: (input: {
      email: string;
      code: string;
      new_password: string;
    }) =>
      request<void>('/api/auth/confirm-forgot-password', {
        method: 'POST',
        body: JSON.stringify(input),
        allowUnauthorized: true,
      }),
  },
  events: {
    list: async () => (await request<unknown[]>('/api/events')).map(normalizeEvent),
    create: async (input: { name: string; date: string; location?: string | null }) =>
      normalizeEvent(
        await request<unknown>('/api/events', {
          method: 'POST',
          body: JSON.stringify(input),
        }),
      ),
    detail: async (id: string) =>
      normalizeEventDetail(await request<unknown>(`/api/events/${id}`)),
    update: async (
      id: string,
      input: Partial<{ name: string; date: string; location: string | null }>,
    ) =>
      normalizeEvent(
        await request<unknown>(`/api/events/${id}`, {
          method: 'PUT',
          body: JSON.stringify(input),
        }),
      ),
    remove: (id: string) => request<void>(`/api/events/${id}`, { method: 'DELETE' }),
    activate: async (id: string) =>
      normalizeEvent(await request<unknown>(`/api/events/${id}/activate`, { method: 'POST' })),
    /** Legacy `{ discipline_id, run_id }` or parallel `{ discipline_id, run_ids }`. */
    setCurrent: async (id: string, body: SetCurrentInput) =>
      normalizeEvent(
        await request<unknown>(`/api/events/${id}/current`, {
          method: 'POST',
          body: JSON.stringify(body),
        }),
      ),
    rotateDeciderTokens: async (id: string) =>
      normalizeEvent(
        await request<unknown>(`/api/events/${id}/decider-tokens/rotate`, { method: 'POST' }),
      ),
  },
  disciplines: {
    create: async (event_id: string, input: DisciplineInput) =>
      normalizeDiscipline(
        await request<unknown>(`/api/events/${event_id}/disciplines`, {
          method: 'POST',
          body: JSON.stringify(input),
        }),
      ) as DisciplineSaveResponse,
    update: async (event_id: string, discipline_id: string, input: DisciplineInput) =>
      normalizeDiscipline(
        await request<unknown>(`/api/events/${event_id}/disciplines/${discipline_id}`, {
          method: 'PUT',
          body: JSON.stringify(input),
        }),
      ) as DisciplineSaveResponse,
    remove: (event_id: string, discipline_id: string) =>
      request<void>(`/api/events/${event_id}/disciplines/${discipline_id}`, {
        method: 'DELETE',
      }),
    setSyncMode: async (event_id: string, discipline_id: string, sync_mode: SyncMode) =>
      normalizeDiscipline(
        await request<unknown>(
          `/api/events/${event_id}/disciplines/${discipline_id}/sync-mode`,
          { method: 'POST', body: JSON.stringify({ sync_mode }) },
        ),
      ),
    exportFile: async (
      event_id: string,
      discipline_id: string,
    ): Promise<{ blob: Blob; filename: string }> => {
      const res = await fetch(
        `${getApiBase()}/api/events/${event_id}/disciplines/${discipline_id}/export-file`,
        { credentials: 'include' },
      );
      if (!res.ok) {
        throw await parseError(res);
      }
      const disposition = res.headers.get('Content-Disposition') ?? '';
      const match = disposition.match(/filename="?([^"]+)"?/i);
      const filename = match?.[1] ?? `kido-${discipline_id}.kido`;
      return { blob: await res.blob(), filename };
    },
    importResults: (event_id: string, discipline_id: string, envelope: unknown) =>
      request<ImportResultsResponse>(
        `/api/events/${event_id}/disciplines/${discipline_id}/import-results`,
        { method: 'POST', body: JSON.stringify(envelope) },
      ),
  },
  teams: {
    create: (event_id: string, input: { name: string; position: number }) =>
      request<Team>(`/api/events/${event_id}/teams`, {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    update: (event_id: string, team_id: string, input: { name: string; position: number }) =>
      request<Team>(`/api/events/${event_id}/teams/${team_id}`, {
        method: 'PUT',
        body: JSON.stringify(input),
      }),
    remove: (event_id: string, team_id: string) =>
      request<void>(`/api/events/${event_id}/teams/${team_id}`, { method: 'DELETE' }),
  },
  runners: {
    // `gender` is no longer sent (2027): the body is built field by field
    // so a stray `gender` on the caller's object never reaches the wire.
    create: async (event_id: string, team_id: string, input: RunnerInput) =>
      normalizeRunner(
        await request<unknown>(`/api/events/${event_id}/teams/${team_id}/runners`, {
          method: 'POST',
          body: JSON.stringify(runnerBody(input)),
        }),
      ),
    update: async (event_id: string, team_id: string, runner_id: string, input: RunnerInput) =>
      normalizeRunner(
        await request<unknown>(
          `/api/events/${event_id}/teams/${team_id}/runners/${runner_id}`,
          { method: 'PUT', body: JSON.stringify(runnerBody(input)) },
        ),
      ),
    remove: (event_id: string, team_id: string, runner_id: string) =>
      request<void>(
        `/api/events/${event_id}/teams/${team_id}/runners/${runner_id}`,
        { method: 'DELETE' },
      ),
  },
  teamEntries: {
    upsert: async (event_id: string, discipline_id: string, input: TeamEntryInput) =>
      normalizeTeamEntry(
        await request<unknown>(
          `/api/events/${event_id}/disciplines/${discipline_id}/entries`,
          {
            method: 'POST',
            body: JSON.stringify({
              team_id: input.team_id,
              start_number: input.start_number,
              lane: input.lane,
              wg: input.wg,
            }),
          },
        ),
      ),
    remove: (event_id: string, discipline_id: string, team_id: string) =>
      request<void>(
        `/api/events/${event_id}/disciplines/${discipline_id}/entries/${team_id}`,
        { method: 'DELETE' },
      ),
    /** "Umsetzen": sets `wg` and appends to `wg_history`. Organizer only. */
    moveWg: async (
      event_id: string,
      discipline_id: string,
      team_id: string,
      input: MoveWgInput,
    ) => {
      const note = input.note?.trim();
      return normalizeTeamEntry(
        await request<unknown>(
          `/api/events/${event_id}/disciplines/${discipline_id}/entries/${team_id}/move-wg`,
          {
            method: 'POST',
            body: JSON.stringify(note ? { wg: input.wg, note } : { wg: input.wg }),
          },
        ),
      );
    },
  },
  runs: {
    list: async (event_id: string, discipline_id: string) =>
      (
        await request<unknown[]>(`/api/events/${event_id}/disciplines/${discipline_id}/runs`)
      ).map(normalizeRun),
    create: async (
      event_id: string,
      discipline_id: string,
      input: {
        team_id: string;
        runner_id: string | null;
        run_number: number;
        lane: number | null;
      },
    ) =>
      normalizeRun(
        await request<unknown>(`/api/events/${event_id}/disciplines/${discipline_id}/runs`, {
          method: 'POST',
          body: JSON.stringify(input),
        }),
      ),
    update: async (
      event_id: string,
      discipline_id: string,
      run_number: number,
      run_id: string,
      input: Partial<{
        status: RunStatus;
        original_time_ms: number | null;
        started_at: string | null;
        ended_at: string | null;
      }>,
    ) =>
      normalizeRun(
        await request<unknown>(
          `/api/events/${event_id}/disciplines/${discipline_id}/runs/${run_number}/${run_id}`,
          { method: 'PUT', body: JSON.stringify(input) },
        ),
      ),
    /** Organizer lineup correction; empty list clears. */
    setLineup: async (
      event_id: string,
      discipline_id: string,
      run_id: string,
      runner_ids: string[],
    ) =>
      normalizeRun(
        await request<unknown>(
          `/api/events/${event_id}/disciplines/${discipline_id}/runs/${run_id}/lineup`,
          { method: 'PUT', body: JSON.stringify({ runner_ids }) },
        ),
      ),
    /** Hand times → `time_source = handstopp`; `{ clear: true }` reverts. */
    setHandTimes: async (
      event_id: string,
      discipline_id: string,
      run_id: string,
      input: HandTimesInput,
    ) =>
      normalizeRun(
        await request<unknown>(
          `/api/events/${event_id}/disciplines/${discipline_id}/runs/${run_id}/hand-times`,
          { method: 'PUT', body: JSON.stringify(input) },
        ),
      ),
    clearHandTimes: (event_id: string, discipline_id: string, run_id: string) =>
      apiClient.runs.setHandTimes(event_id, discipline_id, run_id, { clear: true }),
  },
  penalties: {
    list: async (
      event_id: string,
      opts?: { status?: PenaltyStatus; discipline_id?: string },
    ) => {
      const params = new URLSearchParams();
      if (opts?.status) params.set('status', opts.status);
      if (opts?.discipline_id) params.set('discipline_id', opts.discipline_id);
      const q = params.toString() ? `?${params}` : '';
      return (await request<unknown[]>(`/api/events/${event_id}/penalties${q}`)).map(
        normalizePenalty,
      );
    },
    approve: async (event_id: string, penalty_id: string) =>
      normalizePenalty(
        await request<unknown>(
          `/api/events/${event_id}/penalties/${penalty_id}/approve`,
          { method: 'POST' },
        ),
      ),
    reject: async (event_id: string, penalty_id: string) =>
      normalizePenalty(
        await request<unknown>(
          `/api/events/${event_id}/penalties/${penalty_id}/reject`,
          { method: 'POST' },
        ),
      ),
    /** "Rückgängig": decided → pending (409 once the run was taken over). */
    undo: async (event_id: string, penalty_id: string) =>
      normalizePenalty(
        await request<unknown>(
          `/api/events/${event_id}/penalties/${penalty_id}/undo`,
          { method: 'POST' },
        ),
      ),
    bulkApprove: (event_id: string, run_id: string) =>
      request<BulkApproveResponse>(
        `/api/events/${event_id}/runs/${run_id}/penalties/bulk-approve`,
        { method: 'POST' },
      ),
  },
  results: {
    /** WG rankings + Tagessieger of one discipline (docs/api.md § Results). */
    get: async (event_id: string, discipline_id: string) =>
      normalizeResults(
        await request<unknown>(
          `/api/events/${event_id}/disciplines/${discipline_id}/results`,
        ),
      ),
  },
  catalogue: {
    /** Static, public error catalogue. */
    list: async () =>
      normalizeCatalogue(
        await request<unknown>('/api/catalogue', { allowUnauthorized: true }),
      ),
  },
  apiKeys: {
    list: () => request<ApiKeySummary[]>('/api/users/me/api-keys'),
    create: (input: { name?: string }) =>
      request<ApiKeyCreatedResponse>('/api/users/me/api-keys', {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    revoke: (key_id: string) =>
      request<void>(`/api/users/me/api-keys/${key_id}`, { method: 'DELETE' }),
  },
  decider: {
    state: (owner_sub: string, slot: number, token: string) =>
      request<DeciderState>(`/api/decider/${owner_sub}/${slot}/${token}/state`, JUDGE),
    submit: (
      owner_sub: string,
      slot: number,
      token: string,
      input: {
        delta_ms: number;
        reason_code?: string | null;
        client_id?: string;
      },
    ) =>
      request<Penalty>(`/api/decider/${owner_sub}/${slot}/${token}/penalties`, {
        ...JUDGE,
        method: 'POST',
        body: JSON.stringify(input),
      }),
    withdraw: (owner_sub: string, slot: number, token: string, penalty_id: string) =>
      request<void>(
        `/api/decider/${owner_sub}/${slot}/${token}/penalties/${penalty_id}`,
        { ...JUDGE, method: 'DELETE' },
      ),
  },
  // Station-token flow. Same shape as the slot flow, minus the slot
  // path segment — the station travels in the body instead.
  deciderStation: {
    /** `station` (optional) restricts `penalties` to the judge's station. */
    state: async (owner_sub: string, token: string, station?: StationKind | null) =>
      normalizeStationState(
        await request<unknown>(
          `/api/decider/${owner_sub}/${token}/state${
            station ? `?station=${encodeURIComponent(station)}` : ''
          }`,
          JUDGE,
        ),
      ),
    submit: async (owner_sub: string, token: string, input: StationPenaltyInput) =>
      normalizePenalty(
        await request<unknown>(`/api/decider/${owner_sub}/${token}/penalties`, {
          ...JUDGE,
          method: 'POST',
          body: JSON.stringify(stationPenaltyBody(input)),
        }),
      ),
    withdraw: (
      owner_sub: string,
      token: string,
      penalty_id: string,
      station: StationKind | null,
    ) => {
      const q = station ? `?station=${encodeURIComponent(station)}` : '';
      return request<void>(
        `/api/decider/${owner_sub}/${token}/penalties/${penalty_id}${q}`,
        { ...JUDGE, method: 'DELETE' },
      );
    },
    /** Startlinie only; run must be one of `current_run_ids`. */
    setLineup: async (
      owner_sub: string,
      token: string,
      run_id: string,
      runner_ids: string[],
      station?: StationKind,
    ) =>
      normalizeRun(
        await request<unknown>(`/api/decider/${owner_sub}/${token}/runs/${run_id}/lineup`, {
          ...JUDGE,
          method: 'PUT',
          body: JSON.stringify(station ? { runner_ids, station } : { runner_ids }),
        }),
      ),
  },
};

/** Convenience alias for `apiClient.catalogue.list()`. */
export function getCatalogue(): Promise<CatalogueEntry[]> {
  return apiClient.catalogue.list();
}

function runnerBody(input: RunnerInput): RunnerInput {
  return {
    first_name: input.first_name,
    last_name: input.last_name,
    birth_date: input.birth_date,
    position: input.position,
  };
}

/** Catalogue submissions send `code` (+ `run_id`); the legacy numeric
 *  fields are only included when no `code` is given. */
export function stationPenaltyBody(input: StationPenaltyInput): Record<string, unknown> {
  const body: Record<string, unknown> = { station: input.station };
  if (input.run_id) body.run_id = input.run_id;
  if (input.client_id) body.client_id = input.client_id;
  if (input.code) {
    body.code = input.code;
    return body;
  }
  if (input.delta_ms != null) body.delta_ms = input.delta_ms;
  if (input.is_disqualification) body.is_disqualification = true;
  if (input.reason_code !== undefined) body.reason_code = input.reason_code;
  return body;
}

export const TIMER_MODE_LABELS: Record<TimerMode, string> = {
  gruppenstaffette: 'Gruppenstaffette',
  loeschangriff: 'Löschangriff',
};

/** Maximum number of lanes per mode. Per-discipline value: `Discipline.lane_count`. */
export const TIMER_MODE_LANE_COUNT: Record<TimerMode, number> = {
  gruppenstaffette: 2,
  loeschangriff: 4,
};

export const RUN_STATUS_LABELS: Record<RunStatus, string> = {
  pending: 'Vorgemerkt',
  active: 'Aktiv',
  completed: 'Beendet',
  cancelled: 'Abgebrochen',
};

export const PENALTY_STATUS_LABELS: Record<PenaltyStatus, string> = {
  pending: 'Ausstehend',
  approved: 'Genehmigt',
  rejected: 'Abgelehnt',
};

export const TIME_SOURCE_LABELS: Record<TimeSource, string> = {
  lichtschranke: 'Lichtschranke',
  handstopp: 'Handzeit',
};

export { STATION_KIND_LABELS } from './stations';
export { JUDGE_MODE_LABELS } from './lanes';
export { WERTUNGSGRUPPE_DESCRIPTIONS, WERTUNGSGRUPPE_LABELS } from './wg';
export { PENALTY_KIND_LABELS } from './penalties';
