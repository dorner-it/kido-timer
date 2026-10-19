// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

// SSR test of the organizer judge-access card (no JSX → *.test.ts).

import { createElement as h } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { DecidersPanel } from '../EventDetail';

type Props = Parameters<typeof DecidersPanel>[0];

const data = (station_token: string) =>
  ({
    event: {
      id: 'ev-1',
      owner_sub: 'owner-1',
      station_token,
      decider_tokens: ['s1', 's2', 's3', 's4'],
    },
  }) as unknown as Props['data'];

describe('DecidersPanel (one station link, no legacy slot links)', () => {
  it('shows the station link with copy/open/QR and "Zugang neu erzeugen"', () => {
    const html = renderToStaticMarkup(h(DecidersPanel, { data: data('st-tok'), onChange: () => {} }));
    expect(html).toContain('/decider/owner-1/st-tok');
    expect(html).toContain('Kopieren');
    expect(html).toContain('QR-Code anzeigen: Strafrichter-Zugang');
    expect(html).toContain('Zugang neu erzeugen');
    // Legacy B1–B4 slot links are gone from the UI.
    expect(html).not.toMatch(/\/decider\/owner-1\/[1-4]\//);
    expect(html).not.toContain('>B1<');
  });

  it('offers "Zugang erzeugen" when the event has no station link yet', () => {
    const html = renderToStaticMarkup(h(DecidersPanel, { data: data(''), onChange: () => {} }));
    expect(html).toContain('noch keinen Strafrichter-Zugang');
    expect(html).toContain('Zugang erzeugen');
    expect(html).not.toContain('/decider/');
  });
});
