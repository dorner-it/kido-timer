// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { useEffect, useState } from 'react';

import { ApiError, apiClient, type AuthenticatedUser } from '../../lib/api';
import { Brandmark } from './Brandmark';

interface Props {
  /** Optional left-of-center title; defaults to "Wettkampf-Kontrollstand". */
  context?: string;
}

export default function TopBar({ context }: Props) {
  const [user, setUser] = useState<AuthenticatedUser | null>(null);
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    apiClient.auth
      .me()
      .then((res) => setUser(res.user))
      .catch((err) => {
        if (err instanceof ApiError && err.status === 401) {
          window.location.href = "/login";
        }
      });
  }, []);

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(id);
  }, []);

  const clock = now.toLocaleTimeString('de-DE', { hour12: false });

  return (
    <header className="flex items-center justify-between gap-6 border-b border-[color:var(--hairline-color)] px-7 py-4">
      <div className="flex items-center gap-4">
        <a href="/competitions" aria-label="Zur Übersicht" className="flex items-center gap-4">
          <Brandmark />
          <div className="hidden md:flex flex-col leading-tight">
            <span className="font-display text-[15px] font-medium tracking-wide text-ink-50">
              KI-DO Timer Web
            </span>
            <span className="label">{context ?? 'Wettkampf-Kontrollstand'}</span>
          </div>
        </a>
      </div>

      <div className="flex flex-1 items-center justify-center">
        <div className="flex items-center gap-3 rounded-full border border-[color:var(--hairline-color)] bg-ink-700/40 px-4 py-1.5 backdrop-blur">
          <span className="h-2 w-2 rounded-full bg-status-confirmed" />
          <span className="font-mono text-[11px] uppercase tracking-[0.22em] text-ink-100">
            Verbunden
          </span>
        </div>
      </div>

      <div className="flex items-center gap-5 text-right">
        <div className="flex flex-col items-end leading-tight">
          <span className="font-mono tnum text-[18px] tracking-wider text-ink-50">{clock}</span>
          <span className="label">Ortszeit</span>
        </div>
        {user && (
          <div className="hidden lg:flex flex-col items-end leading-tight">
            <span className="font-mono text-[13px] tracking-wider text-ink-50">
              {user.display_name}
            </span>
            <span className="label">{user.email}</span>
          </div>
        )}
        <a href="/settings" className="btn" title="Einstellungen">
          ⚙
        </a>
        <form method="post" action="/api/auth/logout">
          <button type="submit" className="btn">
            Abmelden
          </button>
        </form>
      </div>
    </header>
  );
}
