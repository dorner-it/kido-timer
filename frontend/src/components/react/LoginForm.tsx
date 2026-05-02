// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { Brandmark } from './Brandmark';
import { apiClient } from '../../lib/api';

export default function LoginForm() {
  return (
    <div className="flex w-full max-w-md flex-col items-center gap-10">
      <div className="flex flex-col items-center gap-5">
        <Brandmark size="lg" />
        <div className="flex flex-col items-center gap-1">
          <span className="font-stencil text-[44px] font-bold leading-none tracking-tight text-ink-50">
            KI-DO TIMER
          </span>
          <span className="label">Wettkampf-Kontrollstand</span>
        </div>
      </div>

      <a
        href={apiClient.auth.loginUrl()}
        className="btn btn-primary px-6 py-3 text-[12px]"
      >
        <span className="h-1.5 w-1.5 animate-pulse-ring rounded-full bg-current" />
        Anmelden
      </a>

      <div className="flex flex-col items-center gap-2 text-center">
        <p className="font-mono text-[11px] uppercase tracking-[0.18em] text-ink-300">
          Authentifizierung über AWS Cognito
        </p>
        <p className="max-w-xs text-[13px] leading-relaxed text-ink-200/80">
          Nur registrierte Organisatoren erhalten Zugang. Bei Problemen mit dem
          Passwort bitte das Cognito-Konsolen-Reset nutzen.
        </p>
      </div>
    </div>
  );
}
