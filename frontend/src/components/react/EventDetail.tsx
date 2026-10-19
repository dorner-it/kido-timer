// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import {
  TIMER_MODE_LABELS,
  apiClient,
  type Discipline,
  type DisciplineSaveResponse,
  type EventDetail as EventDetailData,
  type JudgeMode,
  type SyncMode,
  type TimerMode,
} from '../../lib/api';
import { formatDateDE } from '../../lib/format';
import {
  JUDGE_MODES,
  JUDGE_MODE_LABELS,
  allowedLaneCounts,
  defaultLaneCount,
  effectiveLaneCount,
  isLaneCountEditable,
  laneCountLabel,
} from '../../lib/lanes';
import { disciplineFormSchema, fieldErrors } from '../../lib/schemas';
import { QrIconButton } from './QrCodeModal';
import TeamRoster from './TeamRoster';
import PenaltiesPanel from './PenaltiesPanel';
import EntriesPanel from './EntriesPanel';
import RunsPanel from './RunsPanel';
import { useDialog } from './useDialog';

type Tab = 'disciplines' | 'roster' | 'entries' | 'runs' | 'penalties' | 'deciders';

interface Props {
  id: string;
}

export default function EventDetail({ id }: Props) {
  const [data, setData] = useState<EventDetailData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>('disciplines');
  const { confirm, dialog } = useDialog();

  useEffect(() => {
    refresh();
  }, [id]);

  const refresh = async () => {
    try {
      setData(await apiClient.events.detail(id));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unbekannter Fehler');
    }
  };

  const activate = async () => {
    setError(null);
    try {
      await apiClient.events.activate(id);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Aktivierung fehlgeschlagen');
    }
  };

  const removeEvent = async () => {
    const ok = await confirm({
      title: 'Veranstaltung löschen?',
      message: [
        `„${data?.event.name ?? 'Diese Veranstaltung'}“ wird endgültig gelöscht.`,
        'Alle Wettkämpfe, Mannschaften, Läufer, Läufe und Strafen werden mit gelöscht.',
      ],
      confirmLabel: 'Endgültig löschen',
      tone: 'danger',
    });
    if (!ok) return;
    try {
      await apiClient.events.remove(id);
      window.location.href = '/events';
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Löschen fehlgeschlagen');
    }
  };

  if (error) {
    return (
      <main className="mx-auto w-full max-w-6xl px-7 py-10">
        <p className="surface px-5 py-3 text-[13px] text-status-unknown">{error}</p>
      </main>
    );
  }
  if (!data) {
    return (
      <main className="mx-auto w-full max-w-6xl px-7 py-10">
        <p className="label">Lade …</p>
      </main>
    );
  }

  const { event, disciplines } = data;

  return (
    <main className="mx-auto w-full max-w-6xl px-7 py-10">
      <header className="mb-6 flex items-end justify-between gap-6">
        <div className="flex flex-col gap-2">
          <a href="/events" className="label hover:text-ink-50">
            ← Veranstaltungen
          </a>
          <h1 className="font-stencil text-[40px] font-bold leading-none tracking-tight text-ink-50">
            {event.name}
          </h1>
          <p className="font-mono text-[12px] text-ink-200">
            {formatDateDE(event.date)}
            {event.location ? ` · ${event.location}` : ''}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {event.is_active ? (
            <span className="pill bg-status-running/15 text-status-running">
              <span className="h-1.5 w-1.5 rounded-full bg-current" />
              Aktiv
            </span>
          ) : (
            <button type="button" onClick={activate} className="btn btn-primary">
              Veranstaltung aktivieren
            </button>
          )}
          <button type="button" onClick={removeEvent} className="btn btn-danger">
            Löschen
          </button>
        </div>
      </header>

      <nav className="mb-6 flex gap-2 border-b border-[color:var(--hairline-color)]">
        <TabButton active={tab === 'disciplines'} onClick={() => setTab('disciplines')}>
          Wettkämpfe ({disciplines.length})
        </TabButton>
        <TabButton active={tab === 'roster'} onClick={() => setTab('roster')}>
          Mannschaften & Läufer
        </TabButton>
        <TabButton active={tab === 'entries'} onClick={() => setTab('entries')}>
          Meldungen
        </TabButton>
        <TabButton active={tab === 'runs'} onClick={() => setTab('runs')}>
          Läufe & Prüfung
        </TabButton>
        <TabButton active={tab === 'penalties'} onClick={() => setTab('penalties')}>
          Strafen
        </TabButton>
        <TabButton active={tab === 'deciders'} onClick={() => setTab('deciders')}>
          Strafrichter
        </TabButton>
      </nav>

      {tab === 'disciplines' && (
        <DisciplinesPanel data={data} onChange={refresh} />
      )}
      {tab === 'roster' && <TeamRoster data={data} onChange={refresh} />}
      {tab === 'entries' && <EntriesPanel data={data} onChange={refresh} />}
      {tab === 'runs' && <RunsPanel data={data} onChange={refresh} />}
      {tab === 'penalties' && <PenaltiesPanel data={data} onChange={refresh} />}
      {tab === 'deciders' && <DecidersPanel data={data} onChange={refresh} />}
      {dialog}
    </main>
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
      onClick={onClick}
      className={[
        'border-b-2 px-4 py-2 font-mono text-[11px] uppercase tracking-[0.18em] transition',
        active
          ? 'border-signal text-ink-50'
          : 'border-transparent text-ink-200 hover:text-ink-100',
      ].join(' ')}
    >
      {children}
    </button>
  );
}

// ===== Disciplines =====

function DisciplinesPanel({
  data,
  onChange,
}: {
  data: EventDetailData;
  onChange: () => void;
}) {
  const { event, disciplines, teams } = data;
  const [creating, setCreating] = useState(false);
  const [conflictWarning, setConflictWarning] = useState<string | null>(null);
  const { confirm, dialog } = useDialog();

  const showConflicts = (saved: DisciplineSaveResponse) => {
    const ids = saved.lane_conflicts ?? [];
    if (ids.length === 0) {
      setConflictWarning(null);
      return;
    }
    const names = ids.map((id) => teams.find((t) => t.id === id)?.name ?? id);
    setConflictWarning(
      `„${saved.name}" hat jetzt ${laneCountLabel(saved.lane_count)}. Folgende Meldungen ` +
        `liegen auf einer höheren Bahn und sollten unter „Meldungen" angepasst werden: ` +
        names.join(', '),
    );
  };

  const sorted = [...disciplines].sort((a, b) => a.position - b.position);

  return (
    <section className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <p className="font-mono text-[12px] text-ink-200">
          Lege pro Wettkampf eine Disziplin an (Löschangriff oder Gruppenstaffette).
          Jeder Wettkampf hat seinen eigenen Modus, seine Bahnenzahl, die
          Wertungsrichter-Einteilung und den Live/Offline-Status.
        </p>
        <button
          type="button"
          onClick={() => setCreating((v) => !v)}
          className="btn btn-primary"
        >
          {creating ? 'Abbrechen' : '+ Neuer Wettkampf'}
        </button>
      </div>

      {conflictWarning && (
        <div
          className="surface flex items-start justify-between gap-3 px-5 py-3 text-[13px] text-status-armed"
          role="alert"
        >
          <span>⚠ {conflictWarning}</span>
          <button
            type="button"
            className="text-ink-300 hover:text-ink-50"
            aria-label="Hinweis schließen"
            onClick={() => setConflictWarning(null)}
          >
            ✕
          </button>
        </div>
      )}

      {creating && (
        <DisciplineForm
          eventId={event.id}
          nextPosition={(sorted[sorted.length - 1]?.position ?? 0) + 1}
          onDone={(saved) => {
            setCreating(false);
            showConflicts(saved);
            void onChange();
          }}
        />
      )}

      {sorted.length === 0 ? (
        <p className="surface px-5 py-4 text-[13px] text-ink-200">
          Noch keine Wettkämpfe. Lege oben den ersten an.
        </p>
      ) : (
        <ul className="grid grid-cols-1 gap-3 md:grid-cols-2">
          {sorted.map((d) => (
            <li key={d.id}>
              <DisciplineCard
                event_id={event.id}
                discipline={d}
                isCurrent={event.current_discipline_id === d.id}
                onSetCurrent={async () => {
                  await apiClient.events.setCurrent(event.id, {
                    discipline_id: d.id,
                    run_id: null,
                  });
                  onChange();
                }}
                onSetSyncMode={async (mode) => {
                  await apiClient.disciplines.setSyncMode(event.id, d.id, mode);
                  onChange();
                }}
                onRemove={async () => {
                  const ok = await confirm({
                    title: 'Wettkampf löschen?',
                    message: `„${d.name}“ wird gelöscht.`,
                    confirmLabel: 'Löschen',
                    tone: 'danger',
                  });
                  if (!ok) return;
                  await apiClient.disciplines.remove(event.id, d.id);
                  onChange();
                }}
                onSaved={(saved) => {
                  showConflicts(saved);
                  onChange();
                }}
              />
            </li>
          ))}
        </ul>
      )}
      {dialog}
    </section>
  );
}

function DisciplineCard({
  event_id,
  discipline,
  isCurrent,
  onSetCurrent,
  onSetSyncMode,
  onRemove,
  onSaved,
}: {
  event_id: string;
  discipline: Discipline;
  isCurrent: boolean;
  onSetCurrent: () => Promise<void>;
  onSetSyncMode: (mode: SyncMode) => Promise<void>;
  onRemove: () => Promise<void>;
  onSaved: (saved: DisciplineSaveResponse) => void;
}) {
  const [exporting, setExporting] = useState(false);
  const [editing, setEditing] = useState(false);
  const laneCount = effectiveLaneCount(discipline);

  const exportFile = async () => {
    setExporting(true);
    try {
      const { blob, filename } = await apiClient.disciplines.exportFile(
        event_id,
        discipline.id,
      );
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="surface flex flex-col gap-3 p-5">
      <div className="flex items-start justify-between gap-3">
        <div className="flex flex-col gap-1">
          <span className="label">{TIMER_MODE_LABELS[discipline.mode]}</span>
          <h3 className="font-display text-[18px] font-semibold text-ink-50">
            {discipline.name}
          </h3>
          <span className="font-mono text-[11px] text-ink-200">
            {laneCountLabel(laneCount)} · {JUDGE_MODE_LABELS[discipline.judge_mode ?? 'per_station']}
          </span>
        </div>
        {isCurrent && (
          <span className="pill bg-signal/15 text-signal-glow">Auf der Bahn</span>
        )}
      </div>

      {editing && (
        <DisciplineForm
          eventId={event_id}
          existing={discipline}
          nextPosition={discipline.position}
          onDone={(saved) => {
            setEditing(false);
            onSaved(saved);
          }}
          onCancel={() => setEditing(false)}
        />
      )}

      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          className={discipline.sync_mode === 'live' ? 'btn btn-confirm' : 'btn'}
          onClick={() =>
            onSetSyncMode(discipline.sync_mode === 'live' ? 'offline' : 'live')
          }
          title="Umschalten Live ↔ Offline"
        >
          {discipline.sync_mode === 'live' ? '● Live' : '○ Offline'}
        </button>
        {!isCurrent && (
          <button type="button" className="btn" onClick={onSetCurrent}>
            Aktiv schalten
          </button>
        )}
        <button
          type="button"
          className="btn"
          onClick={exportFile}
          disabled={exporting}
          title="Signierte .kido-Datei herunterladen"
        >
          {exporting ? '⏳' : '⇩'} .kido
        </button>
        <a className="btn" href={`/events/${event_id}/disciplines/${discipline.id}/results`}>
          Ergebnisse
        </a>
        <button type="button" className="btn" onClick={() => setEditing((v) => !v)}>
          {editing ? 'Schließen' : 'Bearbeiten'}
        </button>
        <button type="button" className="btn btn-danger ml-auto" onClick={onRemove}>
          Löschen
        </button>
      </div>
    </div>
  );
}

const TIMER_MODES: TimerMode[] = ['gruppenstaffette', 'loeschangriff'];

function DisciplineForm({
  eventId,
  nextPosition,
  existing,
  onDone,
  onCancel,
}: {
  eventId: string;
  nextPosition: number;
  /** Set → edit mode (PUT), otherwise create (POST). */
  existing?: Discipline;
  onDone: (saved: DisciplineSaveResponse) => void;
  onCancel?: () => void;
}) {
  const [name, setName] = useState(existing?.name ?? '');
  const [mode, setMode] = useState<TimerMode>(existing?.mode ?? 'loeschangriff');
  const [laneCount, setLaneCount] = useState<number>(
    existing ? effectiveLaneCount(existing) : defaultLaneCount('loeschangriff'),
  );
  const [judgeMode, setJudgeMode] = useState<JudgeMode>(
    existing?.judge_mode ?? 'per_station',
  );
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const idp = existing ? `d-${existing.id}` : 'd-new';

  const changeMode = (m: TimerMode) => {
    setMode(m);
    // Keep the lane count if still valid for the new mode, else default.
    if (!allowedLaneCounts(m).includes(laneCount)) setLaneCount(defaultLaneCount(m));
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const parsed = disciplineFormSchema.safeParse({
      name,
      mode,
      position: nextPosition,
      lane_count: laneCount,
      judge_mode: judgeMode,
    });
    if (!parsed.success) {
      setErrors(fieldErrors(parsed.error));
      return;
    }
    setErrors({});
    setBusy(true);
    setErr(null);
    try {
      const saved = existing
        ? await apiClient.disciplines.update(eventId, existing.id, parsed.data)
        : await apiClient.disciplines.create(eventId, parsed.data);
      onDone(saved);
    } catch (e) {
      setErr(e instanceof Error ? e.message : 'Fehler');
    } finally {
      setBusy(false);
    }
  };

  const laneChoices = allowedLaneCounts(mode);

  return (
    <form
      onSubmit={submit}
      className="surface grid grid-cols-1 gap-4 p-5 md:grid-cols-3 animate-rise"
    >
      <div className="field md:col-span-2">
        <label className="label" htmlFor={`${idp}-name`}>
          Name
        </label>
        <input
          id={`${idp}-name`}
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="z.B. Löschangriff Männer, Gruppenstaffette Jugend"
          autoFocus
        />
        {errors.name && <p className="text-[12px] text-status-unknown">{errors.name}</p>}
      </div>
      <div className="field">
        <label className="label" htmlFor={`${idp}-mode`}>
          Timer-Modus
        </label>
        <select
          id={`${idp}-mode`}
          className="input"
          value={mode}
          onChange={(e) => changeMode(e.target.value as TimerMode)}
        >
          {TIMER_MODES.map((m) => (
            <option key={m} value={m}>
              {TIMER_MODE_LABELS[m]}
            </option>
          ))}
        </select>
      </div>
      <div className="field">
        <label className="label" htmlFor={`${idp}-lanes`}>
          Bahnen
        </label>
        <select
          id={`${idp}-lanes`}
          className="input"
          value={String(laneCount)}
          disabled={!isLaneCountEditable(mode)}
          title={isLaneCountEditable(mode) ? undefined : 'Löschangriff hat immer 4 Bahnen.'}
          onChange={(e) => setLaneCount(parseInt(e.target.value, 10))}
        >
          {laneChoices.map((n) => (
            <option key={n} value={String(n)}>
              {laneCountLabel(n)}
            </option>
          ))}
        </select>
        {errors.lane_count && (
          <p className="text-[12px] text-status-unknown">{errors.lane_count}</p>
        )}
      </div>
      <div className="field md:col-span-2">
        <label className="label" htmlFor={`${idp}-judges`}>
          Wertungsrichter
        </label>
        <select
          id={`${idp}-judges`}
          className="input"
          value={judgeMode}
          onChange={(e) => setJudgeMode(e.target.value as JudgeMode)}
        >
          {JUDGE_MODES.map((j) => (
            <option key={j} value={j}>
              {JUDGE_MODE_LABELS[j]}
            </option>
          ))}
        </select>
      </div>
      {err && (
        <p className="md:col-span-3 text-[12px] text-status-unknown">{err}</p>
      )}
      <div className="md:col-span-3 flex justify-end gap-2">
        {onCancel && (
          <button type="button" className="btn" onClick={onCancel}>
            Abbrechen
          </button>
        )}
        <button type="submit" className="btn btn-primary" disabled={busy}>
          {busy ? 'Speichere …' : existing ? 'Speichern' : 'Wettkampf anlegen'}
        </button>
      </div>
    </form>
  );
}

// ===== Deciders =====

function DecidersPanel({
  data,
  onChange,
}: {
  data: EventDetailData;
  onChange: () => void;
}) {
  const { event } = data;
  const [busy, setBusy] = useState(false);
  const { confirm, dialog } = useDialog();
  const origin = typeof window !== 'undefined' ? window.location.origin : '';
  // 2027 judge app: one shared station token, station picked in the app.
  const stationUrl = event.station_token
    ? `${origin}/decider/${event.owner_sub}/${event.station_token}`
    : '';

  const rotate = async () => {
    if (busy) return;
    const ok = await confirm({
      title: 'Decider-Codes neu generieren?',
      message: 'Bisherige QR-Codes werden ungültig und müssen neu verteilt werden.',
      confirmLabel: 'Neu generieren',
      tone: 'danger',
    });
    if (!ok) return;
    setBusy(true);
    try {
      await apiClient.events.rotateDeciderTokens(event.id);
      onChange();
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="flex flex-col gap-4">
      <p className="font-mono text-[12px] text-ink-200">
        Vier QR-Codes — einer pro Strafrichter-Slot. Verteile sie am Ort und
        rotiere bei Verlust.
      </p>
      {stationUrl && (
        <div className="surface flex flex-col gap-2 p-5">
          <div className="flex items-baseline justify-between">
            <span className="label">Strafrichter-Zugang (Stationen)</span>
          </div>
          <code className="block break-all rounded border border-[color:var(--hairline-color)] bg-[color:var(--soft-fill)] px-3 py-2 font-mono text-[11px] text-ink-100">
            {stationUrl}
          </code>
          <div className="flex gap-2">
            <button
              type="button"
              className="btn"
              onClick={() => {
                void navigator.clipboard.writeText(stationUrl);
              }}
            >
              Kopieren
            </button>
            <a href={stationUrl} target="_blank" rel="noreferrer" className="btn">
              Öffnen
            </a>
            <QrIconButton title="Strafrichter-Zugang (Stationen)" url={stationUrl} />
          </div>
        </div>
      )}
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {event.decider_tokens.map((token, i) => {
          const slot = i + 1;
          const url = `${origin}/decider/${event.owner_sub}/${slot}/${token}`;
          return (
            <div key={slot} className="surface flex flex-col gap-2 p-5">
              <div className="flex items-baseline justify-between">
                <span className="label">Strafrichter</span>
                <span className="font-stencil text-[28px] font-bold leading-none tracking-tight text-ink-50">
                  B{slot}
                </span>
              </div>
              <code className="block break-all rounded border border-[color:var(--hairline-color)] bg-[color:var(--soft-fill)] px-3 py-2 font-mono text-[11px] text-ink-100">
                {url}
              </code>
              <div className="flex gap-2">
                <button
                  type="button"
                  className="btn"
                  onClick={() => {
                    void navigator.clipboard.writeText(url);
                  }}
                >
                  Kopieren
                </button>
                <a
                  href={url}
                  target="_blank"
                  rel="noreferrer"
                  className="btn"
                >
                  Öffnen
                </a>
                <QrIconButton title={`Strafrichter B${slot}`} url={url} />
              </div>
            </div>
          );
        })}
      </div>
      <div className="flex justify-end">
        <button type="button" className="btn btn-danger" onClick={rotate} disabled={busy}>
          {busy ? 'Rotiere …' : 'Codes neu generieren'}
        </button>
      </div>
      {dialog}
    </section>
  );
}
