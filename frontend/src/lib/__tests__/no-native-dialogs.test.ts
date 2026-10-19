// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

// Guard: no native `alert()` / `confirm()` / `prompt()` in the web
// frontend or the desktop app — use `useDialog()` (organizer),
// `useJudgeDialog()` (judge app) or the desktop `ConfirmDialog`.
//
// TS/TSX/JS files are checked with the TypeScript type checker, so a
// call counts as native exactly when it resolves to the DOM lib
// declaration (bare `confirm(...)`, `window.confirm(...)`,
// `globalThis.alert(...)`), while the `confirm` returned by
// `useDialog()` is fine. `.astro` files (inline scripts) are checked
// textually.
//
// Sources are read via Vite's `import.meta.glob` (`?raw`), so the test
// needs no Node APIs / @types/node.

import ts from 'typescript';
import { describe, expect, it } from 'vitest';

const NATIVE = new Set(['alert', 'confirm', 'prompt']);

const raw = {
  ...import.meta.glob('/src/**/*.{ts,tsx,js,jsx,mjs,astro}', {
    query: '?raw',
    import: 'default',
    eager: true,
  }),
  ...import.meta.glob('../../../../desktop/src/**/*.{ts,tsx,js,jsx,mjs}', {
    query: '?raw',
    import: 'default',
    eager: true,
  }),
} as Record<string, string>;

/** Repo-relative display name (`frontend/src/…`, `desktop/src/…`). */
function display(key: string): string {
  return key.startsWith('/src/') ? `frontend${key}` : key.replace(/^(\.\.\/)+/, '');
}

function isTest(path: string): boolean {
  return /\/__tests__\//.test(path) || /\.(test|spec)\.[cm]?[jt]sx?$/.test(path);
}

const sources = new Map<string, string>();
for (const [key, text] of Object.entries(raw)) {
  if (!isTest(key) && !key.endsWith('.d.ts')) sources.set(display(key), text);
}
const scriptNames = [...sources.keys()].filter((f) => !f.endsWith('.astro'));
const astroNames = [...sources.keys()].filter((f) => f.endsWith('.astro'));

const OPTIONS: ts.CompilerOptions = {
  allowJs: true,
  jsx: ts.JsxEmit.ReactJSX,
  target: ts.ScriptTarget.ESNext,
  module: ts.ModuleKind.ESNext,
  lib: ['lib.esnext.d.ts', 'lib.dom.d.ts'],
  noResolve: true,
  noEmit: true,
  skipLibCheck: true,
  types: [],
};

/** Program over in-memory files; the TS default libs come from disk. */
function programFor(files: Map<string, string>): ts.Program {
  const host = ts.createCompilerHost(OPTIONS);
  const orig = host.getSourceFile.bind(host);
  host.getSourceFile = (name, lang, ...rest) => {
    const text = files.get(name);
    if (text === undefined) return orig(name, lang, ...rest);
    const kind = /\.[jt]sx$/.test(name) ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
    return ts.createSourceFile(name, text, ts.ScriptTarget.ESNext, true, kind);
  };
  host.fileExists = (name) => files.has(name) || ts.sys.fileExists(name);
  return ts.createProgram([...files.keys()], OPTIONS, host);
}

/** Native dialog calls in all non-lib source files of `program`. */
function findNativeCalls(program: ts.Program): string[] {
  const checker = program.getTypeChecker();
  const fromDomLib = (node: ts.Node): boolean =>
    (checker.getSymbolAtLocation(node)?.getDeclarations() ?? []).some((d) =>
      program.isSourceFileDefaultLibrary(d.getSourceFile()),
    );
  const hits: string[] = [];
  for (const sf of program.getSourceFiles()) {
    if (sf.isDeclarationFile) continue;
    const visit = (node: ts.Node) => {
      if (ts.isCallExpression(node)) {
        const callee = node.expression;
        const name = ts.isIdentifier(callee)
          ? callee
          : ts.isPropertyAccessExpression(callee)
            ? callee.name
            : null;
        if (name && NATIVE.has(name.text) && fromDomLib(name)) {
          const { line } = sf.getLineAndCharacterOfPosition(node.getStart(sf));
          hits.push(`${sf.fileName}:${line + 1}: ${callee.getText(sf)}()`);
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(sf);
  }
  return hits;
}

function nativeCallsInAstro(): string[] {
  const re = /(?<![\w$.])(?:(?:window|globalThis|self)\s*\.\s*)?(?:alert|confirm|prompt)\s*\(/;
  const hits: string[] = [];
  for (const name of astroNames) {
    (sources.get(name) ?? '').split('\n').forEach((text: string, i: number) => {
      if (re.test(text)) hits.push(`${name}:${i + 1}: ${text.trim()}`);
    });
  }
  return hits;
}

describe('no native browser dialogs', () => {
  it('scans frontend and desktop sources', () => {
    expect(scriptNames.filter((f) => f.startsWith('frontend/src/')).length).toBeGreaterThan(20);
    expect(scriptNames.filter((f) => f.startsWith('desktop/src/')).length).toBeGreaterThan(5);
    expect(astroNames.length).toBeGreaterThan(0);
    expect(scriptNames.some((f) => f.includes('__tests__'))).toBe(false);
  });

  it('detects native calls but not the useDialog() confirm (self-check)', () => {
    const sample = `
      export async function f() {
        if (!confirm('a')) return;
        window.alert('b');
        globalThis.prompt('c');
        const { confirm: ask } = useDialog();
        await ask({ title: 't', message: 'm' });
      }
      export function g() {
        const { confirm, alert } = useDialog();
        void confirm({ title: 't', message: 'm' });
        void alert({ title: 't', message: 'm' });
      }
      declare function useDialog(): {
        confirm: (o: object) => Promise<boolean>;
        alert: (o: object) => Promise<void>;
      };
    `;
    const hits = findNativeCalls(programFor(new Map([['sample.tsx', sample]])));
    expect(hits.map((h) => h.replace(/^.*: /, ''))).toEqual([
      'confirm()',
      'window.alert()',
      'globalThis.prompt()',
    ]);
  });

  it('frontend/src and desktop/src use styled dialogs only', () => {
    const scripts = new Map(scriptNames.map((n) => [n, sources.get(n) ?? '']));
    expect([...findNativeCalls(programFor(scripts)), ...nativeCallsInAstro()]).toEqual([]);
  });
});
