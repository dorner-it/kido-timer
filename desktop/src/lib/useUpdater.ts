import { useCallback, useEffect, useRef, useState } from "react";
import type { Update } from "@tauri-apps/plugin-updater";
import { checkForUpdate, installAndRelaunch, type DownloadProgress } from "./updater";

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; version: string; update: Update }
  | { kind: "downloading"; version: string; progress: DownloadProgress }
  | { kind: "installing"; version: string }
  | { kind: "failed"; message: string }
  | { kind: "dismissed"; version: string };

const STARTUP_DELAY_MS = 4000;

export function useUpdater() {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });
  const dismissedRef = useRef<string | null>(null);

  // Run a check shortly after startup so it doesn't compete with app boot.
  useEffect(() => {
    const id = window.setTimeout(() => runCheck(), STARTUP_DELAY_MS);
    return () => window.clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const runCheck = useCallback(async () => {
    setState({ kind: "checking" });
    const { update, availableVersion } = await checkForUpdate();
    if (!update || !availableVersion) {
      setState({ kind: "idle" });
      return;
    }
    if (dismissedRef.current === availableVersion) {
      setState({ kind: "dismissed", version: availableVersion });
      return;
    }
    setState({ kind: "available", version: availableVersion, update });
  }, []);

  const install = useCallback(async () => {
    if (state.kind !== "available") return;
    const { update, version } = state;
    setState({
      kind: "downloading",
      version,
      progress: { downloaded: 0, total: null },
    });
    try {
      await installAndRelaunch(update, (progress) => {
        setState({ kind: "downloading", version, progress });
      });
      // After relaunch the process exits; setting "installing" is mostly
      // a fallback if relaunch hasn't fired yet.
      setState({ kind: "installing", version });
    } catch (err) {
      setState({ kind: "failed", message: String(err) });
    }
  }, [state]);

  const dismiss = useCallback(() => {
    if (state.kind === "available") {
      dismissedRef.current = state.version;
      setState({ kind: "dismissed", version: state.version });
    } else {
      setState({ kind: "idle" });
    }
  }, [state]);

  return { state, runCheck, install, dismiss };
}
