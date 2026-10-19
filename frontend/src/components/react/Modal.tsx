// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useId, type ReactNode } from 'react';

import { toBody } from './portal';

interface ModalProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  tone?: 'default' | 'danger';
  /** Tailwind z-index class of the overlay (dialogs stacked on top use z-50). */
  layer?: string;
}

/**
 * Minimal accessible modal (Escape / backdrop closes). Rendered into
 * `document.body` ({@link toBody}) so it always covers the whole page.
 */
export function Modal({ title, onClose, children, footer, tone = 'default', layer = 'z-40' }: ModalProps) {
  const titleId = useId();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  return toBody(
    <div
      className={`fixed inset-0 ${layer} flex items-center justify-center bg-ink-50/40 p-4`}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className={[
          'surface flex max-h-[90vh] w-full max-w-lg flex-col gap-4 overflow-y-auto p-6 animate-rise',
          tone === 'danger' ? 'border-status-unknown/60 ring-2 ring-status-unknown/30' : '',
        ].join(' ')}
      >
        <h2
          id={titleId}
          className={[
            'font-display text-[20px] font-semibold',
            tone === 'danger' ? 'text-status-unknown' : 'text-ink-50',
          ].join(' ')}
        >
          {title}
        </h2>
        <div className="flex flex-col gap-3 text-[13px] text-ink-100">{children}</div>
        {footer && <div className="flex flex-wrap justify-end gap-2">{footer}</div>}
      </div>
    </div>
  );
}

interface ConfirmDialogProps {
  title: string;
  children: ReactNode;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  tone?: 'default' | 'danger';
  busy?: boolean;
  confirmDisabled?: boolean;
}

export function ConfirmDialog({
  title,
  children,
  confirmLabel,
  onConfirm,
  onCancel,
  tone = 'default',
  busy = false,
  confirmDisabled = false,
}: ConfirmDialogProps) {
  return (
    <Modal
      title={title}
      onClose={onCancel}
      tone={tone}
      footer={
        <>
          <button type="button" className="btn" onClick={onCancel} disabled={busy}>
            Abbrechen
          </button>
          <button
            type="button"
            className={tone === 'danger' ? 'btn btn-danger' : 'btn btn-primary'}
            onClick={onConfirm}
            disabled={busy || confirmDisabled}
            autoFocus
          >
            {busy ? '…' : confirmLabel}
          </button>
        </>
      }
    >
      {children}
    </Modal>
  );
}
