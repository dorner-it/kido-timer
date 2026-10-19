// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it, vi } from 'vitest';

import { clientId } from '../penalty-queue';
import { bytesToUuidV4, isUuidV4, uuidV4, type CryptoLike } from '../uuid';

describe('bytesToUuidV4', () => {
  it('sets version and variant bits', () => {
    const id = bytesToUuidV4(new Uint8Array(16).fill(0xff));
    expect(id).toBe('ffffffff-ffff-4fff-bfff-ffffffffffff');
    expect(isUuidV4(id)).toBe(true);
    expect(bytesToUuidV4(new Uint8Array(16))).toBe('00000000-0000-4000-8000-000000000000');
  });

  it('rejects short input', () => {
    expect(() => bytesToUuidV4(new Uint8Array(4))).toThrow();
  });
});

describe('uuidV4', () => {
  it('uses randomUUID when available (secure context)', () => {
    const c: CryptoLike = { randomUUID: () => '123e4567-e89b-42d3-a456-426614174000' };
    expect(uuidV4(c)).toBe('123e4567-e89b-42d3-a456-426614174000');
  });

  it('insecure context (no randomUUID) → getRandomValues', () => {
    const getRandomValues = vi.fn(<T extends Uint8Array>(a: T) => {
      a.fill(0xab);
      return a;
    });
    const id = uuidV4({ getRandomValues });
    expect(getRandomValues).toHaveBeenCalledOnce();
    expect(id).toBe('abababab-abab-4bab-abab-abababababab');
    expect(isUuidV4(id)).toBe(true);
  });

  it('throwing randomUUID falls back', () => {
    const id = uuidV4({
      randomUUID: () => {
        throw new Error('not allowed');
      },
      getRandomValues: <T extends Uint8Array>(a: T) => a,
    });
    expect(id).toBe('00000000-0000-4000-8000-000000000000');
  });

  it('randomUUID returning garbage falls back', () => {
    const id = uuidV4({ randomUUID: () => 'nope', getRandomValues: <T extends Uint8Array>(a: T) => a });
    expect(isUuidV4(id)).toBe(true);
  });

  it('no crypto at all → Math.random fallback', () => {
    const id = uuidV4(null, () => 0.5);
    expect(id).toBe('80808080-8080-4080-8080-808080808080');
  });

  it('throwing getRandomValues → Math.random fallback', () => {
    const id = uuidV4(
      {
        getRandomValues: () => {
          throw new Error('x');
        },
      },
      () => 0,
    );
    expect(id).toBe('00000000-0000-4000-8000-000000000000');
  });

  it('default environment yields unique valid ids', () => {
    const ids = new Set(Array.from({ length: 200 }, () => uuidV4()));
    expect(ids.size).toBe(200);
    for (const id of ids) expect(isUuidV4(id)).toBe(true);
  });

  it('clientId delegates to uuidV4', () => {
    expect(isUuidV4(clientId())).toBe(true);
  });

  it('isUuidV4', () => {
    expect(isUuidV4('123e4567-e89b-12d3-a456-426614174000')).toBe(false); // v1
    expect(isUuidV4(42)).toBe(false);
  });
});
