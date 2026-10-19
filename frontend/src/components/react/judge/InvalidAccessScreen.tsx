// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import { INVALID_ACCESS_TEXT, INVALID_ACCESS_TITLE, unsentMessage } from '../../../lib/judge-access';
import {
  browserScannerEnv,
  scannerAvailability,
  type ScannerAvailability,
} from '../../../lib/judge-qr';
import { Brandmark } from '../Brandmark';
import { QrScanner } from './QrScanner';
import { BTN_NEUTRAL, BTN_PRIMARY, CARD } from './ui';

/**
 * Full-screen "Zugang ungültig" state (judge link rotated / invalid,
 * HTTP 401). Polling has stopped; "Erneut versuchen" reloads the page.
 * Unsent penalty submits stay in local storage and are counted here so
 * the judge can tell the Wettkampfleitung.
 *
 * "QR-Code scannen" opens an in-page camera scanner for the new judge
 * QR code; where the camera cannot be used (plain-HTTP on-prem server,
 * no `mediaDevices`) a hint to use the phone's camera app is shown
 * instead. `scanner` overrides the browser check (tests).
 */
export function InvalidAccessScreen({
  unsent,
  onRetry,
  scanner,
}: {
  unsent: number;
  onRetry: () => void;
  scanner?: ScannerAvailability;
}) {
  const unsentText = unsentMessage(unsent);
  const [avail, setAvail] = useState<ScannerAvailability | null>(scanner ?? null);
  const [scanning, setScanning] = useState(false);

  useEffect(() => {
    if (!scanner) setAvail(scannerAvailability(browserScannerEnv()));
  }, [scanner]);

  const openScanner = () => {
    const a = scanner ?? scannerAvailability(browserScannerEnv());
    setAvail(a);
    if (a.ok) setScanning(true);
  };
  return (
    <div className="mx-auto flex min-h-screen w-full max-w-lg flex-col px-4 py-6">
      <div className="mb-5 flex items-center gap-3">
        <Brandmark size="sm" />
        <span className="text-[13px] font-semibold uppercase tracking-wide text-ink-200">
          Strafrichter
        </span>
      </div>

      <div className={`${CARD} p-5`} role="alert">
        <h1 className="text-[22px] font-bold text-ink-50">{INVALID_ACCESS_TITLE}</h1>
        <p className="mt-2 text-[16px] text-ink-100">{INVALID_ACCESS_TEXT}</p>
        {unsentText && (
          <p className="mt-4 rounded-xl border-2 border-red-700 bg-red-50 px-4 py-3 text-[15px] font-semibold text-red-900">
            {unsentText}
          </p>
        )}
        {scanning ? (
          <div className="mt-5">
            <QrScanner onCancel={() => setScanning(false)} availability={scanner} />
          </div>
        ) : (
          <div className="mt-5 flex flex-col gap-3">
            {avail && !avail.ok ? (
              <p
                className="rounded-xl border-2 border-amber-600 bg-amber-50 px-4 py-3 text-[15px] font-semibold text-ink-50"
                role="status"
              >
                {avail.message}
              </p>
            ) : (
              <button type="button" className={BTN_PRIMARY} onClick={openScanner}>
                <QrGlyph />
                QR-Code scannen
              </button>
            )}
            <button type="button" className={`${BTN_NEUTRAL} justify-center`} onClick={onRetry}>
              Erneut versuchen
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function QrGlyph() {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      width="24"
      height="24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M3 7V4a1 1 0 0 1 1-1h3M17 3h3a1 1 0 0 1 1 1v3M21 17v3a1 1 0 0 1-1 1h-3M7 21H4a1 1 0 0 1-1-1v-3" />
      <path d="M7 12h10" />
    </svg>
  );
}
