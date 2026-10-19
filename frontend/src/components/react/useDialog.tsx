// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Styled replacement for `window.confirm` / `window.alert`.
 *
 * Each Astro island is its own React root, so there is no global
 * provider: a component calls `useDialog()` and renders the returned
 * `dialog` element somewhere in its tree.
 *
 *   const { confirm, dialog } = useDialog();
 *   if (!(await confirm({ title: 'Läufer löschen?', message: '…',
 *     confirmLabel: 'Löschen', tone: 'danger' }))) return;
 *   …
 *   return <>{…}{dialog}</>;
 *
 * Keyboard: Escape cancels, Enter activates the focused button (or
 * confirms when focus is elsewhere). Initial focus is on „Abbrechen“ for
 * danger dialogs, on the confirm button otherwise. Focus returns to the
 * element that was focused when the dialog was requested.
 *
 * The dialog is portaled to `document.body`: `.surface` cards use
 * `backdrop-filter`, which would otherwise trap the `position: fixed`
 * overlay inside the card.
 */

import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';

import {
  createDialogController,
  type AlertOptions,
  type ConfirmOptions,
  type DialogRequest,
} from '../../lib/dialog';
import { Modal } from './Modal';
import { inBody } from './portal';

export interface DialogQueue {
  confirm: (opts: ConfirmOptions) => Promise<boolean>;
  alert: (opts: AlertOptions) => Promise<void>;
  /** The dialog to render now, or `null`. */
  request: DialogRequest | null;
  settle: (confirmed: boolean) => void;
}

function activeElement(): unknown {
  return typeof document === 'undefined' ? null : document.activeElement;
}

function refocus(el: unknown) {
  if (el && typeof (el as HTMLElement).focus === 'function' && (el as HTMLElement).isConnected) {
    (el as HTMLElement).focus();
  }
}

/** Renderer-agnostic state of a dialog queue (shared by organizer + judge UI). */
export function useDialogQueue(): DialogQueue {
  const [ctl] = useState(createDialogController);
  const [request, setRequest] = useState<DialogRequest | null>(null);

  useEffect(() => {
    const unsubscribe = ctl.subscribe(() => setRequest(ctl.current()));
    setRequest(ctl.current());
    return () => {
      unsubscribe();
      ctl.dispose();
    };
  }, [ctl]);

  const confirm = useCallback((opts: ConfirmOptions) => ctl.confirm(opts, activeElement()), [ctl]);
  const alert = useCallback((opts: AlertOptions) => ctl.alert(opts, activeElement()), [ctl]);
  const settle = useCallback(
    (confirmed: boolean) => {
      const done = ctl.settle(confirmed);
      // Only restore focus once the queue is empty; a follow-up dialog
      // takes focus itself.
      if (done && !ctl.current()) refocus(done.opener);
    },
    [ctl],
  );

  return { confirm, alert, request, settle };
}

export { inBody } from './portal';

/**
 * Escape → cancel, Enter → confirm unless a button has focus (then the
 * browser activates that button). Runs in the capture phase on
 * `document` and stops propagation so underlying modals (which listen on
 * `window`) don't close as well.
 */
export function useDialogKeys(open: boolean, onConfirm: () => void, onCancel: () => void) {
  const handlers = useRef({ onConfirm, onCancel });
  handlers.current = { onConfirm, onCancel };
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        handlers.current.onCancel();
      } else if (e.key === 'Enter') {
        e.stopPropagation();
        const t = e.target as HTMLElement | null;
        if (t && t.tagName === 'BUTTON') return;
        e.preventDefault();
        handlers.current.onConfirm();
      }
    };
    document.addEventListener('keydown', onKey, true);
    return () => document.removeEventListener('keydown', onKey, true);
  }, [open]);
}

export function DialogView({
  request,
  settle,
}: {
  request: DialogRequest;
  settle: (confirmed: boolean) => void;
}) {
  const ok = () => settle(true);
  const cancel = () => settle(false);
  useDialogKeys(true, ok, cancel);
  const danger = request.tone === 'danger';
  const focusCancel = danger && request.cancelLabel !== null;
  return (
    <Modal
      title={request.title}
      onClose={cancel}
      tone={request.tone}
      layer="z-50"
      footer={
        <>
          {request.cancelLabel !== null && (
            <button type="button" className="btn" onClick={cancel} autoFocus={focusCancel}>
              {request.cancelLabel}
            </button>
          )}
          <button
            type="button"
            className={danger ? 'btn btn-danger' : 'btn btn-primary'}
            onClick={ok}
            autoFocus={!focusCancel}
          >
            {request.confirmLabel}
          </button>
        </>
      }
    >
      {request.paragraphs.map((p, i) => (
        <p key={i} className="whitespace-pre-line">
          {p}
        </p>
      ))}
    </Modal>
  );
}

/** Organizer-styled confirm/alert for one React island. */
export function useDialog(): {
  confirm: DialogQueue['confirm'];
  alert: DialogQueue['alert'];
  dialog: ReactNode;
} {
  const { confirm, alert, request, settle } = useDialogQueue();
  const dialog = inBody(
    request ? <DialogView key={request.id} request={request} settle={settle} /> : null,
  );
  return { confirm, alert, dialog };
}
