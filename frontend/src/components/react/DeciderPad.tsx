// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useCallback, useEffect, useMemo, useState } from 'react';

import { ApiError, apiClient, type DeciderState, type TimerMode } from '../../lib/api';
import {
  PRESETS_BY_MODE,
  reasonsForMode,
  variantClasses,
  type PenaltyPreset,
} from '../../lib/decider-presets';
import { formatTimeMs } from '../../lib/format';
import {
  attemptSubmit,
  clientId,
  droppedLog,
  penaltyQueue,
  usePenaltyQueue,
  type DroppedSubmit,
} from '../../lib/penalty-queue';
import { Brandmark } from './Brandmark';

export interface DeciderPadProps {
  ownerSub: string;
  slot: string;
  token: string;
}

const FLASH_MS = 700;

interface ConfirmedSubmission {
  id: string;
  client_id: string;
  label: string;
  at: number;
}

export default function DeciderPad({ ownerSub, slot: slotStr, token }: DeciderPadProps) {
  const slot = Number(slotStr);
  const [state, setState] = useState<DeciderState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState<ConfirmedSubmission[]>([]);
  const [busy, setBusy] = useState(false);
  const [flashKey, setFlashKey] = useState<string | null>(null);
  const [customOpen, setCustomOpen] = useState(false);

  const presets = state ? PRESETS_BY_MODE[state.competition.mode] : [];

  // Promote drained-queue items to the confirmed list so the operator
  // sees their late-arriving entries with a withdraw button.
  const handleConfirmed = useCallback(
    (entry: { client_id: string; id: string }) => {
      setConfirmed((c) => {
        if (c.some((x) => x.client_id === entry.client_id)) return c;
        // We don't carry the original label across reloads; show the
        // confirmed-but-relabelled-from-queue entry generically.
        return [
          { id: entry.id, client_id: entry.client_id, label: 'Strafe gesendet', at: Date.now() },
          ...c,
        ].slice(0, 8);
      });
    },
    [],
  );

  const handleDropped = useCallback((entry: DroppedSubmit) => {
    setError(
      `Strafe „${entry.label}" wurde verworfen (${entry.reason}). Bitte nochmal eingeben.`,
    );
  }, []);

  const queueOptions = useMemo(
    () => ({ onConfirmed: handleConfirmed, onDropped: handleDropped }),
    [handleConfirmed, handleDropped],
  );

  const { items: queued, online, draining } = usePenaltyQueue(queueOptions);

  useEffect(() => {
    refresh();
    const t = window.setInterval(refresh, 4000);
    return () => window.clearInterval(t);
  }, [ownerSub, slot, token]);

  const refresh = async () => {
    try {
      setState(await apiClient.decider.state(ownerSub, slot, token));
      setError(null);
    } catch (err) {
      if (err instanceof ApiError) {
        if (err.status === 401) {
          setError('Ungültiger Token. QR-Code neu laden.');
        } else if (err.status === 404) {
          setError('Aktuell kein aktiver Wettkampf.');
        } else {
          setError(err.message);
        }
      } else {
        setError('Verbindung verloren. Neu versuchen …');
      }
    }
  };

  const submit = async (
    flash: string,
    label: string,
    deltaMs: number,
    reason: string | null,
  ) => {
    if (busy) return;
    if (!state?.current_run) {
      setError('Aktuell läuft kein Lauf.');
      return;
    }
    setBusy(true);

    const queueItem = {
      client_id: clientId(),
      owner_sub: ownerSub,
      slot,
      token,
      delta_ms: deltaMs,
      reason_code: reason,
      label,
      queued_at: Date.now(),
    };

    // Always flash + vibrate immediately so the tap feels responsive
    // even if the backend hasn't acknowledged yet. The operator sees
    // *something happened*; the queue takes over from there.
    setFlashKey(flash);
    if (typeof navigator !== 'undefined' && navigator.vibrate) {
      navigator.vibrate(50);
    }
    window.setTimeout(() => setFlashKey((k) => (k === flash ? null : k)), FLASH_MS);

    try {
      const result = await attemptSubmit(queueItem);
      if (result.ok) {
        setConfirmed((c) =>
          [
            { id: result.id, client_id: queueItem.client_id, label, at: Date.now() },
            ...c,
          ].slice(0, 8),
        );
        setError(null);
      } else if (result.retriable) {
        penaltyQueue.enqueue(queueItem);
      } else {
        droppedLog.push({
          ...queueItem,
          dropped_at: Date.now(),
          reason: result.message,
          http_status: result.status,
        });
        setError(`Strafe abgelehnt (${result.message}). Bitte nochmal eingeben.`);
      }
    } finally {
      setBusy(false);
    }
  };

  const withdraw = async (target: { client_id: string; id?: string }) => {
    // Queued (not yet sent) → just remove locally.
    if (!target.id) {
      penaltyQueue.remove(target.client_id);
      return;
    }
    try {
      await apiClient.decider.withdraw(ownerSub, slot, token, target.id);
      setConfirmed((c) => c.filter((x) => x.client_id !== target.client_id));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Zurückziehen fehlgeschlagen');
    }
  };

  return (
    <div className="min-h-screen flex flex-col">
      <header className="flex items-center justify-between px-6 py-5 border-b border-[color:var(--hairline-color)]">
        <div className="flex items-center gap-3">
          <Brandmark size="sm" />
          <div className="flex flex-col leading-tight">
            <span className="label">Strafrichter</span>
            <span className="font-display text-[14px] text-ink-50">
              {state?.competition.name ?? '—'}
            </span>
          </div>
        </div>
        <ConnectivityPill online={online} pending={queued.length} draining={draining} />
        <div className="flex flex-col items-end leading-tight">
          <span className="font-stencil text-[28px] font-bold text-signal-glow leading-none tnum">
            B{slot}
          </span>
          <span className="label">Slot</span>
        </div>
      </header>

      <main className="flex-1 px-6 pb-10 pt-8 max-w-2xl mx-auto w-full">
        {error && (
          <div className="surface mb-6 px-5 py-3 text-[13px] text-status-unknown" role="alert">
            {error}
          </div>
        )}

        {state?.disabled ? (
          <div className="surface flex flex-col items-center gap-3 py-16 animate-rise">
            <span className="label text-status-armed">Offline-Modus</span>
            <span className="font-display text-[20px] font-medium text-ink-50">
              Wettkampf läuft offline
            </span>
            <p className="text-center font-mono text-[12px] text-ink-200 max-w-sm">
              Strafrichter sind während des Offline-Betriebs deaktiviert. Strafen werden
              direkt am Wettkampf-Laptop entschieden und später per Datei in die Web-App
              übernommen.
            </p>
          </div>
        ) : state?.current_run ? (
          <CurrentRunBanner
            startNumber={state.current_run.team.start_number}
            teamName={state.current_run.team.name}
            runnerName={
              state.current_run.runner
                ? `${state.current_run.runner.first_name} ${state.current_run.runner.last_name}`
                : null
            }
            runNumber={state.current_run.run.run_number}
            originalTimeMs={state.current_run.run.original_time_ms}
            status={state.current_run.run.status}
          />
        ) : (
          <div className="surface flex flex-col items-center gap-2 py-12 animate-rise">
            <span className="label">Bereitschaft</span>
            <span className="font-display text-[18px] text-ink-100">
              Kein Lauf aktiv
            </span>
          </div>
        )}

        {state && !state.disabled && (
          <section className="mt-8">
            <span className="label">Strafen senden</span>
            <div className="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-3 stagger-rise">
              {presets.map((p) => (
                <PresetButton
                  key={p.reason_code}
                  preset={p}
                  disabled={!state?.current_run || busy}
                  flashing={flashKey === p.reason_code}
                  onClick={() =>
                    submit(p.reason_code, p.reason_label, p.delta_ms, p.reason_code)
                  }
                />
              ))}
              <button
                type="button"
                disabled={!state?.current_run || busy}
                onClick={() => setCustomOpen((v) => !v)}
                className={`surface group relative flex flex-col items-center justify-center gap-1 py-7 transition active:scale-[0.97] disabled:opacity-40 border-dashed ${
                  customOpen ? 'ring-2 ring-signal/60' : ''
                }`}
              >
                <span className="font-stencil text-[28px] font-bold text-ink-50 leading-none">
                  {customOpen ? '×' : 'Andere'}
                </span>
                <span className="label">{customOpen ? 'Schließen' : 'Eigener Wert'}</span>
              </button>
            </div>

            {customOpen && (
              <CustomPenaltyForm
                mode={state!.competition.mode}
                disabled={!state?.current_run || busy}
                flashing={flashKey === 'custom'}
                onSubmit={async (seconds, reasonCode) => {
                  await submit(
                    'custom',
                    `+ ${seconds} s`,
                    Math.round(seconds * 1000),
                    reasonCode,
                  );
                }}
              />
            )}
          </section>
        )}

        {state && !state.disabled && (
        <section className="mt-10">
          <span className="label">Eigene Eingaben</span>
          {queued.length === 0 && confirmed.length === 0 ? (
            <p className="mt-2 font-mono text-[12px] text-ink-200">Noch nichts gesendet.</p>
          ) : (
            <ul className="mt-3 flex flex-col divide-y divide-[color:var(--hairline-color)]">
              {queued.map((q) => (
                <li
                  key={q.client_id}
                  className="flex items-center justify-between gap-3 py-3"
                >
                  <span className="flex items-center gap-2 font-display text-[15px] text-status-armed">
                    <span className="h-1.5 w-1.5 rounded-full bg-status-armed animate-pulse" />
                    {q.label}
                  </span>
                  <span className="label">wird gesendet …</span>
                  <button
                    type="button"
                    className="btn btn-danger"
                    onClick={() => withdraw({ client_id: q.client_id })}
                  >
                    Verwerfen
                  </button>
                </li>
              ))}
              {confirmed.map((r) => (
                <li
                  key={r.client_id}
                  className="flex items-center justify-between gap-3 py-3"
                >
                  <span className="font-display text-[15px] text-ink-50">{r.label}</span>
                  <span className="label">{relativeTime(r.at)}</span>
                  <button
                    type="button"
                    className="btn btn-danger"
                    onClick={() => withdraw({ client_id: r.client_id, id: r.id })}
                  >
                    Zurückziehen
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
        )}
      </main>
    </div>
  );
}

function CurrentRunBanner(props: {
  startNumber: number;
  teamName: string;
  runnerName: string | null;
  runNumber: number;
  originalTimeMs: number | null;
  status: string;
}) {
  const { startNumber, teamName, runnerName, runNumber, originalTimeMs, status } = props;
  const showTime = originalTimeMs != null;
  return (
    <div className="surface relative overflow-hidden p-7 animate-rise">
      <div className="flex items-center justify-between gap-5">
        <div className="flex items-center gap-5">
          <span className="font-stencil text-[64px] font-bold leading-none text-signal-glow tnum">
            {startNumber.toString().padStart(2, '0')}
          </span>
          <div className="flex flex-col gap-1">
            <span className="label">Lauf {runNumber}</span>
            <span className="font-display text-[26px] font-semibold leading-tight text-ink-50">
              {teamName}
            </span>
            {runnerName && (
              <span className="font-mono text-[13px] text-ink-200">{runnerName}</span>
            )}
          </div>
        </div>
        {showTime && (
          <div className="flex flex-col items-end leading-tight">
            <span className="label">{status === 'completed' ? 'Endzeit' : 'Aktuell'}</span>
            <span className="font-stencil text-[40px] font-bold leading-none text-signal-glow tnum">
              {formatTimeMs(originalTimeMs)}
            </span>
          </div>
        )}
      </div>
      <div className="scanlines pointer-events-none absolute inset-0 opacity-50" />
    </div>
  );
}

function relativeTime(at: number): string {
  const sec = Math.max(0, Math.floor((Date.now() - at) / 1000));
  if (sec < 60) return `${sec}s`;
  return `${Math.floor(sec / 60)}m`;
}

function ConnectivityPill({
  online,
  pending,
  draining,
}: {
  online: boolean;
  pending: number;
  draining: boolean;
}) {
  if (!online) {
    return (
      <span className="pill bg-status-unknown/15 text-status-unknown" role="status">
        <span className="h-1.5 w-1.5 rounded-full bg-current animate-pulse" />
        Offline{pending > 0 ? ` (${pending})` : ''}
      </span>
    );
  }
  if (pending > 0) {
    return (
      <span className="pill bg-status-armed/15 text-status-armed" role="status">
        <span className="h-1.5 w-1.5 rounded-full bg-current animate-pulse" />
        {draining ? 'Sendet …' : 'Wartend'} ({pending})
      </span>
    );
  }
  return (
    <span className="pill bg-status-confirmed/15 text-status-confirmed" role="status">
      <span className="h-1.5 w-1.5 rounded-full bg-current" />
      Live
    </span>
  );
}

function PresetButton({
  preset,
  disabled,
  flashing,
  onClick,
}: {
  preset: PenaltyPreset;
  disabled: boolean;
  flashing: boolean;
  onClick: () => void;
}) {
  const variant = variantClasses(preset.variant);
  const flashCls = flashing
    ? 'ring-2 ring-status-confirmed bg-status-confirmed/15 scale-[0.98]'
    : '';
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`surface group relative flex flex-col items-center justify-center gap-1 py-7 transition active:scale-[0.97] disabled:opacity-40 ${variant} ${flashCls}`}
    >
      <span className="font-stencil text-[28px] font-bold text-ink-50 leading-none">
        {preset.label}
      </span>
      <span className="label">{preset.reason_label}</span>
    </button>
  );
}

function CustomPenaltyForm({
  mode,
  disabled,
  flashing,
  onSubmit,
}: {
  mode: TimerMode;
  disabled: boolean;
  flashing: boolean;
  onSubmit: (seconds: number, reasonCode: string | null) => Promise<void>;
}) {
  const reasons = reasonsForMode(mode);
  const [seconds, setSeconds] = useState<string>('');
  const [reasonCode, setReasonCode] = useState<string>(reasons[0]?.code ?? '');
  const [submitting, setSubmitting] = useState(false);

  const handle = async (e: React.FormEvent) => {
    e.preventDefault();
    const s = Number(seconds.replace(',', '.'));
    if (!Number.isFinite(s) || s === 0) return;
    setSubmitting(true);
    try {
      await onSubmit(s, reasonCode || null);
      setSeconds('');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form
      onSubmit={handle}
      className={`surface mt-3 flex flex-wrap gap-3 p-4 animate-rise ${
        flashing ? 'ring-2 ring-status-confirmed' : ''
      }`}
    >
      <div className="field w-32">
        <label className="label" htmlFor="custom_seconds">
          Sekunden
        </label>
        <input
          id="custom_seconds"
          type="number"
          inputMode="decimal"
          step="0.1"
          autoFocus
          className="input tnum"
          placeholder="z. B. 7.5"
          value={seconds}
          onChange={(e) => setSeconds(e.target.value)}
        />
      </div>
      <div className="field flex-1 min-w-[160px]">
        <label className="label" htmlFor="custom_reason">
          Grund
        </label>
        <select
          id="custom_reason"
          className="input"
          value={reasonCode}
          onChange={(e) => setReasonCode(e.target.value)}
        >
          {reasons.map((r) => (
            <option key={r.code} value={r.code}>
              {r.label}
            </option>
          ))}
          <option value="">— ohne Grund —</option>
        </select>
      </div>
      <button
        type="submit"
        className="btn btn-primary self-end"
        disabled={disabled || submitting || !seconds}
      >
        {submitting ? 'Sende …' : 'Senden'}
      </button>
      <p className="font-mono text-[10px] text-ink-300 w-full">
        Negative Werte (z. B. -2) für eine Korrektur zugunsten der Mannschaft sind erlaubt.
      </p>
    </form>
  );
}
