// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { describe, expect, it, vi } from 'vitest';

import { createDialogController } from '../dialog';

describe('dialog controller', () => {
  it('confirm resolves true when confirmed', async () => {
    const d = createDialogController();
    const p = d.confirm({ title: 'Löschen?', message: 'Text', confirmLabel: 'Löschen', tone: 'danger' });
    const cur = d.current();
    expect(cur).toMatchObject({
      kind: 'confirm',
      title: 'Löschen?',
      paragraphs: ['Text'],
      confirmLabel: 'Löschen',
      cancelLabel: 'Abbrechen',
      tone: 'danger',
    });
    d.settle(true);
    await expect(p).resolves.toBe(true);
    expect(d.current()).toBeNull();
  });

  it('confirm resolves false when cancelled', async () => {
    const d = createDialogController();
    const p = d.confirm({ title: 'X?', message: 'Y' });
    d.settle(false);
    await expect(p).resolves.toBe(false);
  });

  it('applies defaults and drops empty paragraphs', () => {
    const d = createDialogController();
    void d.confirm({ title: 'X?', message: ['a', '', '  ', 'b'] });
    expect(d.current()).toMatchObject({
      confirmLabel: 'OK',
      cancelLabel: 'Abbrechen',
      tone: 'default',
      paragraphs: ['a', 'b'],
    });
  });

  it('alert has a single button and resolves on settle (either way)', async () => {
    const d = createDialogController();
    const a = d.alert({ title: 'Fertig', message: 'Bestätigt: 2' });
    expect(d.current()).toMatchObject({ kind: 'alert', cancelLabel: null, confirmLabel: 'OK' });
    d.settle(false); // Escape / backdrop on an alert
    await expect(a).resolves.toBeUndefined();
  });

  it('shows one dialog at a time and queues the rest FIFO', async () => {
    const d = createDialogController();
    const first = d.confirm({ title: 'Eins', message: '1' });
    const second = d.confirm({ title: 'Zwei', message: '2' });
    const third = d.alert({ title: 'Drei', message: '3' });
    expect(d.current()?.title).toBe('Eins');
    expect(d.pending()).toBe(2);

    d.settle(true);
    await expect(first).resolves.toBe(true);
    expect(d.current()?.title).toBe('Zwei');

    d.settle(false);
    await expect(second).resolves.toBe(false);
    expect(d.current()?.title).toBe('Drei');

    d.settle(true);
    await expect(third).resolves.toBeUndefined();
    expect(d.current()).toBeNull();
    expect(d.pending()).toBe(0);
  });

  it('settle without an open dialog is a no-op', () => {
    const d = createDialogController();
    expect(d.settle(true)).toBeNull();
  });

  it('gives every request a fresh id and keeps the opener', () => {
    const d = createDialogController();
    const opener = { tag: 'button' };
    void d.confirm({ title: 'A', message: 'a' }, opener);
    const a = d.current();
    d.settle(true);
    void d.confirm({ title: 'B', message: 'b' });
    expect(d.current()?.id).not.toBe(a?.id);
    expect(a?.opener).toBe(opener);
  });

  it('notifies subscribers when the head changes, not when queueing behind it', () => {
    const d = createDialogController();
    const listener = vi.fn();
    const unsubscribe = d.subscribe(listener);
    void d.confirm({ title: 'A', message: 'a' });
    expect(listener).toHaveBeenCalledTimes(1);
    void d.confirm({ title: 'B', message: 'b' });
    expect(listener).toHaveBeenCalledTimes(1);
    d.settle(true);
    expect(listener).toHaveBeenCalledTimes(2);
    unsubscribe();
    d.settle(true);
    expect(listener).toHaveBeenCalledTimes(2);
  });

  it('dispose cancels current and queued dialogs', async () => {
    const d = createDialogController();
    const a = d.confirm({ title: 'A', message: 'a' });
    const b = d.confirm({ title: 'B', message: 'b' });
    const c = d.alert({ title: 'C', message: 'c' });
    d.dispose();
    await expect(a).resolves.toBe(false);
    await expect(b).resolves.toBe(false);
    await expect(c).resolves.toBeUndefined();
    expect(d.current()).toBeNull();
  });

  it('stays usable after dispose (StrictMode re-mount)', async () => {
    const d = createDialogController();
    d.dispose();
    const p = d.confirm({ title: 'A', message: 'a' });
    expect(d.current()?.title).toBe('A');
    d.settle(true);
    await expect(p).resolves.toBe(true);
  });
});
