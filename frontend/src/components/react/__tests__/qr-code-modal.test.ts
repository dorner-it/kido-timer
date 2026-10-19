// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

// SSR smoke tests of the organizer QR icon/modal (no JSX → *.test.ts).

import { createElement as h } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { QrCodeModal, QrIconButton } from '../QrCodeModal';

const URL_ = 'https://time.ki-do.it/decider/sub-1/2/tok';

describe('QrIconButton', () => {
  it('renders a small icon button, modal closed', () => {
    const html = renderToStaticMarkup(h(QrIconButton, { title: 'Strafrichter B2', url: URL_ }));
    expect(html).toContain('aria-label="QR-Code anzeigen: Strafrichter B2"');
    expect(html).toContain('<svg');
    expect(html).not.toContain('role="dialog"');
  });
});

describe('QrCodeModal', () => {
  it('shows title, a large QR SVG, the URL and a close button', () => {
    const html = renderToStaticMarkup(
      h(QrCodeModal, { title: 'Strafrichter B2', url: URL_, onClose: () => {} }),
    );
    expect(html).toContain('role="dialog"');
    expect(html).toContain('Strafrichter B2');
    expect(html).toContain('viewBox="0 0 ');
    expect(html).toContain(URL_);
    expect(html).toContain('Schließen');
  });
});
