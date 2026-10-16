export type ChannelStatus =
  | "inactive"
  | "running"
  | "captured"
  | "confirmed"
  | "unknown";

export type DeviceMode = "standby" | "active" | "unknown";
export type StateFlag = "measuring" | "armed" | "unknown";

export interface ChannelDto {
  timeMs: number;
  status: ChannelStatus;
  statusRaw: number;
}

export interface FrameDto {
  deviceMode: DeviceMode;
  deviceModeRaw: number;
  lane: number;
  stateFlag: StateFlag;
  stateFlagRaw: number;
  channels: [ChannelDto, ChannelDto, ChannelDto, ChannelDto];
  rawHex: string;
}

export type ConnectionEvent =
  | { type: "frame"; frame: FrameDto }
  | { type: "error"; message: string }
  | { type: "disconnected" }
  | { type: "connected"; source: string };

export type ConnectionStatus = "idle" | "connecting" | "connected" | "error";

export interface RunEntry {
  id: string;
  /** When the result was first confirmed by the device. */
  timestamp: number;
  /** Device lane field (from the protocol frame). */
  lane: number;
  /** 1-indexed channel/lane number this entry belongs to. */
  channel: number;
  /** Original time as reported by the timer, before any correction. */
  originalTimeMs: number;
  /** Operator-applied correction (penalty / handicap) in milliseconds. */
  correctionMs: number;
}

/** Sum of the device-measured time and any operator correction. */
export function totalTimeMs(entry: RunEntry): number {
  return entry.originalTimeMs + entry.correctionMs;
}
