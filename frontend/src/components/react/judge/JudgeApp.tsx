// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  ApiError,
  apiClient,
  type CatalogueEntry,
  type StationDeciderState,
} from '../../../lib/api';
import {
  canEnterErrors,
  catalogueSubmitBody,
  currentRunLabel,
  resolveSubmitTarget,
  visibleRuns,
  type JudgeContext,
} from '../../../lib/decider';
import {
  countUnsentForAccess,
  isInvalidAccessError,
  isInvalidAccessStatus,
  startJudgePolling,
} from '../../../lib/judge-access';
import {
  cancelEntry,
  chooseRun,
  reconcileFlow,
  startEntry,
  type EntryFlow,
} from '../../../lib/judge-entry';
import {
  EMPTY_SESSION,
  clearJudgeSession,
  disciplineLaneCount,
  laneChoices,
  loadJudgeSession,
  nextLoginStep,
  saveJudgeSession,
  sessionLabel,
  validateSession,
  type JudgeSession,
} from '../../../lib/judge-session';
import {
  buildSubmissionRows,
  pruneLocal,
  type LocalSubmission,
  type SubmissionRow,
} from '../../../lib/judge-submissions';
import { isLineupConfirmed, type LineupRequestBody } from '../../../lib/lineup';
import { catalogueForStation, indexCatalogue } from '../../../lib/penalties';
import {
  attemptSubmit,
  clientId,
  droppedLog,
  isStationSubmit,
  penaltyQueue,
  stationQueueItem,
  usePenaltyQueue,
  type DroppedSubmit,
  type QueuedSubmit,
} from '../../../lib/penalty-queue';
import {
  STATION_KIND_LABELS,
  pickerStations,
  stationRecordsLineup,
} from '../../../lib/stations';
import { Brandmark } from '../Brandmark';
import { EntryDialog, ErrorPad } from './ErrorPad';
import { InvalidAccessScreen } from './InvalidAccessScreen';
import { useJudgeDialog } from './JudgeDialog';
import JudgeLogin from './JudgeLogin';
import { LineupPanel } from './LineupPanel';
import { SubmissionList } from './SubmissionList';
import { CARD } from './ui';

/**
 * Strafrichter app (station token, 2027). Flow:
 *
 * 1. Login: station → (per_station_and_lane with 2 lanes) lane;
 *    persisted per event in localStorage.
 * 2. "Fehler": catalogue buttons of the station grouped SZ / FV. One
 *    visible run → one tap (SZ). Two runs (per_station) → tap error,
 *    then "Bahn 1 · Team" / "Bahn 2 · Team". FV always asks
 *    "Fehlversuch – Lauf ungültig?" (the lane buttons double as the
 *    confirmation).
 * 3. "Eigene Eingaben" with review status + withdraw (pending only).
 * 4. Startlinie only: "Besetzung" tab per current run.
 *
 * Submits go through the offline queue (retries with the same
 * client_id → idempotent server-side).
 */
export interface JudgeAppProps {
  ownerSub: string;
  token: string;
}

const POLL_MS = 4000;
const FLASH_MS = 900;

type Tab = 'fehler' | 'besetzung';

export default function JudgeApp({ ownerSub, token }: JudgeAppProps) {
  const [state, setState] = useState<StationDeciderState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [catalogue, setCatalogue] = useState<CatalogueEntry[] | null>(null);
  const [catalogueFailed, setCatalogueFailed] = useState(false);
  const [session, setSession] = useState<JudgeSession>(EMPTY_SESSION);
  const [hydratedFor, setHydratedFor] = useState<string | null>(null);
  const [flow, setFlow] = useState<EntryFlow<CatalogueEntry>>({ step: 'idle' });
  const [busy, setBusy] = useState(false);
  const [flashCode, setFlashCode] = useState<string | null>(null);
  const [local, setLocal] = useState<LocalSubmission[]>([]);
  const [tab, setTab] = useState<Tab>('fehler');
  /** 401 on any judge request: link rotated/invalid → final screen. */
  const [invalid, setInvalid] = useState(false);
  const invalidRef = useRef(false);
  const markInvalid = useCallback(() => {
    invalidRef.current = true;
    setInvalid(true);
  }, []);
  const { confirm, dialog } = useJudgeDialog();

  const stationRef = useRef(session.station);
  stationRef.current = session.station;

  // ----- data -----------------------------------------------------------

  const loadCatalogue = useCallback(async () => {
    try {
      const c = await apiClient.catalogue.list();
      setCatalogue(c);
      setCatalogueFailed(false);
    } catch {
      setCatalogueFailed(true);
    }
  }, []);

  useEffect(() => {
    void loadCatalogue();
  }, [loadCatalogue]);

  /** Loads the state; throws (401 included) for the caller to handle. */
  const loadState = useCallback(async () => {
    if (invalidRef.current) return;
    const r = await apiClient.deciderStation.state(ownerSub, token, stationRef.current);
    if (invalidRef.current) return;
    setState({ ...r, allowed_stations: pickerStations(r.allowed_stations, r.discipline?.mode) });
    setError(null);
    // The link works → items once blocked by a 401 may be sent again.
    penaltyQueue.unblock(ownerSub, token);
  }, [ownerSub, token]);

  const showLoadError = useCallback((err: unknown) => {
    if (err instanceof ApiError) {
      if (err.status === 404) setError('Aktuell keine aktive Veranstaltung.');
      else setError(err.message);
    } else {
      setError('Verbindung verloren – versuche es erneut …');
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      await loadState();
    } catch (err) {
      if (isInvalidAccessError(err)) markInvalid();
      else showLoadError(err);
    }
  }, [loadState, markInvalid, showLoadError]);

  // Poll the state; a 401 stops polling for good (no retry loop).
  useEffect(
    () =>
      startJudgePolling({
        load: loadState,
        intervalMs: POLL_MS,
        onInvalid: markInvalid,
        onError: showLoadError,
      }),
    [loadState, markInvalid, showLoadError],
  );

  // Retry the catalogue together with the state poll until it loads.
  useEffect(() => {
    if (catalogue || !catalogueFailed || !state) return;
    void loadCatalogue();
  }, [state, catalogue, catalogueFailed, loadCatalogue]);

  const discipline = state?.discipline ?? null;
  const allowed = useMemo(() => state?.allowed_stations ?? [], [state?.allowed_stations]);
  const eventId = state?.event.id ?? null;

  // Hydrate the stored login once per event; afterwards re-validate it
  // whenever the discipline (mode, lane count, judge mode) changes.
  useEffect(() => {
    if (!eventId || allowed.length === 0) return;
    if (hydratedFor !== eventId) {
      setSession(loadJudgeSession(eventId, allowed, discipline));
      setHydratedFor(eventId);
      return;
    }
    setSession((s) => {
      const v = validateSession(s, allowed, discipline);
      return v.station === s.station && v.lane === s.lane ? s : v;
    });
  }, [eventId, allowed, discipline, hydratedFor]);

  // Refetch right away when the station changes (penalties filter).
  useEffect(() => {
    if (session.station) void refresh();
  }, [session.station, refresh]);

  const ctx: JudgeContext = useMemo(
    () => ({
      judgeMode: discipline?.judge_mode ?? null,
      laneCount: disciplineLaneCount(discipline),
      myLane: session.lane,
    }),
    [discipline, session.lane],
  );

  const currentRuns = useMemo(() => state?.current_runs ?? [], [state?.current_runs]);
  const runs = useMemo(() => visibleRuns(currentRuns, ctx), [currentRuns, ctx]);
  const runIds = useMemo(() => runs.map((r) => r.run.id), [runs]);
  const target = resolveSubmitTarget(currentRuns, ctx);

  // Close / shrink an open dialog when the current runs change.
  useEffect(() => {
    setFlow((f) => reconcileFlow(f, runIds));
  }, [runIds]);

  // Leaving Startlinie → back to the error tab.
  useEffect(() => {
    if (!stationRecordsLineup(session.station)) setTab('fehler');
  }, [session.station]);

  // ----- offline queue ----------------------------------------------------

  const handleConfirmed = useCallback((entry: { client_id: string; id: string }) => {
    setLocal((l) =>
      l.map((x) => (x.client_id === entry.client_id ? { ...x, id: entry.id, state: 'sent' } : x)),
    );
  }, []);
  const handleDropped = useCallback((entry: DroppedSubmit) => {
    setLocal((l) => l.filter((x) => x.client_id !== entry.client_id));
    setError(`„${entry.label}" wurde abgelehnt (${entry.reason}). Bitte erneut melden.`);
  }, []);
  const handleUnauthorized = useCallback(
    (entry: QueuedSubmit) => {
      if (entry.owner_sub === ownerSub && entry.token === token) markInvalid();
    },
    [ownerSub, token, markInvalid],
  );
  const queueOptions = useMemo(
    () => ({
      onConfirmed: handleConfirmed,
      onDropped: handleDropped,
      onUnauthorized: handleUnauthorized,
    }),
    [handleConfirmed, handleDropped, handleUnauthorized],
  );
  const { items: queued, online, draining } = usePenaltyQueue(queueOptions);

  // ----- derived lists ------------------------------------------------------

  const catalogueIndex = useMemo(() => indexCatalogue(catalogue), [catalogue]);
  const stationEntries = useMemo(
    () => catalogueForStation(catalogue, session.station),
    [catalogue, session.station],
  );

  const localWithQueue = useMemo<LocalSubmission[]>(() => {
    const fromQueue: LocalSubmission[] = queued
      .filter(isStationSubmit)
      .filter((q) => q.owner_sub === ownerSub && q.token === token && q.body.run_id)
      .map((q) => ({
        client_id: q.client_id,
        id: null,
        code: q.body.code ?? null,
        kind: catalogueIndex.get(q.body.code ?? '')?.kind ?? 'sz',
        label: q.label,
        run_id: q.body.run_id!,
        station: q.body.station,
        at: q.queued_at,
        state: 'sending' as const,
      }));
    const queuedIds = new Set(fromQueue.map((q) => q.client_id));
    // Server rows (same id = client_id) override "sending" once known.
    return [...fromQueue, ...local.filter((l) => !queuedIds.has(l.client_id))];
  }, [queued, local, ownerSub, token, catalogueIndex]);

  const rows = useMemo(
    () =>
      buildSubmissionRows({
        local: localWithQueue,
        server: state?.penalties,
        station: session.station,
        runIds,
        catalogue: catalogueIndex,
      }),
    [localWithQueue, state?.penalties, session.station, runIds, catalogueIndex],
  );

  // Forget local entries of runs that are no longer current.
  useEffect(() => {
    setLocal((l) => {
      const p = pruneLocal(l, runIds);
      return p.length === l.length ? l : p;
    });
  }, [runIds]);

  // ----- actions -------------------------------------------------------------

  const flash = (code: string) => {
    setFlashCode(code);
    window.setTimeout(() => setFlashCode((c) => (c === code ? null : c)), FLASH_MS);
    try {
      if (typeof navigator !== 'undefined' && typeof navigator.vibrate === 'function') {
        navigator.vibrate(60);
      }
    } catch {
      // not supported
    }
  };

  const submit = async (entry: CatalogueEntry, runId: string) => {
    const station = session.station;
    if (!station || busy) return;
    setBusy(true);
    flash(entry.code);
    const cid = clientId();
    const label = entry.label_de;
    const item = stationQueueItem({
      owner_sub: ownerSub,
      token,
      body: catalogueSubmitBody(station, entry, runId, cid),
      label,
      client_id: cid,
    });
    setLocal((l) => [
      { client_id: cid, id: null, code: entry.code, kind: entry.kind, label, run_id: runId, station, at: Date.now(), state: 'sending' },
      ...l,
    ]);
    try {
      const result = await attemptSubmit(item);
      if (result.ok) {
        setLocal((l) =>
          l.map((x) => (x.client_id === cid ? { ...x, id: result.id, state: 'sent' } : x)),
        );
        setError(null);
        void refresh();
      } else if (isInvalidAccessStatus(result.status)) {
        // Keep it (blocked) so it is counted on the "Zugang ungültig" screen.
        penaltyQueue.block(item);
        markInvalid();
      } else if (result.retriable) {
        penaltyQueue.enqueue(item);
        setNotice('Keine Verbindung – die Meldung wird automatisch nachgesendet.');
      } else {
        setLocal((l) => l.filter((x) => x.client_id !== cid));
        droppedLog.push({
          ...item,
          dropped_at: Date.now(),
          reason: result.message,
          http_status: result.status,
        });
        setError(`Meldung abgelehnt (${result.message}).`);
      }
    } finally {
      setBusy(false);
    }
  };

  const onTap = (entry: CatalogueEntry) => {
    setNotice(null);
    const next = startEntry(entry, target);
    if (next.step === 'submit') {
      setFlow({ step: 'idle' });
      void submit(next.entry, next.run_id);
    } else if (next.step === 'blocked') {
      setError(next.message);
    } else {
      setFlow(next);
    }
  };

  const onChoose = (runId: string) => {
    const next = chooseRun(flow, runId);
    if (next.step === 'submit') {
      setFlow({ step: 'idle' });
      void submit(next.entry, next.run_id);
    }
  };

  const withdraw = async (row: SubmissionRow) => {
    if (row.status === 'sending' || !row.id) {
      penaltyQueue.remove(row.client_id);
      setLocal((l) => l.filter((x) => x.client_id !== row.client_id));
      return;
    }
    try {
      await apiClient.deciderStation.withdraw(ownerSub, token, row.id, session.station);
      setLocal((l) => {
        const exists = l.some((x) => x.client_id === row.client_id || x.id === row.id);
        const mark = (x: LocalSubmission): LocalSubmission =>
          x.client_id === row.client_id || x.id === row.id ? { ...x, state: 'withdrawn' } : x;
        return exists
          ? l.map(mark)
          : [
              {
                client_id: row.client_id,
                id: row.id,
                code: null,
                kind: row.kind,
                label: row.label,
                run_id: row.run_id,
                station: session.station!,
                at: row.at,
                state: 'withdrawn',
              },
              ...l,
            ];
      });
      setError(null);
    } catch (err) {
      if (isInvalidAccessError(err)) {
        markInvalid();
        return;
      }
      if (err instanceof ApiError && err.status === 409) {
        setError('Bereits von der Wettkampfleitung bearbeitet – nicht mehr zurückziehbar.');
      } else {
        setError(err instanceof Error ? err.message : 'Zurückziehen fehlgeschlagen.');
      }
    }
    void refresh();
  };

  const confirmLineup = async (runId: string, body: LineupRequestBody) => {
    let run;
    try {
      run = await apiClient.deciderStation.setLineup(
        ownerSub,
        token,
        runId,
        body.runner_ids,
        body.station,
      );
    } catch (err) {
      if (isInvalidAccessError(err)) markInvalid();
      throw err;
    }
    setState((s) =>
      s
        ? {
            ...s,
            current_runs: s.current_runs.map((cr) => (cr.run.id === run.id ? { ...cr, run } : cr)),
          }
        : s,
    );
  };

  const updateSession = (next: JudgeSession) => {
    setSession(next);
    if (eventId) saveJudgeSession(eventId, next);
  };

  const changeLogin = async () => {
    if (!eventId) return;
    const ok = await confirm({
      title: 'Station/Bahn wechseln?',
      message: 'Deine aktuelle Auswahl wird zurückgesetzt, danach wählst du Station und Bahn neu.',
      confirmLabel: 'Wechseln',
    });
    if (!ok) return;
    clearJudgeSession(eventId);
    setSession(EMPTY_SESSION);
    setFlow({ step: 'idle' });
  };

  // ----- render ----------------------------------------------------------------

  if (invalid) {
    return (
      <InvalidAccessScreen
        unsent={countUnsentForAccess(queued, ownerSub, token)}
        onRetry={() => window.location.reload()}
      />
    );
  }

  const loginStep = nextLoginStep(session, discipline);
  const hydrating = !!eventId && allowed.length > 0 && hydratedFor !== eventId;
  if (state && hydrating) {
    return <p className="py-16 text-center text-[16px] text-ink-200">Lade …</p>;
  }
  if (state && loginStep !== 'done' && !state.disabled) {
    return (
      <JudgeLogin
        step={loginStep}
        eventName={state.event.name}
        disciplineName={discipline?.name ?? null}
        allowed={allowed}
        station={session.station}
        lanes={laneChoices(discipline)}
        onStation={(s) => updateSession({ station: s, lane: null })}
        onLane={(lane) => updateSession({ station: session.station, lane })}
        onBack={() => updateSession(EMPTY_SESSION)}
      />
    );
  }

  const isStart = stationRecordsLineup(session.station);
  const openLineups = runs.filter((r) => !isLineupConfirmed(r.run)).length;
  const multi = runs.length > 1;
  const errorsEnabled = canEnterErrors(target) && !busy && stationEntries.length > 0;

  return (
    <div className="mx-auto flex min-h-screen w-full max-w-2xl flex-col">
      <header className="sticky top-0 z-40 border-b-2 border-ink-50 bg-white px-4 py-3">
        <div className="flex items-center gap-3">
          <Brandmark size="sm" />
          <div className="flex min-w-0 flex-1 flex-col leading-tight">
            <span className="text-[17px] font-bold leading-tight text-ink-50">
              {session.station ? sessionLabel(session, (s) => STATION_KIND_LABELS[s], discipline) : 'Strafrichter'}
            </span>
            <span className="truncate text-[13px] text-ink-200">
              {state?.event.name ?? '—'}
              {discipline ? ` · ${discipline.name}` : ''}
            </span>
          </div>
          <ConnectivityPill online={online} pending={queued.length} draining={draining} />
        </div>
        {session.station && (
          <button
            type="button"
            onClick={() => void changeLogin()}
            className="mt-2 min-h-[40px] rounded-lg border-2 border-ink-300 px-3 text-[13px] font-semibold text-ink-100"
          >
            Station/Bahn wechseln
          </button>
        )}
      </header>

      <main className="flex-1 px-4 pb-16 pt-4">
        {error && (
          <div
            className="mb-4 rounded-xl border-2 border-red-700 bg-red-50 px-4 py-3 text-[15px] font-semibold text-red-900"
            role="alert"
          >
            {error}
          </div>
        )}
        {notice && !error && (
          <div
            className="mb-4 rounded-xl border-2 border-amber-600 bg-amber-50 px-4 py-3 text-[15px] font-semibold text-amber-900"
            role="status"
          >
            {notice}
          </div>
        )}

        {!state ? (
          <p className="py-16 text-center text-[16px] text-ink-200">Lade …</p>
        ) : state.disabled ? (
          <div className={`${CARD} px-5 py-10 text-center`}>
            <p className="text-[20px] font-bold text-ink-50">
              {discipline ? 'Wettkampf läuft offline' : 'Kein Wettkampf aktiv'}
            </p>
            <p className="mt-2 text-[15px] text-ink-100">
              {discipline
                ? 'Strafrichter sind in diesem Modus deaktiviert. Fehler werden direkt am Wettkampf-Laptop erfasst.'
                : 'Sobald die Wettkampfleitung einen Wettkampf startet, geht es hier weiter.'}
            </p>
          </div>
        ) : (
          <>
            <RunsBanner runs={runs} target={target} />

            {isStart && (
              <div className="mt-4 grid grid-cols-2 gap-2" role="tablist">
                <TabButton active={tab === 'fehler'} onClick={() => setTab('fehler')}>
                  Fehler
                </TabButton>
                <TabButton active={tab === 'besetzung'} onClick={() => setTab('besetzung')}>
                  Besetzung
                  {openLineups > 0 && (
                    <span className="ml-2 rounded-full bg-amber-400 px-2 text-[13px] font-black text-black">
                      {openLineups}
                    </span>
                  )}
                </TabButton>
              </div>
            )}

            {isStart && tab === 'besetzung' ? (
              <section className="mt-4">
                <LineupPanel runs={runs} onConfirm={confirmLineup} />
              </section>
            ) : (
              <>
                <section className="mt-5">
                  {catalogue === null && !catalogueFailed && (
                    <p className="text-[15px] text-ink-200">Fehlerkatalog wird geladen …</p>
                  )}
                  {catalogueFailed && !catalogue && (
                    <div className="rounded-xl border-2 border-red-700 bg-red-50 p-4">
                      <p className="text-[15px] font-semibold text-red-900">
                        Fehlerkatalog konnte nicht geladen werden.
                      </p>
                      <button
                        type="button"
                        className="mt-3 min-h-[48px] rounded-lg border-2 border-ink-50 bg-white px-4 font-semibold"
                        onClick={() => void loadCatalogue()}
                      >
                        Erneut versuchen
                      </button>
                    </div>
                  )}
                  {catalogue && stationEntries.length === 0 && (
                    <p className="text-[15px] text-ink-200">
                      Für diese Station sind keine Fehler im Katalog hinterlegt.
                    </p>
                  )}
                  {stationEntries.length > 0 && (
                    <>
                      {multi && (
                        <p className="mb-3 text-[14px] font-semibold text-ink-100">
                          Zwei Läufe aktiv: nach dem Fehler die Bahn wählen.
                        </p>
                      )}
                      <ErrorPad
                        entries={stationEntries}
                        disabled={!errorsEnabled}
                        flashCode={flashCode}
                        onTap={onTap}
                      />
                    </>
                  )}
                </section>

                <section className="mt-8">
                  <h2 className="mb-2 text-[14px] font-bold uppercase tracking-wide text-ink-100">
                    Eigene Eingaben
                  </h2>
                  <SubmissionList
                    rows={rows}
                    runLabel={(id) => {
                      if (!multi) return null;
                      const cr = runs.find((r) => r.run.id === id);
                      return cr ? currentRunLabel(cr) : null;
                    }}
                    onWithdraw={(r) => void withdraw(r)}
                  />
                </section>
              </>
            )}
          </>
        )}
      </main>

      <EntryDialog
        flow={flow}
        busy={busy}
        onChoose={onChoose}
        onCancel={() => setFlow(cancelEntry() as EntryFlow<CatalogueEntry>)}
      />
      {dialog}
    </div>
  );
}

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={`flex min-h-[56px] items-center justify-center rounded-xl border-2 border-ink-50 text-[17px] font-bold ${
        active ? 'bg-ink-50 text-white' : 'bg-white text-ink-50'
      }`}
    >
      {children}
    </button>
  );
}

function RunsBanner({
  runs,
  target,
}: {
  runs: StationDeciderState['current_runs'];
  target: ReturnType<typeof resolveSubmitTarget>;
}) {
  if (runs.length === 0) {
    const msg = !target.ok ? target.message : 'Kein Lauf aktiv.';
    return (
      <div className={`${CARD} px-5 py-8 text-center`}>
        <p className="text-[13px] font-bold uppercase tracking-wide text-ink-200">Bereitschaft</p>
        <p className="mt-1 text-[19px] font-bold text-ink-50">{msg}</p>
      </div>
    );
  }
  return (
    <div className={`grid gap-2 ${runs.length > 1 ? 'grid-cols-2' : 'grid-cols-1'}`}>
      {runs.map((cr) => (
        <div
          key={cr.run.id}
          className={`${CARD} flex items-center gap-2 py-3 ${runs.length > 1 ? 'px-2' : 'px-3'}`}
        >
          <span
            className={`grid shrink-0 place-items-center rounded-xl bg-ink-50 font-black text-white tnum ${
              runs.length > 1 ? 'h-10 w-10 text-[22px]' : 'h-14 w-14 text-[28px]'
            }`}
          >
            {cr.run.lane ?? '–'}
          </span>
          <div className="flex min-w-0 flex-col leading-tight">
            <span className="text-[12px] font-bold uppercase tracking-wide text-ink-200">
              {cr.run.lane != null ? `Bahn ${cr.run.lane} · ` : ''}Lauf {cr.run.run_number}
            </span>
            <span className="hyphens-auto break-words text-[17px] font-bold text-ink-50" lang="de">
              {cr.team.name}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
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
  const base = 'shrink-0 rounded-full border-2 px-3 py-1 text-[13px] font-bold';
  if (!online) {
    return (
      <span className={`${base} border-red-800 bg-red-700 text-white`} role="status">
        Offline{pending > 0 ? ` (${pending})` : ''}
      </span>
    );
  }
  if (pending > 0) {
    return (
      <span className={`${base} border-amber-700 bg-amber-400 text-black`} role="status">
        {draining ? 'Sendet …' : 'Wartend'} ({pending})
      </span>
    );
  }
  return (
    <span className={`${base} border-green-800 bg-green-700 text-white`} role="status">
      Live
    </span>
  );
}
