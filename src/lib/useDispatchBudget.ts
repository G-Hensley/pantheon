// Owns the dispatch-budget slice of ConductorBar's state: the last snapshot
// read from the backend, and the pending/error state of a human-triggered
// reset. Pulled out of App.tsx so the actual async reset handler (not just
// the props ConductorBar renders) has something narrow to mount tests
// against, without mocking every IPC call App.tsx makes.
import { useCallback, useRef, useState } from "react";
import { conductorState, resetDispatchBudget, type DispatchBudget } from "./ipc";

export type UseDispatchBudget = {
  budget: DispatchBudget | null;
  resetPending: boolean;
  resetError: string | null;
  // Call immediately before starting a conductor_state read (each poll tick
  // and each conductor-changed event), and keep the returned ticket to pass
  // to applySnapshot once that read resolves. A boolean "is a reset in
  // flight" cannot do this job: a read that started before a reset can
  // still resolve arbitrarily late, including after the reset and its own
  // reconciling read have both already finished, and by then there is no
  // in-flight flag left to check. A ticket travels with the read itself, so
  // it can still be recognized as stale no matter when it lands.
  beginRead: () => number;
  applySnapshot: (budget: DispatchBudget | null | undefined, ticket: number) => void;
  reset: () => Promise<void>;
};

export function useDispatchBudget(): UseDispatchBudget {
  const [budget, setBudget] = useState<DispatchBudget | null>(null);
  const [resetPending, setResetPending] = useState(false);
  const [resetError, setResetError] = useState<string | null>(null);

  // Monotonically increasing: every read, ordinary or the reset's own
  // reconciling one, gets a ticket strictly greater than every ticket
  // issued before it started.
  const nextTicket = useRef(1);
  // The ticket of whatever is currently applied to `budget`. A read's
  // result is only ever applied if its ticket is greater than this, and
  // doing so raises this to that ticket. This is what makes staleness a
  // permanent property of a ticket rather than something that stops
  // mattering once a reset "finishes": a ticket issued before the barrier
  // below can never become newer than latestApplied again, no matter how
  // long its own read takes to resolve.
  const latestApplied = useRef(0);

  const beginRead = useCallback(() => {
    const ticket = nextTicket.current;
    nextTicket.current += 1;
    return ticket;
  }, []);

  const applySnapshot = useCallback((next: DispatchBudget | null | undefined, ticket: number) => {
    if (ticket <= latestApplied.current) return;
    latestApplied.current = ticket;
    setBudget(next ?? null);
  }, []);

  const reset = useCallback(async () => {
    // Invalidates every ticket already issued, including ones for reads
    // still in flight right now: whatever they eventually resolve with
    // predates this reset and must never be applied over it, however late
    // it arrives. Reads that get a ticket *after* this point (including the
    // reconciling read below) are still eligible; they will only win if
    // they turn out to be the highest ticket actually applied, which the
    // reconciling read below is guaranteed to be relative to anything that
    // could have started before this line ran.
    latestApplied.current = nextTicket.current - 1;
    setResetPending(true);
    setResetError(null);
    try {
      await resetDispatchBudget();
      // Reconciles from a conductor_state call explicitly started only now,
      // after resetDispatchBudget's own promise has already resolved, i.e.
      // after the backend has committed the reset. That causal ordering is
      // what makes this read trustworthy, not this function's return value
      // (deliberately not used directly) and not any read that happened to
      // start earlier, no matter when any of those resolve relative to this
      // one: its ticket is higher than every ticket issued before this
      // reset began, and applySnapshot enforces that ordering permanently.
      const ticket = beginRead();
      const state = await conductorState();
      applySnapshot(state.dispatch_budget, ticket);
    } catch (err) {
      setResetError(err instanceof Error ? err.message : String(err));
    } finally {
      setResetPending(false);
    }
  }, [applySnapshot, beginRead]);

  return { budget, resetPending, resetError, beginRead, applySnapshot, reset };
}
