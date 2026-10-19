// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import {
  SUBMISSION_STATUS_LABELS,
  canWithdraw,
  isStruck,
  struckByLabel,
  type SubmissionRow,
  type SubmissionStatus,
} from '../../../lib/judge-submissions';
import { KindBadge } from './ErrorPad';

const STATUS_CLASSES: Record<SubmissionStatus, string> = {
  sending: 'bg-amber-100 text-amber-900 border-amber-700',
  pending: 'bg-white text-ink-50 border-ink-50',
  approved: 'bg-green-700 text-white border-green-800',
  rejected: 'bg-ink-600 text-ink-100 border-ink-400',
  withdrawn: 'bg-ink-600 text-ink-100 border-ink-400',
};

/** "Eigene Eingaben" for the current run(s). */
export function SubmissionList({
  rows,
  runLabel,
  onWithdraw,
}: {
  rows: SubmissionRow[];
  /** Run id → `"Bahn 1 · Team"`; `null` hides the line (single run). */
  runLabel: (runId: string) => string | null;
  onWithdraw: (row: SubmissionRow) => void;
}) {
  // Two-tap withdraw: first tap arms the button for 4 s.
  const [armed, setArmed] = useState<string | null>(null);
  useEffect(() => {
    if (!armed) return;
    const t = window.setTimeout(() => setArmed(null), 4000);
    return () => window.clearTimeout(t);
  }, [armed]);

  if (rows.length === 0) {
    return <p className="text-[15px] text-ink-200">Für den aktuellen Lauf noch nichts gemeldet.</p>;
  }
  return (
    <ul className="flex flex-col gap-2">
      {rows.map((r) => {
        const struck = isStruck(r.status);
        const by = struckByLabel(r.status);
        const lane = runLabel(r.run_id);
        return (
          <li
            key={r.key}
            className={`flex items-center gap-3 rounded-xl border-2 px-3 py-3 ${
              struck ? 'border-ink-400 bg-ink-700' : 'border-ink-50 bg-white'
            }`}
          >
            <KindBadge kind={r.kind} />
            <div className="flex min-w-0 flex-1 flex-col">
              <span
                className={`text-[16px] font-semibold leading-snug ${
                  struck ? 'text-ink-300 line-through' : 'text-ink-50'
                }`}
              >
                {r.label}
              </span>
              <span className="mt-1 flex flex-wrap items-center gap-2 text-[13px] text-ink-200">
                <span
                  className={`rounded-md border px-2 py-0.5 font-bold uppercase tracking-wide ${STATUS_CLASSES[r.status]}`}
                >
                  {SUBMISSION_STATUS_LABELS[r.status]}
                </span>
                {by && <span>{by}</span>}
                {lane && <span>{lane}</span>}
              </span>
            </div>
            {canWithdraw(r.status) && (
              <button
                type="button"
                className={`min-h-[52px] shrink-0 rounded-lg border-2 px-3 text-[14px] font-bold ${
                  armed === r.key
                    ? 'border-red-800 bg-red-700 text-white'
                    : 'border-red-700 bg-white text-red-800'
                }`}
                onClick={() => {
                  if (armed === r.key) {
                    setArmed(null);
                    onWithdraw(r);
                  } else {
                    setArmed(r.key);
                  }
                }}
              >
                {armed === r.key ? 'Wirklich?' : r.status === 'sending' ? 'Verwerfen' : 'Zurückziehen'}
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
}
