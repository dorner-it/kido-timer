// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Entry of the static on-prem judge build (`npm run build:judge` →
 * `dist-judge/`, base `/judge/`). Mounts the very same `JudgeApp` as the
 * Astro page `/decider/[owner_sub]/[token]`; only the token/owner come
 * from the URL (`?t=…&o=…`) and the API base is the serving origin (the
 * desktop app in the venue LAN).
 */
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import JudgeApp from '../components/react/judge/JudgeApp';
import { resolveJudgeParams, standaloneApiBase } from '../lib/judge-spa-params';
import '../styles/global.css';

// Same origin by default — never a build-time cloud URL.
window.__KIDO_API_BASE__ = standaloneApiBase(window.__KIDO_API_BASE__);

const root = document.getElementById('root');
if (root) {
  const resolved = resolveJudgeParams(window.location.search, window.__KIDO_OWNER_SUB__);
  createRoot(root).render(
    <StrictMode>
      {resolved.ok ? (
        <JudgeApp ownerSub={resolved.params.ownerSub} token={resolved.params.token} />
      ) : (
        <main className="mx-auto flex min-h-screen max-w-xl flex-col items-center justify-center px-6 text-center">
          <p className="text-[20px] font-bold text-ink-50">Strafrichter</p>
          <p className="mt-3 text-[16px] text-ink-100">{resolved.message}</p>
        </main>
      )}
    </StrictMode>,
  );
}
