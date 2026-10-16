import type { FrameDto } from "../lib/types";
import { LaneCard } from "./LaneCard";

interface Props {
  frame: FrameDto | null;
  corrections: number[];
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
  selectedLane,
  onSelectLane,
  onAdjust,
  onClear,
}: Props) {
  const f = frame ?? PLACEHOLDER_FRAME;

  return (
    <div className="grid h-full grid-cols-2 grid-rows-2 gap-5">
      {f.channels.map((ch, idx) => (
        <LaneCard
          key={idx}
          lane={idx + 1}
          channel={ch}
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
