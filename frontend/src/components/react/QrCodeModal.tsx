// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useMemo, useState } from 'react';

import { qrSvg } from '../../lib/qr-svg';
import { Modal } from './Modal';

/**
 * Large QR code of a judge link, generated locally (`qr-svg.ts`). Sized
 * so a phone held in front of the laptop screen can scan it.
 */
export function QrCodeModal({
  title,
  url,
  onClose,
}: {
  title: string;
  url: string;
  onClose: () => void;
}) {
  const svg = useMemo(() => qrSvg(url), [url]);
  return (
    <Modal
      title={title}
      onClose={onClose}
      footer={
        <button type="button" className="btn btn-primary" onClick={onClose} autoFocus>
          Schließen
        </button>
      }
    >
      <div
        className="mx-auto w-full max-w-[min(420px,60vh)] rounded bg-white p-2 [&>svg]:block [&>svg]:h-auto [&>svg]:w-full"
        role="img"
        aria-label={`QR-Code für ${title}`}
        // SVG built by `qrcode-generator` from our own URL (modules only, no text).
        dangerouslySetInnerHTML={{ __html: svg }}
      />
      <code className="block break-all rounded border border-[color:var(--hairline-color)] bg-[color:var(--soft-fill)] px-3 py-2 font-mono text-[11px] text-ink-100">
        {url}
      </code>
    </Modal>
  );
}

/** Small QR icon button next to copy/open; opens {@link QrCodeModal}. */
export function QrIconButton({ title, url }: { title: string; url: string }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        type="button"
        className="btn"
        onClick={() => setOpen(true)}
        aria-label={`QR-Code anzeigen: ${title}`}
        title="QR-Code anzeigen"
      >
        <QrIcon />
      </button>
      {open && <QrCodeModal title={title} url={url} onClose={() => setOpen(false)} />}
    </>
  );
}

function QrIcon() {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="currentColor"
    >
      <path d="M3 3h8v8H3V3zm2 2v4h4V5H5zm8-2h8v8h-8V3zm2 2v4h4V5h-4zM3 13h8v8H3v-8zm2 2v4h4v-4H5zm8-2h2v2h-2v-2zm2 2h2v2h-2v-2zm2-2h4v2h-4v-2zm2 2h2v2h-2v-2zm-6 2h2v4h-2v-4zm4 0h2v2h-2v-2zm2 2h2v2h-2v-2zm-4 2h2v2h-2v-2z" />
    </svg>
  );
}
