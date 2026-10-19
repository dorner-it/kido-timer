// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * URL parameters of the static on-prem judge build (`npm run build:judge`,
 * served by the desktop under `/judge/`).
 *
 * QR code: `http://<laptop-ip>:<port>/judge/?t=<token>&o=<owner_sub>`.
 * `o` is optional — the desktop also injects `window.__KIDO_OWNER_SUB__`
 * into the served `index.html`.
 */

declare global {
  interface Window {
    /** Owner sub injected by the desktop (on-prem), see `resolveJudgeParams`. */
    __KIDO_OWNER_SUB__?: string;
  }
}

export interface JudgeParams {
  ownerSub: string;
  token: string;
}

export type JudgeParamsResult =
  | { ok: true; params: JudgeParams }
  | { ok: false; message: string };

const SAFE_SEGMENT = /^[A-Za-z0-9._~:@-]+$/;

function clean(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const v = value.trim();
  if (v === '' || !SAFE_SEGMENT.test(v)) return null;
  return v;
}

/**
 * Pure resolver: token from `t` (required), owner from `o`, else the
 * injected owner. Values are used as URL path segments, so anything with
 * `/`, `?`, `#`, whitespace … is rejected.
 */
export function resolveJudgeParams(search: string, injectedOwner?: unknown): JudgeParamsResult {
  const q = new URLSearchParams(search.startsWith('?') ? search.slice(1) : search);
  const token = clean(q.get('t'));
  if (!token) {
    return {
      ok: false,
      message: 'Kein gültiger Zugangscode. Bitte den QR-Code am Wettkampf-Laptop scannen.',
    };
  }
  const ownerSub = clean(q.get('o')) ?? clean(injectedOwner);
  if (!ownerSub) {
    return {
      ok: false,
      message: 'Unvollständiger Link. Bitte den QR-Code am Wettkampf-Laptop erneut scannen.',
    };
  }
  return { ok: true, params: { ownerSub, token } };
}

/**
 * API base of the static build: same origin unless the serving page set
 * `window.__KIDO_API_BASE__` already. Returns the value to assign.
 */
export function standaloneApiBase(current: unknown): string {
  return typeof current === 'string' ? current : '';
}
