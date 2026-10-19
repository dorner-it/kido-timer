// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import type { Run } from '../../lib/api';
import { formatTimeInput, handTimeRawText, isHandTime } from '../../lib/hand-time';
import { resultTimeMs, type RunSummary } from '../../lib/run-review';

type RunTimeFields = Pick<Run, 'original_time_ms' | 'time_source' | 'hand_time_1_ms' | 'hand_time_2_ms'>;

/** "Handzeit" badge with both raw stopwatch readings. */
export function HandTimeBadge({ run }: { run: RunTimeFields }) {
  if (!isHandTime(run)) return null;
  const raw = handTimeRawText(run);
  return (
    <span
      className="pill bg-status-captured/15 text-status-captured"
      title={`Handzeit (Lichtschranke ausgefallen) – Mittelwert aus ${raw}`}
    >
      Handzeit
      <span className="font-mono normal-case tracking-normal opacity-80">{raw}</span>
    </span>
  );
}

/**
 * Measured time (+ Handzeit badge). With a summary, also the result
 * time = measured + 15 s × SZ, or "ungültig".
 */
export function RunTime({
  run,
  summary,
  compact = false,
}: {
  run: RunTimeFields;
  summary?: RunSummary;
  compact?: boolean;
}) {
  const measured = run.original_time_ms;
  const result = summary ? resultTimeMs(run, summary) : null;
  return (
    <span className="inline-flex flex-wrap items-center gap-2">
      <span className="font-mono tnum text-ink-50">
        {measured != null ? formatTimeInput(measured) : '—'}
      </span>
      <HandTimeBadge run={run} />
      {summary && !compact && measured != null && (
        <>
          {!summary.valid ? (
            <span className="font-mono text-[12px] text-status-unknown">→ ungültig</span>
          ) : summary.sz > 0 && result != null ? (
            <span className="font-mono text-[12px] tnum text-ink-100">
              +{summary.penaltyMs / 1000} s → <strong>{formatTimeInput(result)}</strong>
              {summary.provisional && <span className="ml-1 text-status-armed">(vorläufig)</span>}
            </span>
          ) : null}
        </>
      )}
    </span>
  );
}
