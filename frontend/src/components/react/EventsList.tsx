// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import { z } from 'zod';

import { apiClient, type Event } from '../../lib/api';
import { formatDateDE } from '../../lib/format';

const schema = z.object({
  name: z.string().trim().min(1, 'Name erforderlich'),
  date: z.string().min(1, 'Datum erforderlich'),
  location: z.string().trim().optional(),
});
type FormValues = z.infer<typeof schema>;

export default function EventsList() {
  const [events, setEvents] = useState<Event[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    refresh();
  }, []);

  const refresh = async () => {
    try {
      const list = await apiClient.events.list();
      list.sort((a, b) => (a.date < b.date ? 1 : -1));
      setEvents(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unbekannter Fehler');
    }
  };

  const form = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      name: '',
      date: new Date().toISOString().slice(0, 10),
      location: '',
    },
  });

  const onSubmit = async (values: FormValues) => {
    try {
      await apiClient.events.create({
        name: values.name,
        date: values.date,
        location: values.location?.trim() ? values.location.trim() : null,
      });
      form.reset({ ...values, name: '', location: '' });
      setCreating(false);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Erstellen fehlgeschlagen');
    }
  };

  return (
    <main className="mx-auto w-full max-w-6xl px-7 py-10">
      <div className="mb-8 flex items-end justify-between gap-6">
        <div className="flex flex-col gap-2">
          <span className="label">Übersicht</span>
          <h1 className="font-stencil text-[44px] font-bold leading-none tracking-tight text-ink-50">
            Veranstaltungen
          </h1>
          <p className="font-mono text-[12px] text-ink-200">
            Eine Tagesveranstaltung kann mehrere Wettkämpfe (Disziplinen) bündeln.
          </p>
        </div>
        <button
          type="button"
          onClick={() => setCreating((v) => !v)}
          className="btn btn-primary"
        >
          {creating ? 'Abbrechen' : '+ Neue Veranstaltung'}
        </button>
      </div>

      {creating && (
        <form
          onSubmit={form.handleSubmit(onSubmit)}
          className="surface mb-8 grid grid-cols-1 gap-4 p-6 md:grid-cols-3 animate-rise"
          noValidate
        >
          <div className="field">
            <label className="label" htmlFor="ev-name">
              Name
            </label>
            <input id="ev-name" className="input" {...form.register('name')} />
            {form.formState.errors.name && (
              <span className="text-[11px] text-status-unknown">
                {form.formState.errors.name.message}
              </span>
            )}
          </div>
          <div className="field">
            <label className="label" htmlFor="ev-date">
              Datum
            </label>
            <input
              id="ev-date"
              type="date"
              className="input"
              {...form.register('date')}
            />
          </div>
          <div className="field">
            <label className="label" htmlFor="ev-loc">
              Ort (optional)
            </label>
            <input id="ev-loc" className="input" {...form.register('location')} />
          </div>
          <div className="md:col-span-3 flex justify-end">
            <button
              type="submit"
              className="btn btn-primary"
              disabled={form.formState.isSubmitting}
            >
              Speichern
            </button>
          </div>
        </form>
      )}

      {error && (
        <p className="surface mb-6 px-5 py-3 text-[13px] text-status-unknown" role="alert">
          {error}
        </p>
      )}

      {events == null ? (
        <p className="label">Lade …</p>
      ) : events.length === 0 ? (
        <EmptyState />
      ) : (
        <ul className="grid grid-cols-1 gap-5 stagger-rise md:grid-cols-2 lg:grid-cols-3">
          {events.map((e) => (
            <li key={e.id}>
              <EventCard event={e} />
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}

function EventCard({ event }: { event: Event }) {
  return (
    <a
      href={`/events/${event.id}`}
      className="surface group relative flex flex-col gap-5 overflow-hidden p-6 transition hover:-translate-y-0.5 hover:shadow-[0_24px_60px_-20px_rgba(20,15,10,0.16)]"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex flex-col gap-1.5">
          <span className="label">Tagesveranstaltung</span>
          <h2 className="font-display text-[22px] font-semibold leading-tight text-ink-50">
            {event.name}
          </h2>
        </div>
        {event.is_active && (
          <span className="pill bg-status-running/15 text-status-running">
            <span className="h-1.5 w-1.5 rounded-full bg-current" />
            Aktiv
          </span>
        )}
      </div>

      <div className="grid grid-cols-2 gap-4 text-[13px]">
        <div className="flex flex-col gap-1">
          <span className="label">Datum</span>
          <span className="font-mono tnum text-ink-100">
            {formatDateDE(event.date)}
          </span>
        </div>
        {event.location && (
          <div className="flex flex-col gap-1">
            <span className="label">Ort</span>
            <span className="font-mono text-ink-100 truncate">{event.location}</span>
          </div>
        )}
      </div>

      <span className="absolute -bottom-12 -right-10 select-none font-stencil text-[180px] font-bold leading-none tracking-tighter text-[color:var(--watermark-color)]">
        {event.date.slice(8, 10)}
      </span>
    </a>
  );
}

function EmptyState() {
  return (
    <div className="surface flex flex-col items-center justify-center gap-3 py-16">
      <span className="label">Noch keine Veranstaltungen</span>
      <p className="font-display text-[18px] text-ink-200">
        Lege oben eine neue Tagesveranstaltung an.
      </p>
    </div>
  );
}
