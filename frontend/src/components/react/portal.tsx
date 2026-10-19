// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';

const stop = (e: { stopPropagation: () => void }) => e.stopPropagation();

/**
 * Render `node` into `document.body`. Overlays must not stay inside the
 * component that opens them: `.surface` cards use `backdrop-filter`,
 * which makes them the containing block of `position: fixed` children,
 * so an overlay rendered inside a card is clipped to and stacked within
 * that card. React events from a portal bubble up the component tree,
 * so clicks are stopped here and never reach e.g. a clickable row the
 * overlay was opened from.
 *
 * On the server (and in SSR tests) there is no `document`; the node is
 * rendered in place there, overlays are only opened after hydration.
 */
export function toBody(node: ReactNode): ReactNode {
  if (node === null) return null;
  if (typeof document === 'undefined') return node;
  return createPortal(
    <div onClick={stop} onMouseDown={stop} onKeyDown={stop}>
      {node}
    </div>,
    document.body,
  );
}

/** Like {@link toBody}, but renders nothing on the server. */
export function inBody(node: ReactNode): ReactNode {
  if (node === null || typeof document === 'undefined') return null;
  return toBody(node);
}
