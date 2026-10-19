// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import { ApiError, apiClient, type StationPenaltyInput } from './api';
import { uuidV4 } from './uuid';

/**
 * Local persistence + retry for decider-pad penalty submissions.
 *
 * The pad is run from tablets/phones at outdoor events with flaky Wi-Fi.
 * If a submit fails because the network is gone, we'd otherwise lose the
 * penalty — the operator pressed the button, the wire dropped, the
 * penalty never reaches DDB.
 *
 * Queue rules:
 * - Every submit gets a client-generated UUID. The backend uses it as
 *   the penalty id, so any number of retries upserts the same row
 *   (no duplicates).
 * - Network errors → enqueue + retry forever.
 * - HTTP 5xx       → enqueue + retry (server is transiently unhappy).
 * - HTTP 4xx       → drop from queue (won't fix itself: bad token,
 *                    offline mode, no active run, etc.) and surface
 *                    to the caller via the dropped log.
 *
 * Queue state lives in localStorage so a refresh during a flaky run
 * doesn't lose work.
 */

const STORAGE_KEY = 'kido_penalty_queue_v1';
const DROPPED_KEY = 'kido_penalty_dropped_v1';

/** Legacy slot-token submit. Items stored before PR 2 have no `flow`. */
export interface QueuedSlotSubmit {
  flow?: 'slot';
  client_id: string;
  owner_sub: string;
  slot: number;
  token: string;
  delta_ms: number;
  reason_code: string | null;
  /** Display label for the "Eigene Eingaben" list. */
  label: string;
  queued_at: number;
}

/** Station-token submit (2027 catalogue code, PR 2). */
export interface QueuedStationSubmit {
  flow: 'station';
  client_id: string;
  owner_sub: string;
  token: string;
  /** Full body; `client_id` inside equals the item's `client_id`. */
  body: StationPenaltyInput;
  label: string;
  queued_at: number;
}

export type QueuedSubmit = QueuedSlotSubmit | QueuedStationSubmit;

export type DroppedSubmit = QueuedSubmit & {
  dropped_at: number;
  reason: string;
  http_status: number | null;
};

export function isStationSubmit(item: QueuedSubmit): item is QueuedStationSubmit {
  return item.flow === 'station';
}

/** Builds a station queue item; the body always carries the client id
 *  so retries upsert the same penalty server-side. */
export function stationQueueItem(opts: {
  owner_sub: string;
  token: string;
  body: StationPenaltyInput;
  label: string;
  client_id: string;
  now?: number;
}): QueuedStationSubmit {
  return {
    flow: 'station',
    client_id: opts.client_id,
    owner_sub: opts.owner_sub,
    token: opts.token,
    body: { ...opts.body, client_id: opts.client_id },
    label: opts.label,
    queued_at: opts.now ?? Date.now(),
  };
}

type Listener = () => void;
const listeners = new Set<Listener>();

function read<T>(key: string): T[] {
  if (typeof localStorage === 'undefined') return [];
  try {
    const raw = localStorage.getItem(key);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? (parsed as T[]) : [];
  } catch {
    return [];
  }
}

function write<T>(key: string, items: T[]): void {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(key, JSON.stringify(items));
  } catch {
    // quota exceeded / storage blocked: keep going, listeners still fire
  }
  listeners.forEach((fn) => fn());
}

export const penaltyQueue = {
  list(): QueuedSubmit[] {
    return read<QueuedSubmit>(STORAGE_KEY);
  },
  enqueue(item: QueuedSubmit): void {
    const items = penaltyQueue.list();
    if (items.some((i) => i.client_id === item.client_id)) return;
    items.push(item);
    write(STORAGE_KEY, items);
  },
  remove(client_id: string): void {
    write(
      STORAGE_KEY,
      penaltyQueue.list().filter((i) => i.client_id !== client_id),
    );
  },
  size(): number {
    return penaltyQueue.list().length;
  },
  subscribe(fn: Listener): () => void {
    listeners.add(fn);
    return () => {
      listeners.delete(fn);
    };
  },
};

export const droppedLog = {
  list(): DroppedSubmit[] {
    return read<DroppedSubmit>(DROPPED_KEY);
  },
  push(item: DroppedSubmit): void {
    const items = droppedLog.list();
    items.unshift(item);
    write(DROPPED_KEY, items.slice(0, 20));
  },
  clear(): void {
    write(DROPPED_KEY, []);
  },
};

export interface SubmitOk {
  ok: true;
  /** Server-confirmed penalty id (equals client_id when client supplied). */
  id: string;
}
export interface SubmitFail {
  ok: false;
  retriable: boolean;
  status: number | null;
  message: string;
}
export type SubmitResult = SubmitOk | SubmitFail;

/** Error → queue decision: HTTP 5xx / 408 / 429 and network errors are
 *  retriable, other 4xx are final. */
export function classifySubmitError(err: unknown): SubmitFail {
  if (err instanceof ApiError) {
    const retriable = err.status >= 500 || err.status === 408 || err.status === 429;
    return { ok: false, retriable, status: err.status, message: err.message };
  }
  return {
    ok: false,
    retriable: true,
    status: null,
    message: err instanceof Error ? err.message : 'Netzwerkfehler',
  };
}

export type SubmitSender = (item: QueuedSubmit) => Promise<{ id: string }>;

/** Default transport: slot route or station route depending on the item. */
export const defaultSender: SubmitSender = (item) => {
  if (isStationSubmit(item)) {
    return apiClient.deciderStation.submit(item.owner_sub, item.token, item.body);
  }
  return apiClient.decider.submit(item.owner_sub, item.slot, item.token, {
    delta_ms: item.delta_ms,
    reason_code: item.reason_code,
    client_id: item.client_id,
  });
};

/**
 * Send one queued item to the API and translate the outcome into a
 * decision tree the queue can act on. Network errors → retriable;
 * 4xx → drop; 5xx → retriable.
 */
export async function attemptSubmit(
  item: QueuedSubmit,
  send: SubmitSender = defaultSender,
): Promise<SubmitResult> {
  try {
    const created = await send(item);
    return { ok: true, id: created.id };
  } catch (err) {
    return classifySubmitError(err);
  }
}

/**
 * Walk the queue, oldest first. Stops on the first retriable failure
 * (no point hammering when the wire is down). Returns counts and the
 * confirmed ids so the caller can update its UI.
 */
export async function drainOnce(): Promise<{
  confirmed: { client_id: string; id: string }[];
  dropped: DroppedSubmit[];
  remaining: number;
}> {
  const confirmed: { client_id: string; id: string }[] = [];
  const dropped: DroppedSubmit[] = [];

  for (const item of penaltyQueue.list()) {
    const result = await attemptSubmit(item);
    if (result.ok) {
      penaltyQueue.remove(item.client_id);
      confirmed.push({ client_id: item.client_id, id: result.id });
      continue;
    }
    if (!result.retriable) {
      penaltyQueue.remove(item.client_id);
      const drop: DroppedSubmit = {
        ...item,
        dropped_at: Date.now(),
        reason: result.message,
        http_status: result.status,
      };
      droppedLog.push(drop);
      dropped.push(drop);
      continue;
    }
    // First retriable failure: stop draining; the network is bad.
    break;
  }

  return { confirmed, dropped, remaining: penaltyQueue.size() };
}

/**
 * React hook exposing reactive queue state plus an automatic drain loop.
 * Drains on:
 *   - mount,
 *   - browser `online` event,
 *   - every {@link DRAIN_INTERVAL_MS} ms.
 */
const DRAIN_INTERVAL_MS = 8_000;

export function usePenaltyQueue(options?: {
  onConfirmed?: (entry: { client_id: string; id: string }) => void;
  onDropped?: (entry: DroppedSubmit) => void;
}) {
  // Initial values must match the server render (SSR has neither
  // localStorage nor a meaningful navigator.onLine) — the real values
  // are read in the effect right after hydration.
  const [items, setItems] = useState<QueuedSubmit[]>([]);
  const [online, setOnline] = useState(true);
  const [draining, setDraining] = useState(false);

  useEffect(() => {
    setItems(penaltyQueue.list());
    if (typeof navigator !== 'undefined' && typeof navigator.onLine === 'boolean') {
      setOnline(navigator.onLine);
    }
    const sub = penaltyQueue.subscribe(() => setItems(penaltyQueue.list()));
    const onOnline = () => {
      setOnline(true);
    };
    const onOffline = () => setOnline(false);
    window.addEventListener('online', onOnline);
    window.addEventListener('offline', onOffline);
    return () => {
      sub();
      window.removeEventListener('online', onOnline);
      window.removeEventListener('offline', onOffline);
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const drain = async () => {
      if (cancelled) return;
      if (penaltyQueue.size() === 0) return;
      if (typeof navigator !== 'undefined' && navigator.onLine === false) return;
      setDraining(true);
      try {
        const res = await drainOnce();
        res.confirmed.forEach((c) => options?.onConfirmed?.(c));
        res.dropped.forEach((d) => options?.onDropped?.(d));
      } finally {
        if (!cancelled) setDraining(false);
      }
    };
    drain();
    const onOnline = () => drain();
    window.addEventListener('online', onOnline);
    const interval = window.setInterval(drain, DRAIN_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.removeEventListener('online', onOnline);
      window.clearInterval(interval);
    };
  }, [options]);

  return { items, online, draining };
}

/** UUID v4; works outside secure contexts (on-prem over plain HTTP). */
export function clientId(): string {
  return uuidV4();
}
