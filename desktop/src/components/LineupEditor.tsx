import { useMemo, useState } from "react";
import type { Run, Runner } from "../lib/cloudTypes";
import { t } from "../lib/i18n";
import { ConfirmDialog } from "./ConfirmDialog";

interface Props {
  run: Run;
  title: string;
  roster: Runner[];
  onClose: () => void;
  onSave: (runnerIds: string[]) => Promise<boolean>;
}

/** Lineup correction by the organizer (roster of the run's team). */
export function LineupEditor({ run, title, roster, onClose, onSave }: Props) {
  const sorted = useMemo(() => roster.slice().sort((a, b) => a.position - b.position), [roster]);
  const [picked, setPicked] = useState<string[]>(() =>
    run.lineup.filter((id) => sorted.some((r) => r.id === id)),
  );
  const [busy, setBusy] = useState(false);

  const toggle = (id: string) =>
    setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));

  const save = async () => {
    setBusy(true);
    // Keep roster order (position) for a stable lineup.
    const ordered = sorted.map((r) => r.id).filter((id) => picked.includes(id));
    const ok = await onSave(ordered);
    setBusy(false);
    if (ok) onClose();
  };

  return (
    <ConfirmDialog
      title={t.lineup.title(title)}
      confirmLabel={t.lineup.save}
      cancelLabel={t.lineup.cancel}
      busy={busy}
      onCancel={onClose}
      onConfirm={save}
      extraActions={
        <button type="button" className="btn mr-auto" disabled={busy || picked.length === 0} onClick={() => setPicked([])}>
          {t.lineup.clear}
        </button>
      }
    >
      <p className="text-[12px] text-ink-200">{t.lineup.intro}</p>
      {sorted.length === 0 ? (
        <p className="text-[12px] text-ink-200">{t.lineup.empty}</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {sorted.map((r) => (
            <li key={r.id}>
              <label className="flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 hover:bg-ink-50/[0.04]">
                <input
                  type="checkbox"
                  className="h-4 w-4"
                  checked={picked.includes(r.id)}
                  onChange={() => toggle(r.id)}
                />
                <span className="font-mono text-[11px] text-ink-200/70">{r.position}</span>
                <span className="text-[14px] text-ink-50">
                  {r.first_name} {r.last_name}
                </span>
                <span className="ml-auto font-mono text-[11px] text-ink-200/60">{r.birth_date}</span>
              </label>
            </li>
          ))}
        </ul>
      )}
      <p className="font-mono text-[11px] text-ink-200">{t.lineup.count(picked.length)}</p>
    </ConfirmDialog>
  );
}
