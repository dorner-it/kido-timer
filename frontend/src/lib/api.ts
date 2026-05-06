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

async function request<T>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const res = await fetch(`${API_BASE_URL}${path}`, {
    credentials: 'include',
    ...init,
    headers: {
      ...(init.body ? { 'Content-Type': 'application/json' } : {}),
      ...(init.headers ?? {}),
    },
  });
  if (!res.ok) {
    throw await parseError(res);
  }
  if (res.status === 204) {
    return undefined as T;
  }
  return (await res.json()) as T;
}

// ===== domain types — mirror the backend =====

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

export interface Competition {
  id: string;
  owner_sub: string;
  name: string;
  date: string;
  location: string | null;
  mode: TimerMode;
  sync_mode: SyncMode;
  is_active: boolean;
  current_run_id: string | null;
  decider_tokens: [string, string, string, string];
  created_at: string;
  updated_at: string;
}

export interface Team {
  id: string;
  competition_id: string;
  name: string;
  start_number: number;
}

export interface Runner {
  id: string;
  team_id: string;
  competition_id: string;
  first_name: string;
  last_name: string;
  birth_date: string;
  position: number;
}

export interface Run {
  id: string;
  competition_id: string;
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
  competition_id: string;
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

export interface CompetitionDetail {
  competition: Competition;
  teams: Team[];
  runners: Runner[];
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
  competition: { id: string; name: string; mode: TimerMode };
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

// ===== client =====

export type SignInResponse =
  | { result: 'authenticated'; user: AuthenticatedUser }
  | { result: 'new_password_required'; session: string; email: string };

export const apiClient = {
  auth: {
    me: () => request<{ user: AuthenticatedUser }>('/api/auth/me'),
    signIn: (input: { email: string; password: string }) =>
      request<SignInResponse>('/api/auth/sign-in', {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    completeNewPassword: (input: {
      email: string;
      new_password: string;
      session: string;
    }) =>
      request<SignInResponse>('/api/auth/complete-new-password', {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    forgotPassword: (input: { email: string }) =>
      request<void>('/api/auth/forgot-password', {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    confirmForgotPassword: (input: {
      email: string;
      code: string;
      new_password: string;
    }) =>
      request<void>('/api/auth/confirm-forgot-password', {
        method: 'POST',
        body: JSON.stringify(input),
      }),
  },
  competitions: {
    list: () => request<Competition[]>('/api/competitions'),
    create: (input: {
      name: string;
      date: string;
      mode: TimerMode;
      location?: string | null;
    }) =>
      request<Competition>('/api/competitions', {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    detail: (id: string) => request<CompetitionDetail>(`/api/competitions/${id}`),
    update: (
      id: string,
      input: Partial<{ name: string; date: string; mode: TimerMode; location: string | null }>,
    ) =>
      request<Competition>(`/api/competitions/${id}`, {
        method: 'PUT',
        body: JSON.stringify(input),
      }),
    remove: (id: string) =>
      request<void>(`/api/competitions/${id}`, { method: 'DELETE' }),
    activate: (id: string) =>
      request<Competition>(`/api/competitions/${id}/activate`, { method: 'POST' }),
    setCurrentRun: (id: string, run_id: string | null) =>
      request<Competition>(`/api/competitions/${id}/current-run`, {
        method: 'POST',
        body: JSON.stringify({ run_id }),
      }),
    setSyncMode: (id: string, sync_mode: SyncMode) =>
      request<Competition>(`/api/competitions/${id}/sync-mode`, {
        method: 'POST',
        body: JSON.stringify({ sync_mode }),
      }),
    rotateDeciderTokens: (id: string) =>
      request<Competition>(`/api/competitions/${id}/decider-tokens/rotate`, {
        method: 'POST',
      }),
    exportFile: async (id: string): Promise<{ blob: Blob; filename: string }> => {
      const res = await fetch(`${API_BASE_URL}/api/competitions/${id}/export-file`, {
        credentials: 'include',
      });
      if (!res.ok) {
        throw await parseError(res);
      }
      const disposition = res.headers.get('Content-Disposition') ?? '';
      const match = disposition.match(/filename="?([^"]+)"?/i);
      const filename = match?.[1] ?? `kido-${id}.kido`;
      return { blob: await res.blob(), filename };
    },
    importResults: (id: string, envelope: unknown) =>
      request<ImportResultsResponse>(`/api/competitions/${id}/import-results`, {
        method: 'POST',
        body: JSON.stringify(envelope),
      }),
  },
  teams: {
    create: (competition_id: string, input: { name: string; start_number: number }) =>
      request<Team>(`/api/competitions/${competition_id}/teams`, {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    update: (
      competition_id: string,
      team_id: string,
      input: { name: string; start_number: number },
    ) =>
      request<Team>(`/api/competitions/${competition_id}/teams/${team_id}`, {
        method: 'PUT',
        body: JSON.stringify(input),
      }),
    remove: (competition_id: string, team_id: string) =>
      request<void>(`/api/competitions/${competition_id}/teams/${team_id}`, {
        method: 'DELETE',
      }),
  },
  runners: {
    create: (
      competition_id: string,
      team_id: string,
      input: {
        first_name: string;
        last_name: string;
        birth_date: string;
        position: number;
      },
    ) =>
      request<Runner>(
        `/api/competitions/${competition_id}/teams/${team_id}/runners`,
        { method: 'POST', body: JSON.stringify(input) },
      ),
    update: (
      competition_id: string,
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
        `/api/competitions/${competition_id}/teams/${team_id}/runners/${runner_id}`,
        { method: 'PUT', body: JSON.stringify(input) },
      ),
    remove: (competition_id: string, team_id: string, runner_id: string) =>
      request<void>(
        `/api/competitions/${competition_id}/teams/${team_id}/runners/${runner_id}`,
        { method: 'DELETE' },
      ),
  },
  runs: {
    list: (competition_id: string) =>
      request<Run[]>(`/api/competitions/${competition_id}/runs`),
    create: (
      competition_id: string,
      input: {
        team_id: string;
        runner_id: string | null;
        run_number: number;
        lane: number | null;
      },
    ) =>
      request<Run>(`/api/competitions/${competition_id}/runs`, {
        method: 'POST',
        body: JSON.stringify(input),
      }),
    update: (
      competition_id: string,
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
        `/api/competitions/${competition_id}/runs/${run_number}/${run_id}`,
        { method: 'PUT', body: JSON.stringify(input) },
      ),
  },
  penalties: {
    list: (competition_id: string, status?: PenaltyStatus) => {
      const q = status ? `?status=${status}` : '';
      return request<Penalty[]>(`/api/competitions/${competition_id}/penalties${q}`);
    },
    approve: (competition_id: string, penalty_id: string) =>
      request<Penalty>(
        `/api/competitions/${competition_id}/penalties/${penalty_id}/approve`,
        { method: 'POST' },
      ),
    reject: (competition_id: string, penalty_id: string) =>
      request<Penalty>(
        `/api/competitions/${competition_id}/penalties/${penalty_id}/reject`,
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
        /** Stable client-generated UUID; backend uses it as the row id so
         *  retries from a flaky tablet are idempotent. */
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
