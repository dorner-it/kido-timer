// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import { ApiError, apiClient, type DeciderState } from '../../lib/api';
import { Brandmark } from './Brandmark';

export interface DeciderPadProps {
  ownerSub: string;
  slot: string;
  token: string;
}

const PRESETS = [
  { label: '+ 5 s', deltaMs: 5_000, reason: null },
  { label: '+ 10 s', deltaMs: 10_000, reason: null },
  { label: '+ 20 s', deltaMs: 20_000, reason: null },
  { label: '+ 30 s', deltaMs: 30_000, reason: null },
  { label: '+ 1:00', deltaMs: 60_000, reason: null },
  { label: 'Disqualifikation', deltaMs: 999_000, reason: 'DSQ' },
];

export default function DeciderPad({ ownerSub, slot: slotStr, token }: DeciderPadProps) {
  const slot = Number(slotStr);
  const [state, setState] = useState<DeciderState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [recent, setRecent] = useState<{ id: string; label: string; at: number }[]>([]);
  const [busy, setBusy] = useState(false);

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

  const submit = async (label: string, deltaMs: number, reason: string | null) => {
    if (busy) return;
    if (!state?.current_run) {
      setError('Aktuell läuft kein Lauf.');
      return;
    }
    setBusy(true);
    try {
      const created = await apiClient.decider.submit(ownerSub, slot, token, {
        delta_ms: deltaMs,
        reason_code: reason,
      });
      setRecent((r) => [{ id: created.id, label, at: Date.now() }, ...r].slice(0, 5));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Senden fehlgeschlagen');
    } finally {
      setBusy(false);
    }
  };

  const withdraw = async (id: string) => {
    try {
      await apiClient.decider.withdraw(ownerSub, slot, token, id);
      setRecent((r) => r.filter((x) => x.id !== id));
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
          />
        ) : (
          <div className="surface flex flex-col items-center gap-2 py-12 animate-rise">
            <span className="label">Bereitschaft</span>
            <span className="font-display text-[18px] text-ink-100">
              Kein Lauf aktiv
            </span>
          </div>
        )}

        {!state?.disabled && (
          <section className="mt-8">
            <span className="label">Strafen senden</span>
            <div className="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-3 stagger-rise">
              {PRESETS.map((p) => (
                <button
                  key={p.label}
                  type="button"
                  disabled={!state?.current_run || busy}
                  onClick={() => submit(p.label, p.deltaMs, p.reason)}
                  className="surface group relative flex flex-col items-center justify-center gap-1 py-7 transition active:scale-[0.97] disabled:opacity-40"
                >
                  <span className="font-stencil text-[28px] font-bold text-ink-50 leading-none">
                    {p.label}
                  </span>
                  {p.reason && <span className="label">{p.reason}</span>}
                </button>
              ))}
            </div>
          </section>
        )}

        {!state?.disabled && (
        <section className="mt-10">
          <span className="label">Eigene Eingaben</span>
          {recent.length === 0 ? (
            <p className="mt-2 font-mono text-[12px] text-ink-200">Noch nichts gesendet.</p>
          ) : (
            <ul className="mt-3 flex flex-col divide-y divide-[color:var(--hairline-color)]">
              {recent.map((r) => (
                <li key={r.id} className="flex items-center justify-between gap-3 py-3">
                  <span className="font-display text-[15px] text-ink-50">{r.label}</span>
                  <span className="label">{relativeTime(r.at)}</span>
                  <button type="button" className="btn btn-danger" onClick={() => withdraw(r.id)}>
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
}) {
  const { startNumber, teamName, runnerName, runNumber } = props;
  return (
    <div className="surface relative overflow-hidden p-7 animate-rise">
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
      <div className="scanlines pointer-events-none absolute inset-0 opacity-50" />
    </div>
  );
}

function relativeTime(at: number): string {
  const sec = Math.max(0, Math.floor((Date.now() - at) / 1000));
  if (sec < 60) return `${sec}s`;
  return `${Math.floor(sec / 60)}m`;
}
