import { t } from "../lib/i18n";
import type { CatalogueItem, Penalty } from "../lib/cloudTypes";
import {
  formatRunSummary,
  penaltyLabel,
  penaltyStationLabel,
  penaltyStatusLabel,
  summarizeRun,
} from "../lib/penalties";

interface Props {
  penalties: Penalty[];
  catalogue: Map<string, CatalogueItem>;
}

/** Compact live error list of one run + its provisional summary. */
export function RunPenalties({ penalties, catalogue }: Props) {
  const summary = summarizeRun(penalties);
  const tone = !summary.valid
    ? "text-status-unknown"
    : summary.szCount > 0
      ? "text-status-armed"
      : "text-status-confirmed";
  return (
    <div className="mt-2 flex flex-col gap-1">
      <p className={`font-mono text-[11px] tracking-wider ${tone}`}>
        {formatRunSummary(summary)}
      </p>
      {penalties.length === 0 ? (
        <p className="font-mono text-[10px] text-ink-200/60">{t.penalties.none}</p>
      ) : (
        <ul className="flex flex-col gap-0.5">
          {penalties.map((p) => {
            const rejected = p.status === "rejected";
            return (
              <li
                key={p.id}
                className={[
                  "flex items-baseline gap-2 font-mono text-[10px] leading-snug",
                  rejected ? "line-through text-ink-200/50" : "text-ink-100",
                ].join(" ")}
                title={p.code ?? undefined}
              >
                <span
                  className={[
                    "shrink-0 rounded px-1 py-px text-[9px] uppercase tracking-[0.14em] ring-1",
                    p.kind === "fv"
                      ? "bg-status-unknown/15 text-status-unknown ring-status-unknown/30"
                      : "bg-status-armed/15 text-status-armed ring-status-armed/30",
                  ].join(" ")}
                >
                  {p.kind === "fv" ? t.penalties.fv : t.penalties.sz}
                </span>
                <span className="shrink-0 text-ink-200/70">{penaltyStationLabel(p)}</span>
                <span className="min-w-0 flex-1 truncate">{penaltyLabel(p, catalogue)}</span>
                <span className="shrink-0 uppercase tracking-[0.14em] text-ink-200/60">
                  {penaltyStatusLabel(p.status)}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
