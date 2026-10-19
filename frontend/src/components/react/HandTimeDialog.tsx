// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useMemo, useState } from 'react';

import { apiClient, type Run } from '../../lib/api';
import {
  clearHandTimesBody,
  formatTimeInput,
  handTimesBody,
  isHandTime,
  validateHandTimes,
} from '../../lib/hand-time';
import { Modal } from './Modal';

interface Props {
  eventId: string;
  disciplineId: string;
  run: Run;
  title: string;
  onClose: () => void;
  onSaved: (run: Run) => void;
}

/**
 * Hand-time entry: only when the light barrier failed. Two stopwatch
 * readings → mean rounded half-up to 1/100 s (same formula as the
 * backend / kido-core), stored as "Handzeit".
 */
export default function HandTimeDialog({ eventId, disciplineId, run, title, onClose, onSaved }: Props) {
  const [t1, setT1] = useState(formatTimeInput(run.hand_time_1_ms));
  const [t2, setT2] = useState(formatTimeInput(run.hand_time_2_ms));
  const [touched, setTouched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const v = useMemo(() => validateHandTimes(t1, t2), [t1, t2]);
  const hand = isHandTime(run);

  const save = async () => {
    setTouched(true);
    if (!v.ok) return;
    setBusy(true);
    setErr(null);
    try {
      onSaved(await apiClient.runs.setHandTimes(eventId, disciplineId, run.id, handTimesBody(v.t1, v.t2)));
      onClose();
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Speichern fehlgeschlagen');
    } finally {
      setBusy(false);
    }
  };

  const clear = async () => {
    setBusy(true);
    setErr(null);
    try {
      onSaved(await apiClient.runs.setHandTimes(eventId, disciplineId, run.id, clearHandTimesBody()));
      onClose();
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Entfernen fehlgeschlagen');
    } finally {
      setBusy(false);
    }
  };

  const showErr = (field: 't1' | 't2', value: string) =>
    !v.ok && (touched || value.trim() !== '') ? v.errors[field] : undefined;

  return (
    <Modal
      title={`Handzeit – ${title}`}
      onClose={onClose}
      footer={
        <>
          {hand && (
            <button type="button" className="btn btn-danger mr-auto" onClick={clear} disabled={busy}>
              Handzeit entfernen
            </button>
          )}
          <button type="button" className="btn" onClick={onClose} disabled={busy}>
            Abbrechen
          </button>
          <button type="button" className="btn btn-primary" onClick={save} disabled={busy || !v.ok}>
            {busy ? 'Speichere …' : 'Handzeit speichern'}
          </button>
        </>
      }
    >
      <p className="text-[12px] text-ink-200">
        Nur eintragen, wenn die Lichtschranke ausgefallen ist. Ergebnis = Mittelwert der beiden
        Stoppuhren, auf 1/100 s gerundet. Format <code>m:ss,hh</code> oder <code>ss,hh</code>.
      </p>
      {!hand && run.original_time_ms != null && (
        <p className="font-mono text-[12px] text-ink-200">
          Lichtschranke bisher: {formatTimeInput(run.original_time_ms)}
        </p>
      )}
      <form
        className="grid grid-cols-2 gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <div className="field">
          <label className="label" htmlFor={`ht1-${run.id}`}>
            Stoppuhr 1
          </label>
          <input
            id={`ht1-${run.id}`}
            className="input tnum"
            inputMode="decimal"
            placeholder="47,23"
            value={t1}
            onChange={(e) => setT1(e.target.value)}
            autoFocus
          />
          {showErr('t1', t1) && <p className="text-[12px] text-status-unknown">{showErr('t1', t1)}</p>}
        </div>
        <div className="field">
          <label className="label" htmlFor={`ht2-${run.id}`}>
            Stoppuhr 2
          </label>
          <input
            id={`ht2-${run.id}`}
            className="input tnum"
            inputMode="decimal"
            placeholder="47,25"
            value={t2}
            onChange={(e) => setT2(e.target.value)}
          />
          {showErr('t2', t2) && <p className="text-[12px] text-status-unknown">{showErr('t2', t2)}</p>}
        </div>
        <button type="submit" className="hidden" aria-hidden="true" tabIndex={-1} />
      </form>
      <div className="flex items-baseline justify-between rounded-lg border border-[color:var(--hairline-color)] bg-[color:var(--soft-fill)] px-4 py-3">
        <span className="label">Mittelwert (Handzeit)</span>
        <span className="font-stencil text-[28px] font-bold tnum text-ink-50">
          {v.ok ? formatTimeInput(v.mean) : '—'}
        </span>
      </div>
      {v.ok && v.warning && <p className="text-[12px] text-status-armed">⚠ {v.warning}</p>}
      {hand && (
        <p className="text-[11px] text-ink-300">
          „Handzeit entfernen" stellt auf Lichtschranke zurück; die gespeicherte Zeit bleibt, bis
          der Zeitnehmer eine neue Zeit überträgt.
        </p>
      )}
      {err && <p className="text-[12px] text-status-unknown">{err}</p>}
    </Modal>
  );
}
