// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Judge-app variant of `useDialog()`: phone-first bottom sheet with the
 * big touch buttons of `ui.ts` (same look as the ErrorPad dialogs).
 * No external dependencies, works in the static SPA build.
 */

import { useId, type ReactNode } from 'react';

import type { DialogRequest } from '../../../lib/dialog';
import { inBody, useDialogKeys, useDialogQueue, type DialogQueue } from '../useDialog';
import { BTN_CANCEL, BTN_DANGER, BTN_PRIMARY } from './ui';

export function JudgeDialogView({
  request,
  settle,
}: {
  request: DialogRequest;
  settle: (confirmed: boolean) => void;
}) {
  const titleId = useId();
  const ok = () => settle(true);
  const cancel = () => settle(false);
  useDialogKeys(true, ok, cancel);
  const danger = request.tone === 'danger';
  const focusCancel = danger && request.cancelLabel !== null;
  return (
    <div
      className="fixed inset-0 z-[70] flex items-end justify-center bg-black/60 sm:items-center"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) cancel();
      }}
    >
      <div
        role={request.kind === 'alert' ? 'alertdialog' : 'dialog'}
        aria-modal="true"
        aria-labelledby={titleId}
        className={`w-full max-w-lg rounded-t-2xl border-t-4 bg-white p-5 pb-8 sm:rounded-2xl ${
          danger ? 'border-red-700' : 'border-ink-50'
        }`}
      >
        <h2
          id={titleId}
          className={`text-[22px] font-bold leading-tight ${danger ? 'text-red-800' : 'text-ink-50'}`}
        >
          {request.title}
        </h2>
        {request.paragraphs.map((p, i) => (
          <p key={i} className="mt-2 whitespace-pre-line text-[16px] text-ink-100">
            {p}
          </p>
        ))}
        <div className="mt-5 flex flex-col gap-3">
          <button
            type="button"
            className={`${danger ? BTN_DANGER : BTN_PRIMARY} min-h-[72px] text-[19px]`}
            onClick={ok}
            autoFocus={!focusCancel}
          >
            {request.confirmLabel}
          </button>
          {request.cancelLabel !== null && (
            <button type="button" className={BTN_CANCEL} onClick={cancel} autoFocus={focusCancel}>
              {request.cancelLabel}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export function useJudgeDialog(): {
  confirm: DialogQueue['confirm'];
  alert: DialogQueue['alert'];
  dialog: ReactNode;
} {
  const { confirm, alert, request, settle } = useDialogQueue();
  const dialog = inBody(
    request ? <JudgeDialogView key={request.id} request={request} settle={settle} /> : null,
  );
  return { confirm, alert, dialog };
}
