// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useRef, useState } from 'react';

import {
  browserScannerEnv,
  cameraErrorMessage,
  frameSize,
  scannerAvailability,
  stopMediaStream,
  validateJudgeQr,
  type ScannerAvailability,
} from '../../../lib/judge-qr';
import { BTN_CANCEL } from './ui';

const SCAN_INTERVAL_MS = 200;

type Decode = (video: HTMLVideoElement) => Promise<string | null>;

interface BarcodeDetectorLike {
  detect: (source: HTMLVideoElement) => Promise<{ rawValue?: string }[]>;
}
interface BarcodeDetectorCtor {
  new (opts: { formats: string[] }): BarcodeDetectorLike;
  getSupportedFormats?: () => Promise<string[]>;
}

/** Native `BarcodeDetector` when it supports QR, else jsQR on a canvas. */
async function createDecoder(): Promise<Decode> {
  const Native = (window as unknown as { BarcodeDetector?: BarcodeDetectorCtor }).BarcodeDetector;
  if (Native) {
    try {
      const formats = Native.getSupportedFormats ? await Native.getSupportedFormats() : ['qr_code'];
      if (formats.includes('qr_code')) {
        const detector = new Native({ formats: ['qr_code'] });
        return async (video) => {
          const codes = await detector.detect(video);
          return codes.find((c) => typeof c.rawValue === 'string' && c.rawValue !== '')?.rawValue ?? null;
        };
      }
    } catch {
      /* fall back to jsQR */
    }
  }

  const { default: jsQR } = await import('jsqr');
  const canvas = document.createElement('canvas');
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) throw new Error('canvas 2d unavailable');
  return async (video) => {
    const { width, height } = frameSize(video.videoWidth, video.videoHeight);
    if (width === 0 || height === 0) return null;
    if (canvas.width !== width) canvas.width = width;
    if (canvas.height !== height) canvas.height = height;
    ctx.drawImage(video, 0, 0, width, height);
    const img = ctx.getImageData(0, 0, width, height);
    return jsQR(img.data, width, height, { inversionAttempts: 'dontInvert' })?.data ?? null;
  };
}

/**
 * In-page QR scanner (rear camera). Opens only codes accepted by
 * `validateJudgeQr`; everything else shows a hint and scanning goes on.
 * All camera tracks are stopped on cancel, unmount and success.
 *
 * `availability` overrides the browser check (tests / SSR).
 */
export function QrScanner({
  onCancel,
  availability,
}: {
  onCancel: () => void;
  availability?: ScannerAvailability;
}) {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const [error, setError] = useState<string | null>(
    availability && !availability.ok ? availability.message : null,
  );
  const [hint, setHint] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const avail = availability ?? scannerAvailability(browserScannerEnv());
    if (!avail.ok) {
      setError(avail.message);
      return;
    }

    let cancelled = false;
    let stream: MediaStream | null = null;
    let timer: ReturnType<typeof setTimeout> | null = null;
    let lastRejected: string | null = null;

    const stop = () => {
      cancelled = true;
      if (timer !== null) clearTimeout(timer);
      timer = null;
      stopMediaStream(stream);
      stream = null;
      const v = videoRef.current;
      if (v) v.srcObject = null;
    };

    const start = async () => {
      try {
        const s = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: 'environment' },
          audio: false,
        });
        if (cancelled) {
          stopMediaStream(s);
          return;
        }
        stream = s;
        const video = videoRef.current;
        if (!video) {
          stop();
          return;
        }
        video.srcObject = s;
        await video.play();
        const decode = await createDecoder();
        if (cancelled) return;
        setReady(true);

        const tick = async () => {
          if (cancelled) return;
          let raw: string | null = null;
          try {
            raw = await decode(video);
          } catch {
            raw = null;
          }
          if (cancelled) return;
          if (raw !== null) {
            const r = validateJudgeQr(raw, window.location.origin, window.location.href);
            if (r.ok) {
              stop();
              window.location.assign(r.url);
              return;
            }
            if (raw !== lastRejected) {
              lastRejected = raw;
              setHint(r.message);
            }
          }
          timer = setTimeout(() => void tick(), SCAN_INTERVAL_MS);
        };
        void tick();
      } catch (err) {
        if (cancelled) return;
        stop();
        setError(cameraErrorMessage(err));
      }
    };

    void start();
    return stop;
  }, [availability]);

  return (
    <div className="flex flex-col gap-4">
      {error ? (
        <p
          className="rounded-xl border-2 border-amber-600 bg-amber-50 px-4 py-3 text-[16px] font-semibold text-ink-50"
          role="status"
        >
          {error}
        </p>
      ) : (
        <>
          <div className="relative aspect-square w-full overflow-hidden rounded-2xl border-2 border-ink-50 bg-black">
            <video
              ref={videoRef}
              className="h-full w-full object-cover"
              playsInline
              muted
              autoPlay
              aria-label="Kamerabild"
            />
            <div
              aria-hidden="true"
              className="pointer-events-none absolute inset-[14%] rounded-xl border-4 border-white/90 shadow-[0_0_0_9999px_rgba(0,0,0,0.35)]"
            />
            {!ready && (
              <p className="absolute inset-x-0 bottom-3 text-center text-[15px] font-semibold text-white">
                Kamera wird gestartet …
              </p>
            )}
          </div>
          <p className="text-[15px] text-ink-100">
            Neuen QR-Code der Wettkampfleitung in den Rahmen halten.
          </p>
          {hint && (
            <p
              className="rounded-xl border-2 border-red-700 bg-red-50 px-4 py-3 text-[15px] font-semibold text-red-900"
              role="status"
            >
              {hint}
            </p>
          )}
        </>
      )}
      <button type="button" className={BTN_CANCEL} onClick={onCancel}>
        {error ? 'Schließen' : 'Abbrechen'}
      </button>
    </div>
  );
}
