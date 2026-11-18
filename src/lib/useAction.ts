import { useCallback, useRef, useState } from "react";
import { errorText } from "./api";
import type { ActionResult } from "./types";

export interface ActionState {
  /** Name of the action in flight, or null. */
  pending: string | null;
  /** Last result, kept until replaced so the operator can read it. */
  note: { ok: boolean; text: string; at: number } | null;
  dismiss: () => void;
}

export type RunAction = (name: string, action: () => Promise<ActionResult>) => Promise<void>;

/**
 * Runs a backend action, keeps one in flight at a time and reports the outcome.
 *
 * The refresh is deliberate: some actions complete asynchronously (a stop waits
 * for the process to go away), and an immediate sample is what makes the click
 * visibly do something even when the answer is "already running".
 */
export function useAction(onDone: () => void) {
  const [pending, setPending] = useState<string | null>(null);
  const [note, setNote] = useState<ActionState["note"]>(null);
  const inFlight = useRef(false);

  const run = useCallback(
    async (name: string, action: () => Promise<ActionResult>) => {
      if (inFlight.current) return;
      inFlight.current = true;
      setPending(name);
      try {
        const result = await action();
        setNote({ ok: result.ok, text: result.message, at: Date.now() });
      } catch (error) {
        setNote({ ok: false, text: errorText(error), at: Date.now() });
      } finally {
        inFlight.current = false;
        setPending(null);
        onDone();
      }
    },
    [onDone],
  );

  const dismiss = useCallback(() => setNote(null), []);

  return { pending, note, run, dismiss };
}
