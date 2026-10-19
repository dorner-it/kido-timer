// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import { qrSvg } from '../qr-svg';

describe('qrSvg', () => {
  it('returns a scalable SVG for a judge URL', () => {
    const svg = qrSvg('https://time.ki-do.it/decider/8a4b-uuid/Ab3_-xYz');
    expect(svg.startsWith('<svg')).toBe(true);
    expect(svg).toContain('viewBox="0 0 ');
    expect(svg).toContain('<path d="');
    expect(svg.trim().endsWith('</svg>')).toBe(true);
    // scalable: no fixed pixel size, sized via CSS
    expect(svg).not.toMatch(/width="\d+px"/);
  });

  it('does not embed the URL text and is deterministic', () => {
    const url = 'http://192.168.1.20:8787/judge/?t=0123abcd&o=sub-1';
    const a = qrSvg(url);
    expect(a).not.toContain('0123abcd');
    expect(qrSvg(url)).toBe(a);
    expect(qrSvg(`${url}x`)).not.toBe(a);
  });
});

describe('qrcode-generator → jsQR round trip', () => {
  it('a generated judge QR decodes back to the URL and validates', async () => {
    const { default: qrcode } = await import('qrcode-generator');
    const { default: jsQR } = await import('jsqr');
    const { validateJudgeQr } = await import('../judge-qr');
    const url = 'https://time.ki-do.it/decider/8a4b-uuid/Ab3_-xYz0123456789';
    const qr = qrcode(0, 'M');
    qr.addData(url, 'Byte');
    qr.make();
    const cell = 4;
    const margin = 4;
    const n = qr.getModuleCount();
    const size = (n + margin * 2) * cell;
    const px = new Uint8ClampedArray(size * size * 4).fill(255);
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const r = Math.floor(y / cell) - margin;
        const c = Math.floor(x / cell) - margin;
        if (r >= 0 && c >= 0 && r < n && c < n && qr.isDark(r, c)) {
          const i = (y * size + x) * 4;
          px[i] = px[i + 1] = px[i + 2] = 0;
        }
      }
    }
    const decoded = jsQR(px, size, size)?.data;
    expect(decoded).toBe(url);
    expect(validateJudgeQr(decoded, 'http://192.168.1.20:8787')).toEqual({ ok: true, url });
  });
});
