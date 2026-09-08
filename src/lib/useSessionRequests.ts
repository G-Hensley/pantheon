// Owns the session-requests slice of app state: the last list snapshot read
// from the backend, and the mutating calls (initialize/approve/deny/reset),
// each ticketed the same way useDispatchBudget tickets conductor_state reads
// — a poll that started before a reset (or before initialize_sessions'
// startup barrier) must never be allowed to overwrite what came after it, no
// matter how late that poll resolves. See useDispatchBudget.ts for the full
// reasoning; the scheme here is the same one, applied to a different list.
import { useCallback, useRef, useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import {
  approveSessionRequest,
  denySessionRequest,
  initializeSessions,
  listSessionRequests,
  resetSessionRequests,
  type Bytes,
  type SessionRequest,
  type SessionRequestList,
} from "./ipc";

const EMPTY: SessionRequestList = {
  requests: [],
  startups: [],
  outstanding: 0,
  admitted: 0,
  outstanding_limit: 0,
  admitted_limit: 0,
  allocator_ready: false,
};

// A channel this session has already handed to the backend for one request,
// plus whatever it has buffered, kept alive across an uncertain outcome. The
// backend may already be treating the approval as committed and writing to
// this exact channel even though this session cannot yet prove that; losing
// the reference here would silently orphan that output and, on a retry,
// hand the backend a second channel for work it already considers done.
type Attachment = { channel: Channel<Bytes>; buffered: Bytes[] };

// An attachment a later snapshot (this session's own regular poll, a
// different request's reconciling read, anything) confirmed as Started,
// without requiring the human to click Approve again. The caller installs
// the pane from `request` (the authoritative record, not anything captured
// before the approval) and must call `consumeReady` once it has, so the same
// attachment is never installed twice.
export type ReadyAttachment = { request: SessionRequest; channel: Channel<Bytes>; buffered: Bytes[] };

// What approving a request hands back to install a pane with: the backend's
// own authoritative record (never anything captured before the approval —
// a repeated or racing approval can settle on a different accepted model
// than this call asked for) plus the channel and whatever it already
// buffered before this pane could exist. A failure carries a message to
// show, not a thrown exception — approve() already had to fold an RPC
// rejection into "poll for the truth" below, and the caller needs the same
// shape either way. `sessionId` is `request.session_id` narrowed to a
// non-null string, since every `ok: true` path already checked that.
export type ApproveResult =
  | { ok: true; request: SessionRequest; sessionId: string; channel: Channel<Bytes>; buffered: Bytes[] }
  | { ok: false; error: string };

export type UseSessionRequests = {
  list: SessionRequestList;
  // Call immediately before starting a list read (each poll tick and each
  // session-requests-changed event); pass the returned ticket to
  // applySnapshot once that read resolves. See useDispatchBudget's beginRead
  // for why a plain in-flight boolean cannot do this job.
  beginRead: () => number;
  applySnapshot: (next: SessionRequestList | null | undefined, ticket: number) => void;
  initialize: (restoredIds: string[]) => Promise<void>;
  approve: (
    requestId: string,
    editedModel: string | null,
    rows: number,
    cols: number,
  ) => Promise<ApproveResult>;
  deny: (requestId: string) => Promise<void>;
  reset: () => Promise<void>;
  // Attachments a passive reconciliation (not a direct approve() return)
  // just confirmed Started. The caller (App.tsx) watches this, installs a
  // pane for each, then calls consumeReady with their request ids so the
  // same attachment is never handed out twice.
  readyAttachments: ReadyAttachment[];
  consumeReady: (requestIds: string[]) => void;
};

export function useSessionRequests(): UseSessionRequests {
  const [list, setList] = useState<SessionRequestList>(EMPTY);
  const [readyAttachments, setReadyAttachments] = useState<ReadyAttachment[]>([]);

  const nextTicket = useRef(1);
  const latestApplied = useRef(0);
  // Keyed by request_id. An entry exists exactly while that request's true
  // outcome is unknown to this session: the approve RPC rejected (or a
  // retry's status check did) and nothing since has confirmed either a
  // successful Started or a terminal failure. Removed the moment either is
  // confirmed, by whichever path notices first — a retry's own status check
  // or a routine snapshot applied for an unrelated reason.
  const pending = useRef(new Map<string, Attachment>());

  const beginRead = useCallback((): number => {
    const ticket = nextTicket.current;
    nextTicket.current += 1;
    return ticket;
  }, []);

  // Reconciles `pending` against a snapshot that just won the ticket race.
  // Every caller of applySnapshot benefits automatically — a regular poll
  // tick is exactly as capable of resolving an uncertain approval as the
  // approve call that created it, which is the point: reconciliation must
  // continue through the same request id regardless of which read notices
  // the change first.
  const reconcilePending = useCallback((next: SessionRequestList) => {
    if (pending.current.size === 0) return;
    const newlyReady: ReadyAttachment[] = [];
    for (const request of next.requests) {
      const attachment = pending.current.get(request.request_id);
      if (!attachment) continue;
      if (request.state === "started" && request.session_id) {
        pending.current.delete(request.request_id);
        newlyReady.push({ request, channel: attachment.channel, buffered: attachment.buffered });
      } else if (request.state === "failed" || request.state === "denied" || request.state === "stale") {
        // Confirmed terminal, non-success: nothing will ever install this
        // channel now, so release it rather than holding it forever.
        pending.current.delete(request.request_id);
      }
      // Still pending/launching: outcome remains unknown, keep waiting.
    }
    if (newlyReady.length > 0) {
      setReadyAttachments((prev) => [...prev, ...newlyReady]);
    }
  }, []);

  const applySnapshot = useCallback(
    (next: SessionRequestList | null | undefined, ticket: number) => {
      if (ticket <= latestApplied.current) return;
      latestApplied.current = ticket;
      const resolved = next ?? EMPTY;
      setList(resolved);
      reconcilePending(resolved);
    },
    [reconcilePending],
  );

  const consumeReady = useCallback((requestIds: string[]) => {
    if (requestIds.length === 0) return;
    setReadyAttachments((prev) => prev.filter((r) => !requestIds.includes(r.request.request_id)));
  }, []);

  const initialize = useCallback(
    async (restoredIds: string[]) => {
      // Invalidates every ticket issued so far, including any regular poll
      // that started before this barrier and has not resolved yet: whatever
      // it eventually returns predates the restore-id admission and must
      // never land over it.
      latestApplied.current = nextTicket.current - 1;
      const ticket = beginRead();
      const snapshot = await initializeSessions(restoredIds);
      applySnapshot(snapshot, ticket);
    },
    [beginRead, applySnapshot],
  );

  const approve = useCallback(
    async (
      requestId: string,
      editedModel: string | null,
      rows: number,
      cols: number,
    ): Promise<ApproveResult> => {
      const existing = pending.current.get(requestId);
      if (existing) {
        // A previous call for this exact request already reserved its
        // attachment and either has not heard back at all yet (its own
        // invoke is still in flight — a double click, or a race between this
        // call and that one) or reached the backend with an outcome this
        // session never confirmed. Either way, this call must not act as a
        // second, independent attempt: an idempotent backend must not spawn a
        // second process for the same request, so this retry only checks
        // where things stand — it never calls approve_session_request again,
        // and it never creates a second channel for a process that may
        // already be writing (or about to be attached) to the first one.
        try {
          const ticket = beginRead();
          const snapshot = await listSessionRequests();
          applySnapshot(snapshot, ticket);
          const found = snapshot.requests.find((r) => r.request_id === requestId);
          if (found?.state === "started" && found.session_id) {
            pending.current.delete(requestId);
            return { ok: true, request: found, sessionId: found.session_id, channel: existing.channel, buffered: existing.buffered };
          }
          if (found && (found.state === "failed" || found.state === "denied" || found.state === "stale")) {
            pending.current.delete(requestId);
            return { ok: false, error: found.detail ?? `Request ended in state "${found.state}".` };
          }
        } catch {
          /* still uncertain; fall through and keep the attachment */
        }
        return {
          ok: false,
          error: "Still waiting to hear back from the backend about this approval; nothing new was submitted.",
        };
      }

      const channel = new Channel<Bytes>();
      const buffered: Bytes[] = [];
      // Assigned synchronously, before the invoke below can round-trip —
      // Tauri's Channel does not buffer against a not-yet-assigned handler,
      // only against out-of-order delivery to an assigned one (verified by
      // reading its source), so a handler set any later would silently lose
      // whatever the backend sent first. TerminalPane takes this over, and
      // whatever landed here in the meantime, once the pane actually mounts.
      channel.onmessage = (msg) => {
        buffered.push(msg);
      };
      // Reserved synchronously, before the first `await` below, not only once
      // the invoke has already rejected. A repeated approve() for this exact
      // request — a double click, a retry racing this call's own still-open
      // promise, anything synchronous with the code above — must see this
      // entry on the `existing` branch above and never reach
      // approve_session_request a second time or build a second channel: the
      // backend's own claim is a one-time atomic transition (Pending ->
      // Launching), so a second caller that reaches it anyway reads back
      // whatever the first caller already claimed, without ever attaching
      // that second caller's channel to anything. Reserving only from the
      // catch block below (the original shape here) left exactly that gap
      // open for the whole time this call's own invoke was still in flight.
      pending.current.set(requestId, { channel, buffered });
      try {
        const updated = await approveSessionRequest(requestId, editedModel, channel, rows, cols);
        // Reconciles the visible list from the approval's own effect, exactly
        // as deny/reset do below. Not awaited: the caller needs the pane
        // installed now, and this ticketed read can only ever win or lose
        // fairly against whatever else is in flight, never corrupt it. Its
        // own try/catch, deliberately separate from the one around this
        // whole function: a failure here is a lost list refresh, not proof
        // the approval itself failed, and must never be routed into the
        // RPC-failure poll-fallback below, which would misreport this
        // request's real, already-known outcome as an unresolved timeout.
        try {
          const ticket = beginRead();
          listSessionRequests().then((snapshot) => applySnapshot(snapshot, ticket)).catch(() => {});
        } catch {
          /* best-effort only; the outcome already determined below stands regardless */
        }
        if (updated.state === "started" && updated.session_id) {
          pending.current.delete(requestId);
          return { ok: true, request: updated, sessionId: updated.session_id, channel, buffered };
        }
        if (updated.state === "failed" || updated.state === "denied" || updated.state === "stale") {
          pending.current.delete(requestId);
          return { ok: false, error: updated.detail ?? `Request ended in state "${updated.state}".` };
        }
        // Resolved without throwing, yet neither Started nor a terminal
        // failure: the backend's own claim is atomic and a resolved (not
        // rejected) response only ever reports Launching/Pending like this
        // when this call's own invoke did not win the claim — an
        // already-Launching request read back its leader's in-progress
        // state, or an already-terminal request read back that instead. This
        // call's own channel was never attached to anything by the backend
        // and never will be, unlike the genuine-RPC-uncertainty case below,
        // so there is nothing here for a later snapshot to legitimately
        // promote. Release the reservation rather than retaining a channel
        // that can only ever be dead.
        pending.current.delete(requestId);
        return {
          ok: false,
          error:
            updated.detail ??
            `This approval was not authorized to attach a session (request is "${updated.state}").`,
        };
      } catch (err) {
        // The RPC failing is not proof the approval failed — it may have
        // landed and only the response been lost. Poll for the authoritative
        // state instead of assuming failure or retrying blind.
        try {
          const ticket = beginRead();
          const snapshot = await listSessionRequests();
          applySnapshot(snapshot, ticket);
          const found = snapshot.requests.find((r) => r.request_id === requestId);
          if (found?.state === "started" && found.session_id) {
            pending.current.delete(requestId);
            return { ok: true, request: found, sessionId: found.session_id, channel, buffered };
          }
          if (found && (found.state === "failed" || found.state === "denied" || found.state === "stale")) {
            pending.current.delete(requestId);
            return { ok: false, error: found.detail ?? `Request ended in state "${found.state}".` };
          }
        } catch {
          /* still uncertain; fall through, keeping the reservation made above */
        }
        // Neither a confirmed Started nor a confirmed terminal failure: the
        // backend may still commit this (the RPC itself failed, so unlike
        // the resolved-but-not-started case above, this call's own channel
        // may genuinely be the one already attached server-side). The
        // reservation from above is left exactly as it is — nothing to
        // re-set — so a later snapshot (this session's own poll, or a manual
        // retry) can still finish the job without losing bytes or invoking a
        // second launch.
        return { ok: false, error: err instanceof Error ? err.message : String(err) };
      }
    },
    [beginRead, applySnapshot],
  );

  const deny = useCallback(
    async (requestId: string) => {
      await denySessionRequest(requestId);
      // deny_session_request answers with only the one request it changed;
      // reconcile counts (outstanding/admitted) from a full list read
      // started only now, after the deny is already committed.
      const ticket = beginRead();
      const snapshot = await listSessionRequests();
      applySnapshot(snapshot, ticket);
    },
    [beginRead, applySnapshot],
  );

  const reset = useCallback(async () => {
    latestApplied.current = nextTicket.current - 1;
    const ticket = beginRead();
    const snapshot = await resetSessionRequests();
    applySnapshot(snapshot, ticket);
  }, [beginRead, applySnapshot]);

  return { list, beginRead, applySnapshot, initialize, approve, deny, reset, readyAttachments, consumeReady };
}
