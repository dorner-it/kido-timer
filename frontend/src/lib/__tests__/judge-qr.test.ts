// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it } from 'vitest';

import {
  CAMERA_BUSY_TEXT,
  CAMERA_DENIED_TEXT,
  CAMERA_FAILED_TEXT,
  CAMERA_MISSING_TEXT,
  CAMERA_UNAVAILABLE_TEXT,
  CLOUD_JUDGE_ORIGIN,
  QR_INVALID_HINT,
  QR_SAME_LINK_HINT,
  cameraErrorMessage,
  frameSize,
  scannerAvailability,
  stopMediaStream,
  validateJudgeQr,
} from '../judge-qr';

const CLOUD = 'https://time.ki-do.it';
const ONPREM = 'http://192.168.1.20:8787';
const OWNER = '8a4b1c2d-0000-4000-8000-123456789abc';
const STATION_TOKEN = 'Ab3_-xYz0123456789abcdefghijklmnopqrstuvwx';

const ok = (url: string) => ({ ok: true, url });
const bad = { ok: false, message: QR_INVALID_HINT };

describe('validateJudgeQr — accepted', () => {
  it('cloud station URL on the same origin', () => {
    const u = `${CLOUD}/decider/${OWNER}/${STATION_TOKEN}`;
    expect(validateJudgeQr(u, CLOUD)).toEqual(ok(u));
  });

  it('legacy slot decider URL on the same origin', () => {
    const u = `https://app-dev.ki-do.it/decider/${OWNER}/3/tok_en-1`;
    expect(validateJudgeQr(u, 'https://app-dev.ki-do.it')).toEqual(ok(u));
  });

  it('on-prem /judge/?t=…&o=… URL on the same origin', () => {
    const u = `${ONPREM}/judge/?t=0123abcd&o=${OWNER}`;
    expect(validateJudgeQr(u, ONPREM)).toEqual(ok(u));
    // owner is optional (desktop injects it), /judge redirects to /judge/
    expect(validateJudgeQr(`${ONPREM}/judge/?t=0123abcd`, ONPREM).ok).toBe(true);
    expect(validateJudgeQr(`${ONPREM}/judge?t=0123abcd`, ONPREM).ok).toBe(true);
  });

  it('https://time.ki-do.it judge URLs from any origin', () => {
    expect(CLOUD_JUDGE_ORIGIN).toBe(CLOUD);
    const station = `${CLOUD}/decider/${OWNER}/${STATION_TOKEN}`;
    const slot = `${CLOUD}/decider/${OWNER}/1/tok`;
    expect(validateJudgeQr(station, ONPREM)).toEqual(ok(station));
    expect(validateJudgeQr(slot, 'http://localhost:4321')).toEqual(ok(slot));
  });

  it('trims whitespace and normalizes host case; trailing slash ok', () => {
    const r = validateJudgeQr(`  HTTPS://TIME.KI-DO.IT/decider/${OWNER}/tok/\n`, ONPREM);
    expect(r).toEqual(ok(`${CLOUD}/decider/${OWNER}/tok/`));
  });
});

describe('validateJudgeQr — rejected', () => {
  const judgePath = `/decider/${OWNER}/${STATION_TOKEN}`;

  it.each([
    ['other host', `https://evil.com${judgePath}`],
    ['http variant of the cloud host', `http://time.ki-do.it${judgePath}`],
    ['other port on cloud host', `https://time.ki-do.it:8443${judgePath}`],
    ['lookalike suffix host', `https://time.ki-do.it.evil.com${judgePath}`],
    ['host in path', `https://evil.com/time.ki-do.it${judgePath}`],
    ['userinfo trick', `https://time.ki-do.it@evil.com${judgePath}`],
    ['credentials on cloud host', `https://user:pw@time.ki-do.it${judgePath}`],
    ['subdomain of cloud host', `https://x.time.ki-do.it${judgePath}`],
    ['javascript:', 'javascript:alert(1)//time.ki-do.it/decider/a/b'],
    ['data:', 'data:text/html,<script>alert(1)</script>'],
    ['relative path', judgePath],
    ['protocol-relative', `//time.ki-do.it${judgePath}`],
    ['garbage', 'hello world'],
    ['empty', ''],
    ['WIFI QR', 'WIFI:T:WPA;S:kido;P:secret;;'],
  ])('%s', (_label, raw) => {
    expect(validateJudgeQr(raw, ONPREM)).toEqual(bad);
  });

  it.each([
    ['root', '/'],
    ['organizer page', '/events/abc'],
    ['login', '/login'],
    ['api path', `/api/decider/${OWNER}/tok/state`],
    ['decider with one segment', `/decider/${OWNER}`],
    ['decider with slot 5', `/decider/${OWNER}/5/tok`],
    ['decider too deep', `/decider/${OWNER}/1/tok/extra`],
    ['encoded slash in segment', `/decider/${OWNER}/a%2Fb`],
    ['judge without token', '/judge/'],
    ['judge with empty token', '/judge/?t='],
    ['judge with bad token', '/judge/?t=a%20b'],
    ['judge with bad owner', '/judge/?t=tok&o=a%2Fb'],
    ['judge asset path', '/judge/assets/index.js?t=tok'],
  ])('non-judge path on allowed origin: %s', (_label, path) => {
    expect(validateJudgeQr(`${CLOUD}${path}`, CLOUD)).toEqual(bad);
  });

  it('non-string input', () => {
    expect(validateJudgeQr(undefined, CLOUD)).toEqual(bad);
    expect(validateJudgeQr(42, CLOUD)).toEqual(bad);
  });

  it('overlong input', () => {
    expect(validateJudgeQr(`${CLOUD}/decider/${OWNER}/${'a'.repeat(3000)}`, CLOUD)).toEqual(bad);
  });

  it('an unusable current origin only allows the cloud origin', () => {
    expect(validateJudgeQr(`${ONPREM}/judge/?t=tok`, 'null')).toEqual(bad);
    expect(validateJudgeQr(`${CLOUD}/decider/o/t`, 'null').ok).toBe(true);
  });

  it('the current (invalid) link gets its own hint', () => {
    const u = `${ONPREM}/judge/?t=old&o=${OWNER}`;
    expect(validateJudgeQr(u, ONPREM, u)).toEqual({ ok: false, message: QR_SAME_LINK_HINT });
    expect(validateJudgeQr(`${ONPREM}/judge/?t=new&o=${OWNER}`, ONPREM, u).ok).toBe(true);
  });
});

describe('scannerAvailability', () => {
  const gum = () => Promise.resolve();

  it('secure context with getUserMedia → available', () => {
    expect(scannerAvailability({ isSecureContext: true, mediaDevices: { getUserMedia: gum } })).toEqual({
      ok: true,
    });
  });

  it('plain-HTTP on-prem page (not secure, no mediaDevices) → fallback text', () => {
    expect(scannerAvailability({ isSecureContext: false, mediaDevices: undefined })).toEqual({
      ok: false,
      message: CAMERA_UNAVAILABLE_TEXT,
    });
  });

  it('not secure even with mediaDevices → fallback text', () => {
    expect(scannerAvailability({ isSecureContext: false, mediaDevices: { getUserMedia: gum } }).ok).toBe(
      false,
    );
  });

  it('mediaDevices or getUserMedia missing → fallback text', () => {
    expect(scannerAvailability({ isSecureContext: true }).ok).toBe(false);
    expect(scannerAvailability({ isSecureContext: true, mediaDevices: null }).ok).toBe(false);
    expect(scannerAvailability({ isSecureContext: true, mediaDevices: {} })).toEqual({
      ok: false,
      message: CAMERA_UNAVAILABLE_TEXT,
    });
  });

  it('fallback text tells the judge to use the phone camera app', () => {
    expect(CAMERA_UNAVAILABLE_TEXT).toBe(
      'Kamera hier nicht verfügbar – bitte den QR-Code mit der Kamera-App des Handys scannen.',
    );
  });
});

describe('cameraErrorMessage', () => {
  const err = (name: string) => Object.assign(new Error('x'), { name });

  it('maps getUserMedia errors to German texts', () => {
    expect(cameraErrorMessage(err('NotAllowedError'))).toBe(CAMERA_DENIED_TEXT);
    expect(cameraErrorMessage(err('SecurityError'))).toBe(CAMERA_UNAVAILABLE_TEXT);
    expect(cameraErrorMessage(err('NotFoundError'))).toBe(CAMERA_MISSING_TEXT);
    expect(cameraErrorMessage(err('OverconstrainedError'))).toBe(CAMERA_MISSING_TEXT);
    expect(cameraErrorMessage(err('NotReadableError'))).toBe(CAMERA_BUSY_TEXT);
    expect(cameraErrorMessage(err('Whatever'))).toBe(CAMERA_FAILED_TEXT);
    expect(cameraErrorMessage('boom')).toBe(CAMERA_FAILED_TEXT);
    expect(cameraErrorMessage(null)).toBe(CAMERA_FAILED_TEXT);
  });
});

describe('stopMediaStream', () => {
  it('stops every track and tolerates null / throwing tracks', () => {
    const stopped: string[] = [];
    const track = (id: string, throws = false) => ({
      stop: () => {
        if (throws) throw new Error('already');
        stopped.push(id);
      },
    });
    stopMediaStream({ getTracks: () => [track('a'), track('b', true), track('c')] });
    expect(stopped).toEqual(['a', 'c']);
    expect(() => stopMediaStream(null)).not.toThrow();
    expect(() => stopMediaStream(undefined)).not.toThrow();
  });
});

describe('frameSize', () => {
  it('downscales to at most 640 px on the longest side', () => {
    expect(frameSize(1920, 1080)).toEqual({ width: 640, height: 360 });
    expect(frameSize(1080, 1920)).toEqual({ width: 360, height: 640 });
    expect(frameSize(320, 240)).toEqual({ width: 320, height: 240 });
    expect(frameSize(0, 0)).toEqual({ width: 0, height: 0 });
  });
});
