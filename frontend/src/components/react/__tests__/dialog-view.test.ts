// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

// SSR smoke tests of the styled confirm/alert dialogs (no DOM, so only
// markup: title, paragraphs, labels, tone, initial focus).

import { createElement as h } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { createDialogController, type DialogRequest } from '../../../lib/dialog';
import { JudgeDialogView } from '../judge/JudgeDialog';
import { DialogView } from '../useDialog';

function request(open: (d: ReturnType<typeof createDialogController>) => void): DialogRequest {
  const d = createDialogController();
  open(d);
  return d.current()!;
}

const danger = request((d) => {
  void d.confirm({
    title: 'Läufer löschen?',
    message: ['„Max Muster“ wird aus der Mannschaft entfernt.', 'Zweiter Absatz'],
    confirmLabel: 'Löschen',
    tone: 'danger',
  });
});
const plain = request((d) => {
  void d.confirm({ title: 'Durchgang anlegen?', message: '3 Läufe', confirmLabel: 'Durchgang anlegen' });
});
const info = request((d) => {
  void d.alert({ title: 'Lauf übernommen', message: 'Bestätigt: 2' });
});
const noop = () => {};

describe('DialogView (organizer)', () => {
  it('renders title, paragraphs and both buttons for a danger confirm', () => {
    const html = renderToStaticMarkup(h(DialogView, { request: danger, settle: noop }));
    expect(html).toContain('role="dialog"');
    expect(html).toContain('Läufer löschen?');
    expect(html).toContain('„Max Muster“ wird aus der Mannschaft entfernt.');
    expect(html).toContain('Zweiter Absatz');
    expect(html).toContain('btn btn-danger');
    expect(html).toMatch(/>Abbrechen</);
    expect(html).toMatch(/>Löschen</);
  });

  it('uses the primary button for default tone', () => {
    const html = renderToStaticMarkup(h(DialogView, { request: plain, settle: noop }));
    expect(html).toContain('btn btn-primary');
    expect(html).not.toContain('btn-danger');
  });

  it('alerts have a single OK button', () => {
    const html = renderToStaticMarkup(h(DialogView, { request: info, settle: noop }));
    expect(html).not.toContain('Abbrechen');
    expect(html).toMatch(/>OK</);
  });
});

describe('JudgeDialogView', () => {
  it('renders the big touch buttons', () => {
    const html = renderToStaticMarkup(h(JudgeDialogView, { request: danger, settle: noop }));
    expect(html).toContain('Läufer löschen?');
    expect(html).toContain('min-h-[72px]');
    expect(html).toContain('bg-red-700');
    expect(html).toMatch(/>Abbrechen</);
  });

  it('alerts use role=alertdialog without cancel', () => {
    const html = renderToStaticMarkup(h(JudgeDialogView, { request: info, settle: noop }));
    expect(html).toContain('role="alertdialog"');
    expect(html).not.toContain('Abbrechen');
  });
});
