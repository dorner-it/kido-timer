import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import type { ConnectionEvent } from "./types";

export const KIDO_EVENT = "kido://event";

/**
 * True when running inside the Tauri webview. Outside of Tauri (plain browser
 * `vite preview` for visual checks) the app degrades gracefully instead of
 * throwing on every command.
 */
export const IS_TAURI =
  typeof window !== "undefined" &&
  ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

export async function listSerialPorts(): Promise<string[]> {
  if (!IS_TAURI) return [];
  return invoke<string[]>("list_serial_ports");
}

export async function connectSerial(port: string, baud: number): Promise<void> {
  if (!IS_TAURI) throw new Error("Serial port access requires the desktop app");
  await invoke("connect_serial", { port, baud });
}

export async function startDemo(path: string, speed: number): Promise<void> {
  if (!IS_TAURI) throw new Error("Demo replay requires the desktop app");
  await invoke("start_demo", { path, speed });
}

export async function disconnect(): Promise<void> {
  if (!IS_TAURI) return;
  await invoke("disconnect");
}

export async function sendReset(): Promise<void> {
  if (!IS_TAURI) throw new Error("Device reset requires the desktop app");
  await invoke("send_reset");
}

export async function currentSource(): Promise<string | null> {
  if (!IS_TAURI) return null;
  return invoke<string | null>("current_source");
}

export async function pickHexDumpFile(): Promise<string | null> {
  if (!IS_TAURI) return null;
  const result = await openDialog({
    multiple: false,
    directory: false,
    title: "Select a hex-dump capture (.txt)",
    filters: [
      { name: "Hex dump", extensions: ["txt", "log"] },
      { name: "All files", extensions: ["*"] },
    ],
  });
  if (typeof result === "string") return result;
  return null;
}

export async function listenConnectionEvents(
  handler: (event: ConnectionEvent) => void,
): Promise<UnlistenFn> {
  if (!IS_TAURI) return () => {};
  return listen<ConnectionEvent>(KIDO_EVENT, (e) => handler(e.payload));
}

/**
 * Show a save dialog and write the given text to the chosen path. Returns the
 * written path on success, `null` if the user cancelled. In a plain browser
 * (no Tauri runtime) it falls back to a Blob download so dev preview works.
 */
export async function saveTextFile(
  defaultName: string,
  content: string,
  filters: { name: string; extensions: string[] }[] = [],
): Promise<string | null> {
  if (!IS_TAURI) {
    const blob = new Blob([content], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = defaultName;
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return defaultName;
  }
  const path = await saveDialog({ defaultPath: defaultName, filters });
  if (typeof path !== "string") return null;
  await invoke("write_text_file", { path, contents: content });
  return path;
}
