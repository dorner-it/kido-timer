import { useEffect, useRef } from "react";
import type { ChannelDto } from "../lib/types";
import { formatCorrection, formatTime, STATUS_COLOR, STATUS_RGB } from "../lib/format";
import { statusLabelDE, t } from "../lib/i18n";

interface Props {
  lane: number;
  channel: ChannelDto;
  correctionMs: number;
  selected: boolean;
  onSelect: () => void;
  onAdjust: (deltaMs: number) => void;
  onClear: () => void;
}

const HISTORY_POINTS = 80;

export function LaneCard({
  lane,
  channel,
  correctionMs,
  selected,
  onSelect,
  onAdjust,
  onClear,
}: Props) {
  const correctedMs = Math.max(0, channel.timeMs + correctionMs);
  const time = formatTime(correctedMs, channel.status);
  const accent = STATUS_COLOR[channel.status];
  const isLive = channel.status === "running";
  const isConfirmed = channel.status === "confirmed";
  const isCaptured = channel.status === "captured";

  const historyRef = useRef<number[]>([]);
  useEffect(() => {
    if (channel.status === "inactive") {
      historyRef.current = [];
      return;
    }
    historyRef.current.push(channel.timeMs);
    if (historyRef.current.length > HISTORY_POINTS) {
      historyRef.current.shift();
    }
  }, [channel.timeMs, channel.status]);

  const tracePoints = traceLine(historyRef.current, isLive);

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
      }}
      className={[
        "group relative flex h-full flex-col overflow-hidden rounded-2xl text-left cursor-pointer",
        "bg-gradient-to-br from-ink-800/95 to-ink-700/85",
        "ring-1 ring-ink-50/[0.06] transition-all duration-300",
        "focus:outline-none focus-visible:ring-2 focus-visible:ring-ink-50/40",
        selected ? "ring-2 ring-ink-50/30" : "hover:ring-ink-50/15",
        isLive && "shadow-glow-running",
        isConfirmed && "shadow-glow-confirmed",
        isCaptured && "shadow-glow-captured",
        !isLive && !isConfirmed && !isCaptured && "shadow-glow",
      ].filter(Boolean).join(" ")}
    >
      {/* Subtle scanlines + grain background */}
      <div className="pointer-events-none absolute inset-0 scanlines opacity-40" />
      {/* Status accent edge */}
      <span
        aria-hidden
        className="absolute left-0 top-0 h-full w-1 transition-colors"
        style={{ backgroundColor: accent }}
      />
      {/* Watermark giant lane number */}
      <span
        aria-hidden
        className="lane-watermark pointer-events-none absolute -right-6 -bottom-12 font-stencil text-[18rem] font-bold leading-none select-none"
      >
        {lane}
      </span>

      {/* Header */}
      <div className="relative flex items-start justify-between px-6 pt-5">
        <div className="flex items-center gap-3">
          <span className="font-stencil text-[34px] font-bold leading-none tracking-tight text-ink-50">
            <span className="text-ink-300/80">{t.lanes.laneShort}</span>
            <span className="ml-1">{lane}</span>
          </span>
          {selected && (
            <span className="pill border border-ink-50/15 bg-ink-50/5 text-ink-50">
              <span className="h-1.5 w-1.5 rounded-full bg-white" />
              Fokus
            </span>
          )}
        </div>
        <StatusBadge status={channel.status} />
      </div>

      {/* Time */}
      <div className="relative flex flex-1 flex-col items-center justify-center gap-2 px-4 min-w-0">
        <div className="relative max-w-full">
          <span
            className={[
              "digit time-digit block whitespace-nowrap text-[clamp(2.75rem,9vw,5.5rem)] leading-none tnum",
              channel.status === "inactive" && "is-inactive",
              isLive && "is-running",
              isConfirmed && "is-confirmed",
            ]
              .filter(Boolean)
              .join(" ")}
          >
            {time}
          </span>
          {isLive && (
            <span className="absolute -right-3 top-2 h-2 w-2 rounded-full bg-status-running animate-pulse-ring" />
          )}
        </div>
        {correctionMs !== 0 && channel.status !== "inactive" && (
          <Breakdown originalMs={channel.timeMs} correctionMs={correctionMs} />
        )}
      </div>

      {/* Trace line */}
      <div className="relative h-12 px-6">
        <svg viewBox="0 0 100 24" preserveAspectRatio="none" className="h-full w-full">
          <defs>
            <linearGradient id={`trace-${lane}`} x1="0" x2="1" y1="0" y2="0">
              <stop offset="0" style={{ stopColor: accent, stopOpacity: 0 }} />
              <stop offset="0.5" style={{ stopColor: accent, stopOpacity: 0.4 }} />
              <stop offset="1" style={{ stopColor: accent, stopOpacity: 1 }} />
            </linearGradient>
          </defs>
          <path
            d="M0,12 L100,12"
            style={{ stroke: "var(--hairline-color)" }}
            strokeWidth="0.4"
          />
          {tracePoints && (
            <path
              d={tracePoints}
              fill="none"
              stroke={`url(#trace-${lane})`}
              strokeWidth="1.2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          )}
        </svg>
      </div>

      {/* Footer: correction + controls */}
      <div
        className="relative flex items-center justify-between gap-4 border-t border-ink-50/[0.05] bg-ink-50/[0.03] px-6 py-3"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex flex-col leading-tight">
          <span className="label">{t.lanes.correction}</span>
          <span
            className={[
              "font-mono tnum text-sm tracking-wider",
              correctionMs === 0 ? "text-ink-200/70" : "text-status-armed",
            ].join(" ")}
          >
            {formatCorrection(correctionMs)}
          </span>
        </div>
        <div className="flex items-center gap-1">
          <SmallBtn label={t.lanes.minus5} onClick={() => onAdjust(-5000)} />
          <SmallBtn label={t.lanes.minus1} onClick={() => onAdjust(-1000)} />
          <SmallBtn label={t.lanes.plus1} onClick={() => onAdjust(1000)} />
          <SmallBtn label={t.lanes.plus5} onClick={() => onAdjust(5000)} />
          <SmallBtn
            label="↺"
            disabled={correctionMs === 0}
            onClick={() => onClear()}
            title={t.lanes.clearCorrection}
          />
        </div>
      </div>
    </div>
  );
}

function SmallBtn({
  label,
  onClick,
  disabled,
  title,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  title?: string;
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className="rounded-md border border-ink-50/10 bg-ink-50/[0.04] px-2.5 py-1.5 font-mono text-[11px] tabular-nums text-ink-100 transition hover:bg-ink-50/10 active:scale-[0.96] disabled:opacity-30 disabled:cursor-not-allowed"
    >
      {label}
    </button>
  );
}

function Breakdown({
  originalMs,
  correctionMs,
}: {
  originalMs: number;
  correctionMs: number;
}) {
  return (
    <div className="flex items-center gap-3 rounded-md border border-ink-50/[0.05] bg-ink-50/[0.04] px-3 py-1.5 font-mono text-[11px] tabular-nums text-ink-200/90">
      <span className="flex items-center gap-1">
        <span className="text-ink-300/80 uppercase tracking-[0.18em] text-[9px]">
          {t.lanes.original}
        </span>
        <span>{formatTime(originalMs, "confirmed")}</span>
      </span>
      <span className="text-ink-300">·</span>
      <span className="flex items-center gap-1">
        <span className="text-ink-300/80 uppercase tracking-[0.18em] text-[9px]">
          {t.lanes.penalty}
        </span>
        <span className="text-status-armed">{formatCorrection(correctionMs)}</span>
      </span>
    </div>
  );
}

function StatusBadge({ status }: { status: ChannelDto["status"] }) {
  const color = STATUS_COLOR[status];
  const rgb = STATUS_RGB[status];
  const live = status === "running";
  return (
    <div
      className="pill ring-1 ring-ink-50/[0.06]"
      style={{ background: `rgb(${rgb} / 0.10)`, color }}
    >
      <span
        className={`h-1.5 w-1.5 rounded-full ${live ? "animate-pulse" : ""}`}
        style={{ backgroundColor: color }}
      />
      {statusLabelDE(status)}
    </div>
  );
}

/** Build a normalized SVG path from a recent slice of time values. */
function traceLine(values: number[], isLive: boolean): string | null {
  if (values.length < 2) return null;
  const slice = values.slice(-HISTORY_POINTS);
  const max = Math.max(...slice);
  const min = Math.min(...slice);
  const range = Math.max(max - min, 1);
  const w = 100;
  const h = 22;
  const top = 1;
  const pts = slice.map((v, i) => {
    const x = (i / (slice.length - 1)) * w;
    // For a confirmed/captured run, draw a flat line at the value;
    // for a live run, draw the climb so it visibly rises.
    const y = isLive ? h - ((v - min) / range) * (h - top - 1) - 1 : h / 2;
    return `${x.toFixed(2)},${y.toFixed(2)}`;
  });
  return `M${pts.join(" L")}`;
}
