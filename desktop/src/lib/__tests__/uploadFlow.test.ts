import { describe, expect, it, vi } from "vitest";
import type { UploadOutcome } from "../cloudTypes";
import {
  friendlyUploadError,
  isUploadBusy,
  performUpload,
  UPLOAD_IDLE,
  uploadAvailability,
  uploadReducer,
  uploadSummaryText,
  type UploadState,
} from "../uploadFlow";
import { samplePayload } from "./fixtures";

const OUTCOME: UploadOutcome = { runsUpdated: 3, penaltiesUpserted: 12, runsSent: 3, penaltiesSent: 12 };

describe("uploadReducer", () => {
  it("happy path idle → confirm → uploading → done → idle", () => {
    let s: UploadState = UPLOAD_IDLE;
    s = uploadReducer(s, { type: "open" });
    expect(s).toEqual({ step: "confirm", goLive: false });
    s = uploadReducer(s, { type: "toggleGoLive" });
    expect(s).toEqual({ step: "confirm", goLive: true });
    s = uploadReducer(s, { type: "start" });
    expect(s).toEqual({ step: "uploading", goLive: true });
    expect(isUploadBusy(s)).toBe(true);
    s = uploadReducer(s, { type: "succeeded", outcome: OUTCOME });
    expect(s).toEqual({ step: "done", outcome: OUTCOME, wentLive: true });
    s = uploadReducer(s, { type: "close" });
    expect(s).toEqual(UPLOAD_IDLE);
  });

  it("failure → error → retry keeps goLive", () => {
    let s: UploadState = { step: "uploading", goLive: true };
    s = uploadReducer(s, { type: "failed", message: "x" });
    expect(s).toEqual({ step: "error", message: "x", goLive: true });
    s = uploadReducer(s, { type: "retry" });
    expect(s).toEqual({ step: "uploading", goLive: true });
  });

  it("cannot close or cancel while uploading", () => {
    const s: UploadState = { step: "uploading", goLive: false };
    expect(uploadReducer(s, { type: "close" })).toBe(s);
    expect(uploadReducer(s, { type: "cancel" })).toBe(s);
    expect(uploadReducer(s, { type: "open" })).toBe(s);
  });

  it("ignores actions in the wrong state", () => {
    expect(uploadReducer(UPLOAD_IDLE, { type: "start" })).toBe(UPLOAD_IDLE);
    expect(uploadReducer(UPLOAD_IDLE, { type: "succeeded", outcome: OUTCOME })).toBe(UPLOAD_IDLE);
    expect(uploadReducer(UPLOAD_IDLE, { type: "toggleGoLive" })).toBe(UPLOAD_IDLE);
    expect(uploadReducer(UPLOAD_IDLE, { type: "retry" })).toBe(UPLOAD_IDLE);
    const confirm: UploadState = { step: "confirm", goLive: false };
    expect(uploadReducer(confirm, { type: "failed", message: "x" })).toBe(confirm);
    expect(uploadReducer(confirm, { type: "cancel" })).toEqual(UPLOAD_IDLE);
  });

  it("open with goLive preset, reopen after error", () => {
    expect(uploadReducer(UPLOAD_IDLE, { type: "open", goLive: true })).toEqual({
      step: "confirm",
      goLive: true,
    });
    const err: UploadState = { step: "error", message: "x", goLive: false };
    expect(uploadReducer(err, { type: "open" }).step).toBe("confirm");
  });
});

describe("performUpload", () => {
  it("uploads and switches to live when chosen", async () => {
    const upload = vi.fn().mockResolvedValue(OUTCOME);
    const goLive = vi.fn().mockResolvedValue(undefined);
    const next = await performUpload({ step: "uploading", goLive: true }, { upload, goLive });
    expect(next).toEqual({ step: "done", outcome: OUTCOME, wentLive: true });
    expect(upload).toHaveBeenCalledTimes(1);
    expect(goLive).toHaveBeenCalledTimes(1);
  });

  it("does not switch to live unless chosen", async () => {
    const goLive = vi.fn();
    const next = await performUpload(
      { step: "uploading", goLive: false },
      { upload: async () => OUTCOME, goLive },
    );
    expect(next).toEqual({ step: "done", outcome: OUTCOME, wentLive: false });
    expect(goLive).not.toHaveBeenCalled();
  });

  it("upload error → error state with a friendly message, no live switch", async () => {
    const goLive = vi.fn();
    const next = await performUpload(
      { step: "uploading", goLive: true },
      {
        upload: async () => {
          throw "upload failed: error sending request";
        },
        goLive,
      },
    );
    expect(next.step).toBe("error");
    if (next.step === "error") expect(next.message).toContain("Keine Verbindung");
    expect(goLive).not.toHaveBeenCalled();
  });

  it("live switch failure after upload is still a success", async () => {
    const next = await performUpload(
      { step: "uploading", goLive: true },
      {
        upload: async () => OUTCOME,
        goLive: async () => {
          throw new Error("offline");
        },
      },
    );
    expect(next).toEqual({ step: "done", outcome: OUTCOME, wentLive: false });
  });

  it("no-op outside the uploading state", async () => {
    const upload = vi.fn();
    const s: UploadState = { step: "confirm", goLive: false };
    expect(await performUpload(s, { upload, goLive: vi.fn() })).toBe(s);
    expect(upload).not.toHaveBeenCalled();
  });
});

describe("helpers", () => {
  it("availability", () => {
    expect(uploadAvailability({ paired: true, snapshot: samplePayload() })).toEqual({ ok: true });
    const np = uploadAvailability({ paired: false, snapshot: samplePayload() });
    expect(np.ok).toBe(false);
    if (!np.ok) expect(np.reason).toContain("KiDo-Datei");
    expect(uploadAvailability({ paired: true, snapshot: null }).ok).toBe(false);
  });

  it("friendly errors", () => {
    expect(friendlyUploadError("unauthorized: bad key")).toContain("API-Schlüssel");
    expect(friendlyUploadError("Error: not_found: discipline")).toContain("nicht gefunden");
    expect(friendlyUploadError("forbidden: x")).toContain("Berechtigung");
    expect(friendlyUploadError("request failed: dns error")).toContain("Keine Verbindung");
    expect(friendlyUploadError("http 400: validation: code")).toContain("abgelehnt");
    expect(friendlyUploadError("http 500: boom")).toBe("Hochladen fehlgeschlagen: http 500: boom");
  });

  it("summary text", () => {
    expect(uploadSummaryText(OUTCOME)).toBe("3 Läufe und 12 Fehler hochgeladen.");
    expect(uploadSummaryText({ ...OUTCOME, runsUpdated: 1, penaltiesUpserted: 1 })).toBe(
      "1 Lauf und 1 Fehler hochgeladen.",
    );
  });
});
