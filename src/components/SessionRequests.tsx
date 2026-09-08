import { useEffect, useRef, useState } from "react";
import { type SessionRequest, type SessionRequestList, type SessionRequestState } from "../lib/ipc";

type SessionRequestsProps = {
  list: SessionRequestList;
  onApprove: (requestId: string, editedModel: string | null) => void;
  // Every request currently being approved — disables each one's own row
  // rather than the whole panel, so reviewing (or approving) a second
  // request isn't blocked by the first one's in-flight approval, and a
  // second row's approval starting does not make the first row's own
  // in-flight state disappear.
  approvingIds: ReadonlySet<string>;
  approveError: string | null;
  onDeny: (requestId: string) => void;
  onReset: () => void;
  resetPending: boolean;
  onClose: () => void;
};

const OPEN_STATES: ReadonlySet<SessionRequestState> = new Set(["pending", "launching"]);

function stateLabel(state: SessionRequestState): string {
  switch (state) {
    case "pending":
      return "waiting for review";
    case "launching":
      return "launching…";
    case "started":
      return "started";
    case "failed":
      return "failed";
    case "denied":
      return "denied";
    case "stale":
      return "stale";
  }
}

// Review queue for session_request MCP calls: an agent asks for a new
// session, a human approves (optionally editing the model) or denies it
// here. Only those two actions plus Reset — there is no "create a request"
// affordance in this panel; that call is the agent's to make, not the
// human's. Mirrors ConductorBar/TaskDrawer/DispatchDialog conventions:
// backdrop + dialog, Escape closes, no focus trap.
export function SessionRequests({
  list,
  onApprove,
  approvingIds,
  approveError,
  onDeny,
  onReset,
  resetPending,
  onClose,
}: SessionRequestsProps) {
  const closeRef = useRef<HTMLButtonElement>(null);
  // One editable model draft per request, seeded from the request's own
  // model the first time it's shown here and left alone after — a poll
  // landing mid-edit, or the approval itself going through, must never
  // clobber whatever the human has since typed into this field.
  const [modelDrafts, setModelDrafts] = useState<Record<string, string>>({});

  useEffect(() => {
    closeRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  function draftFor(req: SessionRequest): string {
    return modelDrafts[req.request_id] ?? req.model;
  }

  const open = list.requests.filter((r) => OPEN_STATES.has(r.state));
  const history = list.requests.filter((r) => !OPEN_STATES.has(r.state));
  const allocatorReady = list.allocator_ready;

  return (
    <div className="sessreq-backdrop" onClick={onClose}>
      <div className="sessreq" role="dialog" aria-label="Session requests" onClick={(e) => e.stopPropagation()}>
        <div className="sessreq-head">
          <span className="sessreq-title">Session requests</span>
          <div className="spacer" />
          <span className="sessreq-count">
            {list.outstanding}/{list.outstanding_limit} outstanding · {list.admitted}/{list.admitted_limit} admitted
          </span>
          <button ref={closeRef} className="taskdrawer-x" onClick={onClose} title="Close (Esc)" aria-label="Close">
            ✕
          </button>
        </div>

        {!allocatorReady && (
          <div className="sessreq-notready" role="status">
            Restoring session state: approve, deny and reset are disabled until this finishes.
          </div>
        )}
        {approveError && (
          <div className="sessreq-error" role="alert">
            {approveError}
          </div>
        )}

        <div className="sessreq-body">
          {open.length === 0 ? (
            <div className="taskdrawer-empty">No open requests.</div>
          ) : (
            open.map((req) => {
              const draft = draftFor(req);
              const edited = draft.trim() !== req.model;
              const busy = approvingIds.has(req.request_id);
              const flagUnverified = edited || !req.model_verified;
              return (
                <div className="sessreq-item" key={req.request_id}>
                  <div className="sessreq-item-head">
                    <span className="sessreq-kind">{req.kind}</span>
                    <span className="sessreq-state" data-state={req.state}>
                      {stateLabel(req.state)}
                    </span>
                  </div>

                  <label className="sessreq-field">
                    <span>Model</span>
                    <input
                      type="text"
                      value={draft}
                      disabled={busy || !allocatorReady}
                      onChange={(e) =>
                        setModelDrafts((d) => ({ ...d, [req.request_id]: e.target.value }))
                      }
                    />
                  </label>
                  {flagUnverified && (
                    <span className="sessreq-unverified">
                      {edited ? "edited — unverified custom model" : "unverified custom model"}
                    </span>
                  )}

                  <div className="sessreq-meta">
                    <span title="requester">{req.requester}</span>
                    <span title="project">{req.project ?? "no project"}</span>
                    <span title="isolation">{req.isolate ? "isolated" : "not isolated"}</span>
                    <span title="brain">brain: {req.brain}</span>
                  </div>
                  {req.reason && <p className="sessreq-reason">{req.reason}</p>}
                  {req.detail && <p className="sessreq-detail">{req.detail}</p>}

                  <div className="sessreq-actions">
                    <button
                      className="primary"
                      disabled={!allocatorReady || busy}
                      onClick={() => onApprove(req.request_id, edited ? draft.trim() : null)}
                    >
                      {busy ? "Approving…" : "Approve"}
                    </button>
                    <button className="ghost" disabled={!allocatorReady || busy} onClick={() => onDeny(req.request_id)}>
                      Deny
                    </button>
                  </div>
                </div>
              );
            })
          )}

          {history.length > 0 && (
            <div className="sessreq-section">
              <div className="taskdrawer-section-title">History</div>
              {history.map((req) => (
                <div className="sessreq-item sessreq-history" key={req.request_id}>
                  <span className="sessreq-kind">{req.kind}</span>
                  <span className="sessreq-model">{req.model}</span>
                  <span className="sessreq-state" data-state={req.state}>
                    {stateLabel(req.state)}
                  </span>
                  {req.detail && <span className="sessreq-detail">{req.detail}</span>}
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="sessreq-foot">
          {/* Preserve the restore snapshot until initialization finishes.
              Reset changes allowance/history while preserving active requests. */}
          <button className="ghost" disabled={!allocatorReady || resetPending} onClick={onReset}>
            {resetPending ? "Resetting…" : "Reset"}
          </button>
        </div>
      </div>
    </div>
  );
}
