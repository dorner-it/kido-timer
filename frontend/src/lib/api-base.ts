// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Single source of the API base URL.
 *
 * Default: the build-time `PUBLIC_API_BASE_URL` (empty string = same
 * origin), exactly as before.
 *
 * Runtime override: set `window.__KIDO_API_BASE__` **before** the app
 * bundle runs, e.g. from an inline script in the served HTML:
 *
 * ```html
 * <script>window.__KIDO_API_BASE__ = 'http://192.168.1.20:8080';</script>
 * ```
 *
 * The on-prem mode (desktop serves the judge UI over plain HTTP in the
 * venue LAN) uses this to point the same static build at the laptop.
 * An empty string override means "same origin". The value is read on
 * every call (never cached at module import), so it may also be set
 * after load.
 */

declare global {
  interface Window {
    /** Runtime API base override, see `getApiBase()`. */
    __KIDO_API_BASE__?: string;
  }
}

/** Removes trailing slashes so `${base}/api/...` never doubles them. */
export function normalizeApiBase(base: string): string {
  return base.trim().replace(/\/+$/, '');
}

/**
 * Pure resolver: a string `override` wins (even an empty one = same
 * origin); anything else falls back to `envValue` (or `''`).
 */
export function resolveApiBase(override: unknown, envValue: unknown): string {
  if (typeof override === 'string') return normalizeApiBase(override);
  if (typeof envValue === 'string') return normalizeApiBase(envValue);
  return '';
}

function envApiBase(): string | undefined {
  try {
    return import.meta.env.PUBLIC_API_BASE_URL as string | undefined;
  } catch {
    return undefined;
  }
}

function windowOverride(): unknown {
  try {
    return typeof window === 'undefined' ? undefined : window.__KIDO_API_BASE__;
  } catch {
    return undefined;
  }
}

/** Current API base (runtime override → `PUBLIC_API_BASE_URL` → `''`). */
export function getApiBase(): string {
  return resolveApiBase(windowOverride(), envApiBase());
}
