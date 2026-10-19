// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

// SSR smoke test of the "Zugang ungültig" screen (no JSX → *.test.ts).

import { createElement as h } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { CAMERA_UNAVAILABLE_TEXT } from '../../../../lib/judge-qr';
import { InvalidAccessScreen } from '../InvalidAccessScreen';
import { QrScanner } from '../QrScanner';

describe('InvalidAccessScreen', () => {
  it('shows title, text and retry button; no unsent hint when nothing is queued', () => {
    const html = renderToStaticMarkup(h(InvalidAccessScreen, { unsent: 0, onRetry: () => {} }));
    expect(html).toContain('Zugang ungültig');
    expect(html).toContain(
      'Dieser Strafrichter-Zugang ist nicht mehr gültig. Bitte hol dir bei der Wettkampfleitung einen neuen QR-Code.',
    );
    expect(html).toContain('Erneut versuchen');
    expect(html).not.toContain('nicht gesendet');
  });

  it('mentions unsent submissions', () => {
    expect(renderToStaticMarkup(h(InvalidAccessScreen, { unsent: 1, onRetry: () => {} }))).toContain(
      '1 Meldung konnte nicht gesendet werden.',
    );
    expect(renderToStaticMarkup(h(InvalidAccessScreen, { unsent: 4, onRetry: () => {} }))).toContain(
      '4 Meldungen konnten nicht gesendet werden.',
    );
  });
});

describe('InvalidAccessScreen — QR scanner', () => {
  it('offers "QR-Code scannen" when the camera is usable', () => {
    const html = renderToStaticMarkup(
      h(InvalidAccessScreen, { unsent: 0, onRetry: () => {}, scanner: { ok: true } }),
    );
    expect(html).toContain('QR-Code scannen');
    expect(html).toContain('Erneut versuchen');
  });

  it('shows the camera-app hint instead of a broken scanner without camera', () => {
    const html = renderToStaticMarkup(
      h(InvalidAccessScreen, {
        unsent: 0,
        onRetry: () => {},
        scanner: { ok: false, message: CAMERA_UNAVAILABLE_TEXT },
      }),
    );
    expect(html).not.toContain('QR-Code scannen');
    expect(html).toContain(
      'Kamera hier nicht verfügbar – bitte den QR-Code mit der Kamera-App des Handys scannen.',
    );
    expect(html).toContain('Erneut versuchen');
  });
});

describe('QrScanner', () => {
  it('renders the fallback text (no video) when unavailable', () => {
    const html = renderToStaticMarkup(
      h(QrScanner, { onCancel: () => {}, availability: { ok: false, message: CAMERA_UNAVAILABLE_TEXT } }),
    );
    expect(html).toContain(CAMERA_UNAVAILABLE_TEXT);
    expect(html).not.toContain('<video');
  });

  it('renders video with viewfinder and "Abbrechen" when available', () => {
    const html = renderToStaticMarkup(h(QrScanner, { onCancel: () => {}, availability: { ok: true } }));
    expect(html).toContain('<video');
    expect(html).toContain('playsinline');
    expect(html).toContain('Abbrechen');
  });
});
