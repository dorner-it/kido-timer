// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import { ApiError, apiClient } from './api';

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

export interface QueuedSubmit {
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

export interface DroppedSubmit extends QueuedSubmit {
  dropped_at: number;
  reason: string;
  http_status: number | null;
}

type Listener = () => void;
const listeners = new Set<Listener>();

function read<T>(key: string): T[] {
  if (typeof localStorage === 'undefined') return [];
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T[]) : [];
  } catch {
    return [];
  }
}

function write<T>(key: string, items: T[]): void {
  if (typeof localStorage === 'undefined') return;
  localStorage.setItem(key, JSON.stringify(items));
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

/**
 * Send one queued item to the API and translate the outcome into a
 * decision tree the queue can act on. Network errors → retriable;
 * 4xx → drop; 5xx → retriable.
 */
export async function attemptSubmit(item: QueuedSubmit): Promise<SubmitResult> {
  try {
    const created = await apiClient.decider.submit(
      item.owner_sub,
      item.slot,
      item.token,
      {
        delta_ms: item.delta_ms,
        reason_code: item.reason_code,
        client_id: item.client_id,
      },
    );
    return { ok: true, id: created.id };
  } catch (err) {
    if (err instanceof ApiError) {
      const retriable = err.status >= 500;
      return {
        ok: false,
        retriable,
        status: err.status,
        message: err.message,
      };
    }
    return {
      ok: false,
      retriable: true,
      status: null,
      message: err instanceof Error ? err.message : 'Netzwerkfehler',
    };
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
  const [items, setItems] = useState<QueuedSubmit[]>(() => penaltyQueue.list());
  const [online, setOnline] = useState(() =>
    typeof navigator === 'undefined' ? true : navigator.onLine,
  );
  const [draining, setDraining] = useState(false);

  useEffect(() => {
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
      if (!navigator.onLine) return;
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

/** Browser UUID v4 with a fallback for non-crypto environments. */
export function clientId(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  // RFC 4122 v4 fallback for elderly Safari on older iPads.
  const rnd = (n: number) =>
    Array.from({ length: n }, () => Math.floor(Math.random() * 16).toString(16)).join(
      '',
    );
  return `${rnd(8)}-${rnd(4)}-4${rnd(3)}-${((Math.floor(Math.random() * 4) + 8).toString(16))}${rnd(3)}-${rnd(12)}`;
}
