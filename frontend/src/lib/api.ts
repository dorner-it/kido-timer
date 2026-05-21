// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

const API_BASE_URL =
  (import.meta.env.PUBLIC_API_BASE_URL as string | undefined) ?? '';

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
  const res = await fetch(`${API_BASE_URL}${path}`, {
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

// ===== domain types — mirror the backend (schema v2) =====

export type TimerMode = 'single_lane' | 'two_lane_parallel' | 'relay' | 'individual';
export type SyncMode = 'live' | 'offline';
export type RunStatus = 'pending' | 'active' | 'completed' | 'cancelled';
export type PenaltyStatus = 'pending' | 'approved' | 'rejected';

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
  current_run_id: string | null;
  decider_tokens: [string, string, string, string];
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
  created_at: string;
  updated_at: string;
}

export interface Team {
  id: string;
  event_id: string;
  name: string;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface TeamEntry {
  team_id: string;
  discipline_id: string;
  event_id: string;
  start_number: number;
  lane: number | null;
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
  decider_slot: number;
  delta_ms: number;
  reason_code: string | null;
  status: PenaltyStatus;
  created_at: string;
  reviewed_at: string | null;
  reviewed_by: string | null;
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

export interface ImportResultsResponse {
  runs_imported: number;
  penalties_imported: number;
  runs_skipped: number;
  penalties_skipped: number;
}

export type SignInResponse =
  | { result: 'authenticated'; user: AuthenticatedUser }
  | { result: 'new_password_required'; session: string; email: string };

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
    list: () => request<Event[]>('/api/events'),
    create: (input: { name: string; date: string; location?: string | null }) =>
      request<Event>('/api/events', {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    detail: (id: string) => request<EventDetail>(`/api/events/${id}`),
    update: (
      id: string,
      input: Partial<{ name: string; date: string; location: string | null }>,
    ) =>
      request<Event>(`/api/events/${id}`, {
        method: 'PUT',
        body: JSON.stringify(input),
      }),
    remove: (id: string) => request<void>(`/api/events/${id}`, { method: 'DELETE' }),
    activate: (id: string) =>
      request<Event>(`/api/events/${id}/activate`, { method: 'POST' }),
    setCurrent: (id: string, body: { discipline_id: string | null; run_id: string | null }) =>
      request<Event>(`/api/events/${id}/current`, {
        method: 'POST',
        body: JSON.stringify(body),
      }),
    rotateDeciderTokens: (id: string) =>
      request<Event>(`/api/events/${id}/decider-tokens/rotate`, { method: 'POST' }),
  },
  disciplines: {
    create: (event_id: string, input: { name: string; mode: TimerMode; position: number }) =>
      request<Discipline>(`/api/events/${event_id}/disciplines`, {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    update: (
      event_id: string,
      discipline_id: string,
      input: { name: string; mode: TimerMode; position: number },
    ) =>
      request<Discipline>(`/api/events/${event_id}/disciplines/${discipline_id}`, {
        method: 'PUT',
        body: JSON.stringify(input),
      }),
    remove: (event_id: string, discipline_id: string) =>
      request<void>(`/api/events/${event_id}/disciplines/${discipline_id}`, {
        method: 'DELETE',
      }),
    setSyncMode: (event_id: string, discipline_id: string, sync_mode: SyncMode) =>
      request<Discipline>(
        `/api/events/${event_id}/disciplines/${discipline_id}/sync-mode`,
        { method: 'POST', body: JSON.stringify({ sync_mode }) },
      ),
    exportFile: async (
      event_id: string,
      discipline_id: string,
    ): Promise<{ blob: Blob; filename: string }> => {
      const res = await fetch(
        `${API_BASE_URL}/api/events/${event_id}/disciplines/${discipline_id}/export-file`,
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
    create: (
      event_id: string,
      team_id: string,
      input: {
        first_name: string;
        last_name: string;
        birth_date: string;
        position: number;
      },
    ) =>
      request<Runner>(`/api/events/${event_id}/teams/${team_id}/runners`, {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    update: (
      event_id: string,
      team_id: string,
      runner_id: string,
      input: {
        first_name: string;
        last_name: string;
        birth_date: string;
        position: number;
      },
    ) =>
      request<Runner>(
        `/api/events/${event_id}/teams/${team_id}/runners/${runner_id}`,
        { method: 'PUT', body: JSON.stringify(input) },
      ),
    remove: (event_id: string, team_id: string, runner_id: string) =>
      request<void>(
        `/api/events/${event_id}/teams/${team_id}/runners/${runner_id}`,
        { method: 'DELETE' },
      ),
  },
  teamEntries: {
    upsert: (
      event_id: string,
      discipline_id: string,
      input: { team_id: string; start_number: number; lane: number | null },
    ) =>
      request<TeamEntry>(
        `/api/events/${event_id}/disciplines/${discipline_id}/entries`,
        { method: 'POST', body: JSON.stringify(input) },
      ),
    remove: (event_id: string, discipline_id: string, team_id: string) =>
      request<void>(
        `/api/events/${event_id}/disciplines/${discipline_id}/entries/${team_id}`,
        { method: 'DELETE' },
      ),
  },
  runs: {
    list: (event_id: string, discipline_id: string) =>
      request<Run[]>(`/api/events/${event_id}/disciplines/${discipline_id}/runs`),
    create: (
      event_id: string,
      discipline_id: string,
      input: {
        team_id: string;
        runner_id: string | null;
        run_number: number;
        lane: number | null;
      },
    ) =>
      request<Run>(`/api/events/${event_id}/disciplines/${discipline_id}/runs`, {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    update: (
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
      request<Run>(
        `/api/events/${event_id}/disciplines/${discipline_id}/runs/${run_number}/${run_id}`,
        { method: 'PUT', body: JSON.stringify(input) },
      ),
  },
  penalties: {
    list: (event_id: string, opts?: { status?: PenaltyStatus; discipline_id?: string }) => {
      const params = new URLSearchParams();
      if (opts?.status) params.set('status', opts.status);
      if (opts?.discipline_id) params.set('discipline_id', opts.discipline_id);
      const q = params.toString() ? `?${params}` : '';
      return request<Penalty[]>(`/api/events/${event_id}/penalties${q}`);
    },
    approve: (event_id: string, penalty_id: string) =>
      request<Penalty>(
        `/api/events/${event_id}/penalties/${penalty_id}/approve`,
        { method: 'POST' },
      ),
    reject: (event_id: string, penalty_id: string) =>
      request<Penalty>(
        `/api/events/${event_id}/penalties/${penalty_id}/reject`,
        { method: 'POST' },
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
      request<DeciderState>(`/api/decider/${owner_sub}/${slot}/${token}/state`),
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
        method: 'POST',
        body: JSON.stringify(input),
      }),
    withdraw: (owner_sub: string, slot: number, token: string, penalty_id: string) =>
      request<void>(
        `/api/decider/${owner_sub}/${slot}/${token}/penalties/${penalty_id}`,
        { method: 'DELETE' },
      ),
  },
};

export const TIMER_MODE_LABELS: Record<TimerMode, string> = {
  single_lane: 'Einzelbahn',
  two_lane_parallel: 'Zwei Bahnen parallel',
  relay: 'Staffel',
  individual: 'Einzellauf',
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
