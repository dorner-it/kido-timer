// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

const API_BASE_URL =
  (import.meta.env.PUBLIC_API_BASE_URL as string | undefined) ??
  'http://localhost:9000';

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
    // body is not JSON — fall through
  }
  const code = body.error?.code ?? 'unknown_error';
  const message =
    body.error?.message ?? `Request failed with status ${response.status}`;
  return new ApiError(response.status, code, message);
}

export interface LoginInput {
  email: string;
  password: string;
}

export interface AuthenticatedUser {
  id: string;
  email: string;
  display_name: string;
}

export interface LoginResponse {
  access_token: string;
  refresh_token: string;
  user: AuthenticatedUser;
}

export const apiClient = {
  auth: {
    async login(input: LoginInput): Promise<LoginResponse> {
      const response = await fetch(`${API_BASE_URL}/api/auth/login`, {
        method: 'POST',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(input),
      });
      if (!response.ok) {
        throw await parseError(response);
      }
      return (await response.json()) as LoginResponse;
    },
  },
};
