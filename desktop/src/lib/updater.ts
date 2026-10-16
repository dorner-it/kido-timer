import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { IS_TAURI } from "./tauri";

export interface CheckResult {
  /** Tauri's Update handle when an update is available, else null. */
  update: Update | null;
  /** Available version string (e.g. "0.2.0") if there is one. */
  availableVersion: string | null;
}

/**
 * Ask Tauri's updater plugin if a newer release exists. Returns null when
 * already up-to-date, when the network/check fails, or when running outside
 * the desktop runtime.
 */
export async function checkForUpdate(): Promise<CheckResult> {
  if (!IS_TAURI) return { update: null, availableVersion: null };
  try {
    const update = await check();
    if (update) {
      return { update, availableVersion: update.version };
    }
    return { update: null, availableVersion: null };
  } catch (err) {
    // The updater throws on network errors, signature failures, malformed
    // manifests, etc. Treat all as "no update" so the operator never sees a
    // surprise error banner mid-event.
    console.warn("Update check failed:", err);
    return { update: null, availableVersion: null };
  }
}

export interface DownloadProgress {
  downloaded: number;
  total: number | null;
}

/**
 * Download + install the given update and relaunch the app. Optionally
 * receives a progress callback.
 */
export async function installAndRelaunch(
  update: Update,
  onProgress?: (p: DownloadProgress) => void,
): Promise<void> {
  let downloaded = 0;
  let total: number | null = null;

  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        total = event.data.contentLength ?? null;
        onProgress?.({ downloaded: 0, total });
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        onProgress?.({ downloaded, total });
        break;
      case "Finished":
        onProgress?.({ downloaded: total ?? downloaded, total });
        break;
    }
  });

  // Windows: the NSIS installer runs and exits the app; relaunch is needed.
  await relaunch();
}
