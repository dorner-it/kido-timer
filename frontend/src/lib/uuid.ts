// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * UUID v4 that also works outside secure contexts.
 *
 * `crypto.randomUUID` only exists on HTTPS / localhost. The on-prem
 * judge UI is served over plain HTTP in a LAN, so we fall back to
 * `crypto.getRandomValues` (available in insecure contexts) and, as a
 * last resort (very old browsers), to `Math.random`.
 */

export interface CryptoLike {
  randomUUID?: () => string;
  getRandomValues?: <T extends Uint8Array>(array: T) => T;
}

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export function isUuidV4(value: unknown): value is string {
  return typeof value === 'string' && UUID_RE.test(value);
}

/** Formats 16 random bytes as RFC 4122 v4 (sets version + variant bits). */
export function bytesToUuidV4(bytes: Uint8Array): string {
  if (bytes.length < 16) throw new Error('need 16 bytes');
  const b = Array.from(bytes.slice(0, 16));
  b[6] = (b[6]! & 0x0f) | 0x40;
  b[8] = (b[8]! & 0x3f) | 0x80;
  const hex = b.map((x) => x.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function defaultCrypto(): CryptoLike | undefined {
  try {
    return typeof crypto === 'undefined' ? undefined : (crypto as CryptoLike);
  } catch {
    return undefined;
  }
}

export function uuidV4(
  c: CryptoLike | null | undefined = defaultCrypto(),
  random: () => number = Math.random,
): string {
  if (c && typeof c.randomUUID === 'function') {
    try {
      const id = c.randomUUID();
      if (isUuidV4(id)) return id;
    } catch {
      // insecure context or broken polyfill → fall through
    }
  }
  const bytes = new Uint8Array(16);
  if (c && typeof c.getRandomValues === 'function') {
    try {
      c.getRandomValues(bytes);
      return bytesToUuidV4(bytes);
    } catch {
      // fall through
    }
  }
  for (let i = 0; i < 16; i++) bytes[i] = Math.floor(random() * 256) & 0xff;
  return bytesToUuidV4(bytes);
}
