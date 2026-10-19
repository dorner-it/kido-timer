import type { FrameDto } from "../lib/types";
import { laneGridClass } from "../lib/lanes";
import { LaneCard } from "./LaneCard";

interface Props {
  frame: FrameDto | null;
  corrections: number[];
  /** Lanes to show (discipline `lane_count`; 4 without discipline).
   *  Lane N = timer channel N. */
  laneCount: number;
  /** Optional per-lane caption (team on that lane), index = lane - 1. */
  laneCaptions?: (string | null)[];
  selectedLane: number | null;
  onSelectLane: (lane: number | null) => void;
  onAdjust: (channel: number, deltaMs: number) => void;
  onClear: (channel: number) => void;
}

const PLACEHOLDER_FRAME: FrameDto = {
  deviceMode: "standby",
  deviceModeRaw: 0,
  lane: 0,
  stateFlag: "measuring",
  stateFlagRaw: 0,
  channels: [
    { timeMs: 0, status: "inactive", statusRaw: 0 },
    { timeMs: 0, status: "inactive", statusRaw: 0 },
    { timeMs: 0, status: "inactive", statusRaw: 0 },
    { timeMs: 0, status: "inactive", statusRaw: 0 },
  ],
  rawHex: "",
};

export function LaneGrid({
  frame,
  corrections,
  laneCount,
  laneCaptions = [],
  selectedLane,
  onSelectLane,
  onAdjust,
  onClear,
}: Props) {
  const f = frame ?? PLACEHOLDER_FRAME;
  const channels = f.channels.slice(0, Math.max(1, laneCount));

  return (
    <div className={`grid h-full gap-5 ${laneGridClass(channels.length)}`}>
      {channels.map((ch, idx) => (
        <LaneCard
          key={idx}
          lane={idx + 1}
          channel={ch}
          caption={laneCaptions[idx] ?? null}
          correctionMs={corrections[idx] ?? 0}
          selected={selectedLane === idx}
          onSelect={() => onSelectLane(selectedLane === idx ? null : idx)}
          onAdjust={(ms) => onAdjust(idx, ms)}
          onClear={() => onClear(idx)}
        />
      ))}
    </div>
  );
}
