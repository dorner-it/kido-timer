export type Theme = "light" | "dark";

/** Settings persisted across app launches. */
export interface AppSettings {
  /** Last-used serial port (e.g. "COM7" or "/dev/cu.usbserial-XXX"). */
  port: string | null;
  /** Last-used baud rate. */
  baud: number;
  /** When true, the app auto-connects on startup. */
  autoConnect: boolean;
  /** Last hex-dump file used for demo replay. */
  demoPath: string | null;
  /** Last demo replay speed multiplier. */
  demoSpeed: number;
  /** Has the user completed initial setup at least once? */
  hasCompletedSetup: boolean;
  /** UI theme. Default: light. */
  theme: Theme;
  /** Base URL of the kido-timer-web API (overridable per install). */
  cloudBaseUrl: string;
}

const STORAGE_KEY = "kido-timer:settings:v1";

const DEFAULT_SETTINGS: AppSettings = {
  port: null,
  baud: 9600,
  autoConnect: true,
  demoPath: null,
  demoSpeed: 1,
  hasCompletedSetup: false,
  theme: "light",
  cloudBaseUrl: "https://idr9teznt4.execute-api.eu-central-1.amazonaws.com",
};

export function loadSettings(): AppSettings {
  if (typeof window === "undefined") return DEFAULT_SETTINGS;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const parsed = JSON.parse(raw);
    return { ...DEFAULT_SETTINGS, ...parsed };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(settings: AppSettings): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // ignore quota / disabled storage
  }
}

export function patchSettings(patch: Partial<AppSettings>): AppSettings {
  const next = { ...loadSettings(), ...patch };
  saveSettings(next);
  return next;
}
