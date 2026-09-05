import { useEffect, useState } from "react";
import {
  initProjectRepo,
  projectIsRepo,
  SESSION_TYPES,
  type SessionType,
} from "../lib/ipc";
import { readStored, writeStored } from "../lib/storage";

// ⌘K-style launcher: pick which agent/CLI to open in a new pane. (Atelier's
// command-palette feel; keyboard-first with Escape to dismiss.)
export function SessionLauncher({
  onPick,
  onClose,
  project,
}: {
  onPick: (t: SessionType, isolate: boolean, model?: string) => void;
  onClose: () => void;
  project: string | null;
}) {
  // Isolation is the default — new sessions get their own worktree unless you
  // turn it off, and that choice is remembered.
  const [isolate, setIsolate] = useState(() => {
    try {
      return readStored("isolate") !== "0";
    } catch {
      return true;
    }
  });
  const [isRepo, setIsRepo] = useState<boolean | null>(null);
  const [initError, setInitError] = useState<string | null>(null);
  const [initializing, setInitializing] = useState(false);
  
  // Initialize models state from localStorage once
  const [models, setModels] = useState<Map<string, string>>(() => {
    const initial = new Map<string, string>();
    for (const t of SESSION_TYPES) {
      if (t.modelFlag) {
        const stored = readStored(`model:${t.id}`);
        initial.set(t.id, stored || "");
      }
    }
    return initial;
  });

  // Which CLI was picked without the model it requires, so the row can say so
  // instead of spawning a pane that fails on the backend's guard.
  const [modelMissing, setModelMissing] = useState<string | null>(null);

  const effectiveIsolate = isolate && isRepo !== false;

  function pick(t: SessionType) {
    const model = models.get(t.id)?.trim() || undefined;
    if (t.modelRequired && !model) {
      setModelMissing(t.id);
      return;
    }
    setModelMissing(null);
    onPick(t, effectiveIsolate, model);
  }

  async function runGitInit() {
    if (!project || initializing) return;
    setInitializing(true);
    setInitError(null);
    try {
      await initProjectRepo(project);
      setIsRepo(await projectIsRepo(project));
    } catch (e) {
      setInitError(typeof e === "string" ? e : "git init failed");
    } finally {
      setInitializing(false);
    }
  }

  useEffect(() => {
    try {
      writeStored("isolate", isolate ? "1" : "0");
    } catch {
      /* ignore */
    }
  }, [isolate]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      const n = Number(e.key);
      if (n >= 1 && n <= SESSION_TYPES.length) {
        pick(SESSION_TYPES[n - 1]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // pick closes over onPick, effectiveIsolate and models.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [onPick, onClose, effectiveIsolate, models]);

  // Sync model changes to localStorage when models change
  useEffect(() => {
    for (const [id, value] of models.entries()) {
      try {
        writeStored(`model:${id}`, value);
      } catch {/* ignore */}
    }
  }, [models]);

  return (
    <div className="launcher-backdrop" onClick={onClose}>
      <div className="launcher" onClick={(e) => e.stopPropagation()}>
        <div className="launcher-title">New session</div>
        <div className="launcher-list">
          {SESSION_TYPES.map((t, i) => (
            <button
              key={t.id}
              className="launcher-item"
              onClick={() => pick(t)}
            >
              <span className="dot" style={{ background: t.color }} />
              <span className="ll-label">{t.label}</span>
              <span className="ll-cmd">
                {t.program} {t.args.join(" ")}
                {t.modelFlag && (
                  <span>
                    <input
                      type="text"
                      value={models.get(t.id) || ""}
                      onChange={(e) => {
                        const newModels = new Map(models);
                        newModels.set(t.id, e.target.value);
                        setModels(newModels);
                      }}
                      className="ll-model-input"
                      placeholder={t.modelRequired ? "model required" : undefined}
                    />
                    {models.get(t.id) || ""}
                    {modelMissing === t.id && (
                      <span className="launcher-repo-error" role="alert">
                        {t.label} needs a model: a free OpenRouter id such as
                        openrouter/free, or a local provider's model.
                      </span>
                    )}
                  </span>
                )}
              </span>
              <kbd className="ll-key">{i + 1}</kbd>
            </button>
          ))}
        </div>
        <label className="launcher-opt" onClick={(e) => e.stopPropagation()}>
          <input
            type="checkbox"
            checked={effectiveIsolate}
            disabled={isRepo === false}
            onChange={(e) => setIsolate(e.target.checked)}
          />
          <span>
            <b>Isolate</b> — run in its own git worktree + branch, so this agent
            can't clash with the others' edits
          </span>
        </label>
        {isRepo === false && (
          <div className="launcher-repo-note" role="status">
            <span>Isolation needs a git repo — this folder isn't one.</span>
            <button type="button" onClick={runGitInit} disabled={initializing}>
              {initializing ? "Running git init…" : "Run git init"}
            </button>
            {initError && <span className="launcher-repo-error">{initError}</span>}
          </div>
        )}
        <div className="launcher-hint">
          Press 1–{SESSION_TYPES.length}, or Esc to cancel
        </div>
      </div>
    </div>
  );
}
