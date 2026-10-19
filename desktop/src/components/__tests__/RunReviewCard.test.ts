// Smoke tests for the on-prem review card: "Rückgängig" is shown for
// undoable decisions only and calls the undo action. Rendered with SSR
// (no DOM); written without JSX so the `*.test.ts` pattern picks it up.
import { createElement as h } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { Penalty, Run } from "../../lib/cloudTypes";
import { penalty, RUN_A1, samplePayload } from "../../lib/__tests__/fixtures";
import { PenaltyRow, RunReviewCard, type ReviewActions } from "../RunReviewCard";

const ok = async () => true;
const actions: ReviewActions = {
  reviewPenalty: ok,
  undoReview: ok,
  takeOver: ok,
  setLineup: ok,
  setHandTimes: ok,
  clearHandTimes: ok,
};

function card(
  penalties: Penalty[],
  runPatch: Partial<Run> = {},
  webDecided: ReadonlySet<string> = new Set(),
): string {
  const snapshot = samplePayload();
  snapshot.penalties = penalties;
  const run = { ...snapshot.runs[0]!, ...runPatch };
  snapshot.runs[0] = run;
  return renderToStaticMarkup(
    h(RunReviewCard, { snapshot, run, laneLabel: "Bahn 1", catalogue: new Map(), actions, webDecided }),
  );
}

const decided = (status: "approved" | "rejected", reviewed_at: string | null) =>
  penalty("p1", "sz", status, { run_id: RUN_A1, reviewed_at, reviewed_by: reviewed_at ? "d" : null });

describe("RunReviewCard – Rückgängig", () => {
  it("shown for decided errors before Übernehmen", () => {
    expect(card([decided("approved", "2027-06-12T10:01:00.000Z")])).toContain(">Rückgängig<");
    expect(card([decided("rejected", "2027-06-12T10:01:00.000Z")])).toContain(">Rückgängig<");
  });

  it("hidden for pending errors", () => {
    const html = card([penalty("p1", "sz", "pending", { run_id: RUN_A1 })]);
    expect(html).toContain("Bestätigen");
    expect(html).not.toContain("Rückgängig");
  });

  it("hidden once Übernehmen froze the decision, shown for later ones", () => {
    const taken = { penalties_accepted_at: "2027-06-12T10:05:00.000Z" };
    expect(card([decided("approved", "2027-06-12T10:01:00.000Z")], taken)).not.toContain("Rückgängig");
    expect(card([decided("approved", "2027-06-12T10:05:00.000Z")], taken)).not.toContain("Rückgängig");
    expect(card([decided("rejected", "2027-06-12T10:06:00.000Z")], taken)).toContain(">Rückgängig<");
  });

  it("hidden for decisions from the web snapshot (no error toast, just no button)", () => {
    const html = card([decided("approved", "2027-06-12T10:01:00.000Z")], {}, new Set(["p1"]));
    expect(html).toContain("Bestätigt");
    expect(html).not.toContain("Rückgängig");
  });

  it("click calls onUndo", () => {
    const onUndo = vi.fn();
    const onConfirm = vi.fn();
    const onWithdraw = vi.fn();
    // PenaltyRow uses no hooks → call it and walk the element tree.
    const tree = PenaltyRow({
      penalty: decided("approved", "2027-06-12T10:01:00.000Z"),
      catalogue: new Map(),
      busy: false,
      undoable: true,
      onConfirm,
      onWithdraw,
      onUndo,
    });
    const button = findButton(tree, "Rückgängig");
    expect(button).toBeDefined();
    (button!.props as { onClick: () => void }).onClick();
    expect(onUndo).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
    expect(onWithdraw).not.toHaveBeenCalled();

    const none = PenaltyRow({
      penalty: decided("approved", "2027-06-12T10:01:00.000Z"),
      catalogue: new Map(),
      busy: false,
      undoable: false,
      onConfirm,
      onWithdraw,
      onUndo,
    });
    expect(findButton(none, "Rückgängig")).toBeUndefined();
  });
});

type El = { type: unknown; props: { children?: unknown; [k: string]: unknown } };
function findButton(node: unknown, label: string): El | undefined {
  if (node == null || typeof node !== "object") return undefined;
  if (Array.isArray(node)) {
    for (const n of node) {
      const hit = findButton(n, label);
      if (hit) return hit;
    }
    return undefined;
  }
  const el = node as El;
  if (el.type === "button" && textOf(el.props.children) === label) return el;
  return findButton(el.props?.children, label);
}
function textOf(children: unknown): string {
  if (typeof children === "string") return children;
  if (Array.isArray(children)) return children.map(textOf).join("");
  return "";
}
