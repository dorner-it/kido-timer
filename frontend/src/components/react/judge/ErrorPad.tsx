// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import type { CatalogueEntry } from '../../../lib/api';
import {
  choiceButtonLabel,
  flowTitle,
  isDialogOpen,
  type EntryFlow,
} from '../../../lib/judge-entry';
import { BTN_CANCEL, BTN_DANGER, BTN_PRIMARY, FV_BADGE, SZ_BADGE } from './ui';

export function KindBadge({ kind }: { kind: 'sz' | 'fv' }) {
  return kind === 'fv' ? (
    <span className={FV_BADGE} aria-label="Fehlversuch">
      FV<span className="text-[10px] font-semibold">ungültig</span>
    </span>
  ) : (
    <span className={SZ_BADGE} aria-label="Strafzeit 15 Sekunden">
      SZ<span className="text-[10px] font-semibold">+15 s</span>
    </span>
  );
}

/** Catalogue buttons of one station, grouped SZ / FV (sheet order). */
export function ErrorPad({
  entries,
  disabled,
  flashCode,
  onTap,
}: {
  entries: CatalogueEntry[];
  disabled: boolean;
  flashCode: string | null;
  onTap: (entry: CatalogueEntry) => void;
}) {
  const sz = entries.filter((e) => e.kind === 'sz');
  const fv = entries.filter((e) => e.kind === 'fv');
  return (
    <div className="flex flex-col gap-6">
      {sz.length > 0 && (
        <ErrorGroup
          title="Strafzeit · +15 s je Fehler"
          entries={sz}
          disabled={disabled}
          flashCode={flashCode}
          onTap={onTap}
        />
      )}
      {fv.length > 0 && (
        <ErrorGroup
          title="Fehlversuch · Lauf ungültig"
          entries={fv}
          disabled={disabled}
          flashCode={flashCode}
          onTap={onTap}
        />
      )}
    </div>
  );
}

function ErrorGroup({
  title,
  entries,
  disabled,
  flashCode,
  onTap,
}: {
  title: string;
  entries: CatalogueEntry[];
  disabled: boolean;
  flashCode: string | null;
  onTap: (entry: CatalogueEntry) => void;
}) {
  return (
    <section>
      <h2 className="mb-2 text-[14px] font-bold uppercase tracking-wide text-ink-100">{title}</h2>
      <div className="flex flex-col gap-3">
        {entries.map((e) => {
          const flashing = flashCode === e.code;
          const base =
            e.kind === 'fv'
              ? 'border-red-700 bg-red-50 text-ink-50'
              : 'border-ink-50 bg-white text-ink-50';
          return (
            <button
              key={e.code}
              type="button"
              disabled={disabled}
              onClick={() => onTap(e)}
              className={`flex min-h-[72px] w-full items-center gap-3 rounded-xl border-2 px-3 py-3 text-left text-[17px] font-semibold leading-snug transition active:scale-[0.98] disabled:opacity-40 ${base} ${
                flashing ? 'outline outline-4 outline-green-600' : ''
              }`}
            >
              <KindBadge kind={e.kind} />
              <span>{e.label_de}</span>
            </button>
          );
        })}
      </div>
    </section>
  );
}

/** Bottom sheet: lane choice (SZ, two runs) or FV confirmation. */
export function EntryDialog({
  flow,
  busy,
  onChoose,
  onCancel,
}: {
  flow: EntryFlow<CatalogueEntry>;
  busy: boolean;
  onChoose: (runId: string) => void;
  onCancel: () => void;
}) {
  if (!isDialogOpen(flow) || (flow.step !== 'choose_run' && flow.step !== 'confirm_fv')) {
    return null;
  }
  const isFv = flow.step === 'confirm_fv';
  return (
    <div
      className="fixed inset-0 z-[60] flex items-end justify-center bg-black/60 sm:items-center"
      role="dialog"
      aria-modal="true"
      aria-label={flowTitle(flow) ?? undefined}
    >
      <div
        className={`w-full max-w-lg rounded-t-2xl border-t-4 bg-white p-5 pb-8 sm:rounded-2xl ${
          isFv ? 'border-red-700' : 'border-ink-50'
        }`}
      >
        <div className="flex items-start gap-3">
          <KindBadge kind={flow.entry.kind} />
          <div className="flex flex-col">
            <span
              className={`text-[22px] font-bold leading-tight ${isFv ? 'text-red-800' : 'text-ink-50'}`}
            >
              {flowTitle(flow)}
            </span>
            <span className="mt-1 text-[16px] text-ink-100">{flow.entry.label_de}</span>
          </div>
        </div>
        <div className="mt-5 flex flex-col gap-3">
          {flow.choices.map((c) => (
            <button
              key={c.run_id}
              type="button"
              disabled={busy}
              className={`${isFv ? BTN_DANGER : BTN_PRIMARY} min-h-[72px] text-[19px]`}
              onClick={() => onChoose(c.run_id)}
            >
              {choiceButtonLabel(flow, c)}
            </button>
          ))}
          <button type="button" className={BTN_CANCEL} onClick={onCancel}>
            Abbrechen
          </button>
        </div>
      </div>
    </div>
  );
}
