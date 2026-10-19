// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Promise-based confirm/alert queue that replaces the native
 * `window.confirm` / `window.alert` dialogs. Pure logic (no React, no
 * DOM) so it can be tested in node; the React side lives in
 * `components/react/useDialog.tsx` (organizer) and
 * `components/react/judge/JudgeDialog.tsx` (judge app).
 *
 * One dialog is shown at a time. Further requests are queued FIFO and
 * shown once the current one is settled. `dispose()` (component
 * unmount) settles everything as "cancelled", so awaiting callers never
 * hang. The controller stays usable afterwards (React StrictMode
 * re-runs effects on the same instance).
 */

export type DialogTone = 'default' | 'danger';

export interface ConfirmOptions {
  title: string;
  /** One paragraph per array element. */
  message: string | string[];
  /** Label of the confirm button. Default: „OK“. */
  confirmLabel?: string;
  /** Label of the cancel button. Default: „Abbrechen“. */
  cancelLabel?: string;
  tone?: DialogTone;
}

export interface AlertOptions {
  title: string;
  message: string | string[];
  /** Label of the single button. Default: „OK“. */
  okLabel?: string;
  tone?: DialogTone;
}

export interface DialogRequest {
  id: number;
  kind: 'confirm' | 'alert';
  title: string;
  paragraphs: string[];
  confirmLabel: string;
  /** `null` for alerts (single button). */
  cancelLabel: string | null;
  tone: DialogTone;
  /** Opaque value handed back on settle (e.g. the element to refocus). */
  opener?: unknown;
}

export interface DialogController {
  confirm(opts: ConfirmOptions, opener?: unknown): Promise<boolean>;
  alert(opts: AlertOptions, opener?: unknown): Promise<void>;
  /** The dialog to show now, or `null`. */
  current(): DialogRequest | null;
  /** Number of requests waiting behind the current one. */
  pending(): number;
  /** Settle the current dialog (true = confirmed / OK). Returns it. */
  settle(confirmed: boolean): DialogRequest | null;
  subscribe(listener: () => void): () => void;
  /** Cancel everything (current + queued) and drop listeners. */
  dispose(): void;
}

export const DEFAULT_CONFIRM_LABEL = 'OK';
export const DEFAULT_CANCEL_LABEL = 'Abbrechen';

function paragraphs(message: string | string[]): string[] {
  return (Array.isArray(message) ? message : [message]).filter((p) => p.trim() !== '');
}

export function createDialogController(): DialogController {
  type Entry = { req: DialogRequest; resolve: (confirmed: boolean) => void };
  const queue: Entry[] = [];
  const listeners = new Set<() => void>();
  let nextId = 1;

  const notify = () => {
    for (const l of [...listeners]) l();
  };

  const enqueue = (req: Omit<DialogRequest, 'id'>): Promise<boolean> => {
    return new Promise<boolean>((resolve) => {
      queue.push({ req: { ...req, id: nextId++ }, resolve });
      if (queue.length === 1) notify();
    });
  };

  return {
    confirm(opts, opener) {
      return enqueue({
        kind: 'confirm',
        title: opts.title,
        paragraphs: paragraphs(opts.message),
        confirmLabel: opts.confirmLabel ?? DEFAULT_CONFIRM_LABEL,
        cancelLabel: opts.cancelLabel ?? DEFAULT_CANCEL_LABEL,
        tone: opts.tone ?? 'default',
        opener,
      });
    },
    async alert(opts, opener) {
      await enqueue({
        kind: 'alert',
        title: opts.title,
        paragraphs: paragraphs(opts.message),
        confirmLabel: opts.okLabel ?? DEFAULT_CONFIRM_LABEL,
        cancelLabel: null,
        tone: opts.tone ?? 'default',
        opener,
      });
    },
    current() {
      return queue[0]?.req ?? null;
    },
    pending() {
      return Math.max(0, queue.length - 1);
    },
    settle(confirmed) {
      const head = queue.shift();
      if (!head) return null;
      head.resolve(head.req.kind === 'alert' ? true : confirmed);
      notify();
      return head.req;
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    dispose() {
      const all = queue.splice(0);
      for (const e of all) e.resolve(false);
      listeners.clear();
    },
  };
}
