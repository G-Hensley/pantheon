import { useEffect, useState } from "react";
import {
  initProjectRepo,
  listModels,
  projectIsRepo,
  SESSION_TYPES,
  type SessionType,
} from "../lib/ipc";
import { readStored, writeStored } from "../lib/storage";

type ModelChoice = { value: string; label: string };

// Sentinel <option> value for "type your own id"; distinct from any real
// model id so a select's value can tell the two apart.
const CUSTOM_MODEL = "__custom__";

// Small, fixed option sets for the CLIs that don't have a list command.
// claude's are the aliases `claude --help` documents for --model; codex's
// "Config default" (empty value, no -m flag) reproduces what an empty model
// already meant before this picker existed: Codex falls back to its own
// ~/.codex/config.toml. opencode has no entry here: its options come from
// `list_models` instead.
const STATIC_MODEL_OPTIONS: Record<string, ModelChoice[]> = {
  claude: [
    { value: "fable", label: "Fable 5.1" },
    { value: "opus", label: "Opus 5" },
    { value: "sonnet", label: "Sonnet 5" },
  ],
  codex: [{ value: "", label: "Config default" }],
};

// Group opencode's model ids by provider prefix ("openrouter/gpt-4o" ->
// "openrouter") for the select's <optgroup>s. An id with no "/" groups under
// "other" rather than being dropped.
function groupByProvider(ids: string[]): Array<[string, string[]]> {
  const groups = new Map<string, string[]>();
  for (const id of ids) {
    const slash = id.indexOf("/");
    const provider = slash === -1 ? "other" : id.slice(0, slash);
    const group = groups.get(provider);
    if (group) {
      group.push(id);
    } else {
      groups.set(provider, [id]);
    }
  }
  return [...groups.entries()];
}

// The model row for one CLI: a styled select over its known options, plus a
// "Custom..." choice that reveals a styled text input for an id not in the
// list. opencode's options are fetched once per launcher open; claude's and
// codex's are static and available immediately.
function ModelPicker({
  type,
  value,
  onChange,
  missing,
}: {
  type: SessionType;
  value: string;
  onChange: (v: string) => void;
  missing: boolean;
}) {
  const staticOptions = STATIC_MODEL_OPTIONS[type.id];
  const isDynamic = staticOptions === undefined;

  const [dynamicOptions, setDynamicOptions] = useState<string[] | null>(null);
  const [loading, setLoading] = useState(isDynamic);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Preselect the remembered value if it's one of the known options, else
  // start in Custom mode with it filled in. For a dynamic CLI the options
  // aren't known yet at mount, so this starts as Custom and the effect below
  // corrects it once the list arrives.
  const [customMode, setCustomMode] = useState<boolean>(() =>
    staticOptions ? !staticOptions.some((o) => o.value === value) : true,
  );

  useEffect(() => {
    if (!isDynamic) return;
    let cancelled = false;
    setLoading(true);
    setLoadError(null);
    listModels(type.program)
      .then((models) => {
        if (cancelled) return;
        setDynamicOptions(models);
      })
      .catch((e) => {
        if (cancelled) return;
        setLoadError(
          typeof e === "string"
            ? e
            : e instanceof Error
              ? e.message
              : "failed to list models",
        );
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // Fetch once per launcher open (on mount), not on every keystroke.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [type.program]);

  useEffect(() => {
    if (dynamicOptions && value !== "" && dynamicOptions.includes(value)) {
      setCustomMode(false);
    }
    // Only re-check once the list itself changes, not on every value edit.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dynamicOptions]);

  const selectableOptions = isDynamic ? dynamicOptions : staticOptions;

  return (
    // The row's whole label area is a <button onClick={pick}>, so without
    // this a click that opens the select, chooses an option, or focuses the
    // custom input would bubble up and launch the session before the
    // operator finished choosing a model.
    <span className="ll-model-field" onClick={(e) => e.stopPropagation()}>
      {selectableOptions && (
        <select
          className="ll-model-select"
          value={customMode ? CUSTOM_MODEL : value}
          onChange={(e) => {
            const v = e.target.value;
            if (v === CUSTOM_MODEL) {
              setCustomMode(true);
            } else {
              setCustomMode(false);
              onChange(v);
            }
          }}
        >
          {isDynamic
            ? groupByProvider(selectableOptions as string[]).map(
                ([provider, ids]) => (
                  <optgroup key={provider} label={provider}>
                    {ids.map((id) => (
                      <option key={id} value={id}>
                        {id}
                      </option>
                    ))}
                  </optgroup>
                ),
              )
            : (selectableOptions as ModelChoice[]).map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
          <option value={CUSTOM_MODEL}>Custom…</option>
        </select>
      )}
      {isDynamic && loading && (
        <span className="ll-model-loading">loading…</span>
      )}
      {isDynamic && loadError && (
        <span className="launcher-repo-error" role="alert">
          {loadError}
        </span>
      )}
      {customMode && (
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="ll-model-input"
          placeholder={type.modelRequired ? "model required" : undefined}
        />
      )}
      {missing && (
        <span className="launcher-repo-error" role="alert">
          {type.label} needs a model: a free OpenRouter id such as
          openrouter/free, or a local provider's model.
        </span>
      )}
    </span>
  );
}

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
      } catch {
        /* ignore */
      }
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
                  <ModelPicker
                    type={t}
                    value={models.get(t.id) || ""}
                    onChange={(v) => {
                      const newModels = new Map(models);
                      newModels.set(t.id, v);
                      setModels(newModels);
                    }}
                    missing={modelMissing === t.id}
                  />
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
