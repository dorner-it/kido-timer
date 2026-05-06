// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import {
  ApiError,
  apiClient,
  type ApiKeyCreatedResponse,
  type ApiKeySummary,
} from '../../lib/api';
import { formatDateDE } from '../../lib/format';

export default function UserSettings() {
  const [keys, setKeys] = useState<ApiKeySummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [newKeyName, setNewKeyName] = useState('');
  const [justCreated, setJustCreated] = useState<ApiKeyCreatedResponse | null>(null);

  useEffect(() => {
    refresh();
  }, []);

  const refresh = async () => {
    try {
      setKeys(await apiClient.apiKeys.list());
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        window.location.href = '/login';
        return;
      }
      setError(err instanceof Error ? err.message : 'Unbekannter Fehler');
    }
  };

  const create = async (e: React.FormEvent) => {
    e.preventDefault();
    setCreating(true);
    setError(null);
    try {
      const res = await apiClient.apiKeys.create({
        name: newKeyName.trim() || undefined,
      });
      setJustCreated(res);
      setNewKeyName('');
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Erstellung fehlgeschlagen');
    } finally {
      setCreating(false);
    }
  };

  const revoke = async (key: ApiKeySummary) => {
    if (!confirm(`Schlüssel „${key.name}" (${key.prefix}…) widerrufen?`)) return;
    try {
      await apiClient.apiKeys.revoke(key.key_id);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Widerruf fehlgeschlagen');
    }
  };

  return (
    <main className="mx-auto w-full max-w-3xl px-7 py-10">
      <a href="/competitions" className="label mb-3 inline-block hover:text-ink-50">
        ← Zur Übersicht
      </a>

      <h1 className="font-stencil text-[36px] font-bold leading-none tracking-tight text-ink-50">
        Einstellungen
      </h1>

      {error && (
        <p className="surface mt-6 px-5 py-3 text-[13px] text-status-unknown" role="alert">
          {error}
        </p>
      )}

      <section className="mt-8 flex flex-col gap-5">
        <div className="flex items-end justify-between">
          <div>
            <h2 className="font-display text-[20px] font-medium text-ink-50">
              Desktop-API-Schlüssel
            </h2>
            <p className="mt-1 text-[12px] text-ink-200">
              Schlüssel für die Desktop-App (Live-Modus + Datei-Export). Plain-Text wird nur einmal
              bei der Erzeugung angezeigt.
            </p>
          </div>
        </div>

        {justCreated && (
          <div className="surface bg-status-confirmed/5 border-status-confirmed/40 p-5 animate-rise">
            <span className="label text-status-confirmed">Schlüssel erstellt</span>
            <p className="mt-2 text-[13px] text-ink-100">
              Speichern Sie diesen Schlüssel <strong>jetzt</strong>. Er wird nie wieder angezeigt.
            </p>
            <div className="mt-3 flex items-center gap-2">
              <code className="flex-1 break-all rounded-md border border-[color:var(--hairline-color)] bg-ink-700/50 px-3 py-2 font-mono text-[13px] text-ink-50">
                {justCreated.plaintext}
              </code>
              <button
                type="button"
                className="btn"
                onClick={() => navigator.clipboard.writeText(justCreated.plaintext)}
              >
                Kopieren
              </button>
            </div>
            <button
              type="button"
              className="btn mt-3"
              onClick={() => setJustCreated(null)}
            >
              Verstanden
            </button>
          </div>
        )}

        <form onSubmit={create} className="surface flex flex-wrap gap-3 p-4">
          <div className="field flex-1 min-w-[200px]">
            <label className="label" htmlFor="key_name">
              Bezeichnung (optional)
            </label>
            <input
              id="key_name"
              className="input"
              placeholder="z. B. „MacBook Wettkampfleitung"
              value={newKeyName}
              onChange={(e) => setNewKeyName(e.target.value)}
            />
          </div>
          <button type="submit" className="btn btn-primary self-end" disabled={creating}>
            {creating ? 'Erstelle …' : '+ Neuer Schlüssel'}
          </button>
        </form>

        {keys === null ? (
          <p className="label">Lade …</p>
        ) : keys.length === 0 ? (
          <p className="surface px-5 py-6 text-center text-[14px] text-ink-200">
            Noch keine Schlüssel angelegt.
          </p>
        ) : (
          <ul className="flex flex-col gap-2">
            {keys.map((k) => (
              <li
                key={k.key_id}
                className="surface flex items-center justify-between gap-3 px-5 py-3"
              >
                <div className="flex flex-col gap-1">
                  <span className="font-display text-[15px] text-ink-50">{k.name}</span>
                  <span className="font-mono text-[11px] text-ink-200">
                    {k.prefix}… · erstellt {formatDateDE(k.created_at.slice(0, 10))}
                  </span>
                </div>
                <button type="button" className="btn btn-danger" onClick={() => revoke(k)}>
                  Widerrufen
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </main>
  );
}
