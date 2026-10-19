// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import { resolveJudgeParams, standaloneApiBase } from '../judge-spa-params';

describe('resolveJudgeParams', () => {
  it('reads token and owner from the QR URL', () => {
    const r = resolveJudgeParams('?t=0123abcd&o=sub-1');
    expect(r).toEqual({ ok: true, params: { ownerSub: 'sub-1', token: '0123abcd' } });
  });

  it('accepts a search string without leading ?', () => {
    const r = resolveJudgeParams('t=tok&o=8a4b-uuid');
    expect(r.ok && r.params.ownerSub).toBe('8a4b-uuid');
  });

  it('falls back to the owner injected by the desktop', () => {
    const r = resolveJudgeParams('?t=tok', 'injected-sub');
    expect(r).toEqual({ ok: true, params: { ownerSub: 'injected-sub', token: 'tok' } });
  });

  it('URL owner wins over the injected one', () => {
    const r = resolveJudgeParams('?t=tok&o=url-sub', 'injected-sub');
    expect(r.ok && r.params.ownerSub).toBe('url-sub');
  });

  it('missing token → message to scan the QR code', () => {
    const r = resolveJudgeParams('?o=sub-1');
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.message).toContain('QR-Code');
  });

  it('missing owner (no injection) → error', () => {
    expect(resolveJudgeParams('?t=tok').ok).toBe(false);
    expect(resolveJudgeParams('?t=tok', 42).ok).toBe(false);
  });

  it('rejects values that would break the API path', () => {
    expect(resolveJudgeParams('?t=a/b&o=x').ok).toBe(false);
    expect(resolveJudgeParams('?t=a%3Fb&o=x').ok).toBe(false);
    expect(resolveJudgeParams('?t=tok&o=a%20b').ok).toBe(false);
    expect(resolveJudgeParams('?t=%20&o=x').ok).toBe(false);
  });

  it('trims whitespace', () => {
    const r = resolveJudgeParams('?t=%20tok%20&o=sub');
    expect(r.ok && r.params.token).toBe('tok');
  });
});

describe('standaloneApiBase', () => {
  it('defaults to same origin', () => {
    expect(standaloneApiBase(undefined)).toBe('');
    expect(standaloneApiBase(null)).toBe('');
  });
  it('keeps a value injected by the serving page', () => {
    expect(standaloneApiBase('http://192.168.1.20:8787')).toBe('http://192.168.1.20:8787');
    expect(standaloneApiBase('')).toBe('');
  });
});
