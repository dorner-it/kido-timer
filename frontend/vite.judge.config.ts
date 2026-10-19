// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

/**
 * Static build of the judge (Strafrichter) app for the on-prem mode:
 * `npm run build:judge` → `dist-judge/` (plain Vite SPA, no Astro SSR),
 * served by the desktop app under `/judge/` in the venue LAN and embedded
 * into the desktop binary at compile time (`desktop/src-tauri/build.rs`).
 *
 * No external fonts / CDNs; everything is bundled. The API base is the
 * serving origin (see `src/judge-spa/main.tsx`).
 */
import { fileURLToPath } from 'node:url';

import tailwindcss from 'tailwindcss';
import { defineConfig } from 'vite';

const here = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
  root: here('./src/judge-spa'),
  base: '/judge/',
  publicDir: here('./public'),
  // Never bake a cloud API URL into the on-prem build.
  envPrefix: 'KIDO_JUDGE_',
  define: {
    'import.meta.env.PUBLIC_API_BASE_URL': JSON.stringify(''),
  },
  esbuild: { jsx: 'automatic' },
  css: {
    postcss: {
      plugins: [tailwindcss({ config: here('./tailwind.config.cjs') })],
    },
  },
  build: {
    outDir: here('./dist-judge'),
    emptyOutDir: true,
    assetsDir: 'assets',
    target: 'es2019',
    sourcemap: false,
    chunkSizeWarningLimit: 1024,
  },
  logLevel: 'warn',
});
