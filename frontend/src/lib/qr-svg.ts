// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Local QR code generation for the organizer pages (judge links → big
 * QR in a modal). Pure + synchronous, so it also works in SSR and tests;
 * no network / CDN at runtime.
 */

import qrcode from 'qrcode-generator';

/**
 * Scalable SVG markup (no fixed width/height, `viewBox` only — size it
 * with CSS) of a QR code for `text`. Error correction "M", automatic
 * version, 4-module quiet zone, white background.
 */
export function qrSvg(text: string): string {
  const qr = qrcode(0, 'M');
  qr.addData(text, 'Byte');
  qr.make();
  return qr.createSvgTag({ cellSize: 1, margin: 4, scalable: true });
}
