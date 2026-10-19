import { useCallback, useEffect, useRef, useState } from "react";
import type { UploadOutcome } from "../lib/cloudTypes";
import { t } from "../lib/i18n";
import {
  isUploadBusy,
  performUpload,
  UPLOAD_IDLE,
  uploadReducer,
  uploadSummaryText,
  type UploadAction,
  type UploadState,
} from "../lib/uploadFlow";
import { ConfirmDialog } from "./ConfirmDialog";

interface Props {
  open: boolean;
  /** Changed runs / penalties that will be sent (preview). */
  runsToSend: number;
  penaltiesToSend: number;
  onClose: () => void;
  upload: () => Promise<UploadOutcome>;
  goLive: () => Promise<void>;
}

/** "Ins Web hochladen" dialog (state machine in `lib/uploadFlow.ts`). */
export function UploadDialog({ open, runsToSend, penaltiesToSend, onClose, upload, goLive }: Props) {
  const [state, setState] = useState<UploadState>(UPLOAD_IDLE);
  const dispatch = useCallback((a: UploadAction) => setState((s) => uploadReducer(s, a)), []);
  /** The operator ticked "danach auf Live" for the running upload. */
  const goLiveChosen = useRef(false);

  useEffect(() => {
    if (open && state.step === "idle") dispatch({ type: "open" });
  }, [open, state.step, dispatch]);

  useEffect(() => {
    if (state.step !== "uploading") return;
    goLiveChosen.current = state.goLive;
    let alive = true;
    void performUpload(state, { upload, goLive }).then((next) => {
      if (alive) setState(next);
    });
    return () => {
      alive = false;
    };
    // Runs once per entry into "uploading".
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.step]);

  if (!open || state.step === "idle") return null;

  const close = () => {
    if (isUploadBusy(state)) return;
    dispatch({ type: "close" });
    onClose();
  };

  if (state.step === "done") {
    return (
      <ConfirmDialog title={t.upload.done} confirmLabel={t.upload.close} cancelLabel={t.upload.close} onCancel={close} onConfirm={close}>
        <p className="font-semibold text-ink-50">{uploadSummaryText(state.outcome)}</p>
        {state.wentLive && <p>{t.upload.wentLive}</p>}
        {!state.wentLive && goLiveChosen.current && <p className="text-status-armed">{t.upload.goLiveFailed}</p>}
      </ConfirmDialog>
    );
  }

  if (state.step === "error") {
    return (
      <ConfirmDialog
        title={t.upload.title}
        tone="danger"
        confirmLabel={t.upload.retry}
        cancelLabel={t.upload.close}
        onCancel={close}
        onConfirm={() => dispatch({ type: "retry" })}
      >
        <p className="text-status-unknown">{state.message}</p>
        <p className="text-[12px] text-ink-200">{t.upload.fallback}</p>
      </ConfirmDialog>
    );
  }

  const busy = state.step === "uploading";
  return (
    <ConfirmDialog
      title={t.upload.title}
      confirmLabel={busy ? t.upload.busy : t.upload.start}
      cancelLabel={t.upload.cancel}
      busy={busy}
      onCancel={() => {
        if (busy) return;
        dispatch({ type: "cancel" });
        onClose();
      }}
      onConfirm={() => dispatch({ type: "start" })}
    >
      <p>{t.upload.body}</p>
      <p className="font-mono text-[12px] text-ink-50">{t.upload.summary(runsToSend, penaltiesToSend)}</p>
      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          className="h-4 w-4"
          disabled={busy}
          checked={state.goLive}
          onChange={() => dispatch({ type: "toggleGoLive" })}
        />
        <span>{t.upload.goLive}</span>
      </label>
      <p className="text-[12px] text-ink-200">{t.upload.fallback}</p>
    </ConfirmDialog>
  );
}
