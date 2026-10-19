// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Built-in QR scanner of the judge "Zugang ungültig" screen: which
 * scanned codes may be opened, whether the camera can be used here, and
 * German error texts. Pure functions (no DOM access) so they are
 * testable in the node test environment.
 *
 * Accepted judge URL formats (see `pages/decider/…`, `judge-spa-params.ts`
 * and the desktop on-prem server `desktop/src-tauri/src/local/server/`):
 *
 * - `/decider/<owner_sub>/<token>`          — station token (2027 app)
 * - `/decider/<owner_sub>/<slot 1-4>/<token>` — legacy slot QR codes
 * - `/judge/?t=<token>[&o=<owner_sub>]`     — static on-prem build
 *   (`/judge?t=…` redirects there on the desktop server)
 *
 * Only for the origin the page is currently served from, or the cloud
 * app `https://time.ki-do.it`.
 */

/** Production origin of the cloud judge app. */
export const CLOUD_JUDGE_ORIGIN = 'https://time.ki-do.it';

export const QR_INVALID_HINT = 'Kein gültiger Strafrichter-Code';
export const QR_SAME_LINK_HINT =
  'Das ist der bisherige, ungültige Code – bitte einen neuen QR-Code holen.';

export const CAMERA_UNAVAILABLE_TEXT =
  'Kamera hier nicht verfügbar – bitte den QR-Code mit der Kamera-App des Handys scannen.';
export const CAMERA_DENIED_TEXT =
  'Kamerazugriff wurde nicht erlaubt – bitte den QR-Code mit der Kamera-App des Handys scannen oder den Zugriff in den Browser-Einstellungen erlauben.';
export const CAMERA_MISSING_TEXT =
  'Keine Kamera gefunden – bitte den QR-Code mit der Kamera-App des Handys scannen.';
export const CAMERA_BUSY_TEXT =
  'Kamera ist gerade belegt – andere Apps schließen und erneut versuchen, oder den QR-Code mit der Kamera-App des Handys scannen.';
export const CAMERA_FAILED_TEXT =
  'Kamera konnte nicht gestartet werden – bitte den QR-Code mit der Kamera-App des Handys scannen.';

/** Same character set as `judge-spa-params.ts` (one URL path segment). */
const SEG = '[A-Za-z0-9._~:@-]+';
const STATION_PATH = new RegExp(`^/decider/${SEG}/${SEG}/?$`);
const SLOT_PATH = new RegExp(`^/decider/${SEG}/[1-4]/${SEG}/?$`);
const ONPREM_PATH = /^\/judge\/?$/;
const SAFE_VALUE = new RegExp(`^${SEG}$`);
const MAX_LEN = 2048;

export type JudgeQrResult = { ok: true; url: string } | { ok: false; message: string };

function originOf(value: string | null | undefined): string | null {
  if (!value) return null;
  try {
    const u = new URL(value);
    return u.protocol === 'http:' || u.protocol === 'https:' ? u.origin : null;
  } catch {
    return null;
  }
}

/** `true` if `url` (already parsed) has one of the judge paths above. */
export function isJudgePath(url: URL): boolean {
  const p = url.pathname;
  if (STATION_PATH.test(p) || SLOT_PATH.test(p)) return true;
  if (ONPREM_PATH.test(p)) {
    const t = url.searchParams.get('t');
    const o = url.searchParams.get('o');
    return !!t && SAFE_VALUE.test(t) && (o === null || SAFE_VALUE.test(o));
  }
  return false;
}

/**
 * Validates scanned QR content. Accepts only absolute http(s) URLs whose
 * origin is `currentOrigin` or {@link CLOUD_JUDGE_ORIGIN}, without user
 * info, with a judge path. `currentHref` (the page that just turned out
 * invalid) is rejected with its own hint, so the judge does not loop.
 */
export function validateJudgeQr(
  raw: unknown,
  currentOrigin: string,
  currentHref?: string,
): JudgeQrResult {
  const invalid: JudgeQrResult = { ok: false, message: QR_INVALID_HINT };
  if (typeof raw !== 'string') return invalid;
  const text = raw.trim();
  if (text === '' || text.length > MAX_LEN) return invalid;

  let url: URL;
  try {
    url = new URL(text);
  } catch {
    return invalid;
  }
  if (url.protocol !== 'https:' && url.protocol !== 'http:') return invalid;
  if (url.username !== '' || url.password !== '') return invalid;

  const allowed = [originOf(currentOrigin), CLOUD_JUDGE_ORIGIN].filter(
    (o): o is string => o !== null,
  );
  if (!allowed.includes(url.origin)) return invalid;
  if (!isJudgePath(url)) return invalid;

  if (currentHref) {
    try {
      if (new URL(currentHref).href === url.href) {
        return { ok: false, message: QR_SAME_LINK_HINT };
      }
    } catch {
      /* ignore an unparsable current href */
    }
  }
  return { ok: true, url: url.href };
}

export type ScannerAvailability = { ok: true } | { ok: false; message: string };

/** What the browser exposes; `undefined` fields = not available. */
export interface ScannerEnv {
  isSecureContext?: boolean;
  mediaDevices?: { getUserMedia?: unknown } | null;
}

/**
 * Camera scanning needs a secure context (https or localhost) and
 * `navigator.mediaDevices.getUserMedia`. On the desktop on-prem server
 * (plain `http://<LAN-IP>`) neither is there → fallback text.
 */
export function scannerAvailability(env: ScannerEnv): ScannerAvailability {
  if (env.isSecureContext === false) return { ok: false, message: CAMERA_UNAVAILABLE_TEXT };
  if (!env.mediaDevices || typeof env.mediaDevices.getUserMedia !== 'function') {
    return { ok: false, message: CAMERA_UNAVAILABLE_TEXT };
  }
  return { ok: true };
}

/** Browser environment for {@link scannerAvailability}; safe in SSR. */
export function browserScannerEnv(): ScannerEnv {
  if (typeof window === 'undefined' || typeof navigator === 'undefined') return {};
  return { isSecureContext: window.isSecureContext, mediaDevices: navigator.mediaDevices };
}

/** German text for a `getUserMedia` / `video.play()` failure. */
export function cameraErrorMessage(err: unknown): string {
  const name =
    typeof err === 'object' && err !== null && 'name' in err
      ? String((err as { name: unknown }).name)
      : '';
  switch (name) {
    case 'NotAllowedError':
    case 'PermissionDeniedError':
      return CAMERA_DENIED_TEXT;
    case 'SecurityError':
      return CAMERA_UNAVAILABLE_TEXT;
    case 'NotFoundError':
    case 'DevicesNotFoundError':
    case 'OverconstrainedError':
      return CAMERA_MISSING_TEXT;
    case 'NotReadableError':
    case 'TrackStartError':
    case 'AbortError':
      return CAMERA_BUSY_TEXT;
    default:
      return CAMERA_FAILED_TEXT;
  }
}

/** Stops every track of `stream` (camera light off). Null-safe. */
export function stopMediaStream(
  stream: { getTracks: () => readonly { stop: () => void }[] } | null | undefined,
): void {
  if (!stream) return;
  for (const t of stream.getTracks()) {
    try {
      t.stop();
    } catch {
      /* already stopped */
    }
  }
}

/** Target size of a decode frame: longest side at most `max` px. */
export function frameSize(
  width: number,
  height: number,
  max = 640,
): { width: number; height: number } {
  if (!(width > 0) || !(height > 0)) return { width: 0, height: 0 };
  const scale = Math.min(1, max / Math.max(width, height));
  return { width: Math.round(width * scale), height: Math.round(height * scale) };
}
