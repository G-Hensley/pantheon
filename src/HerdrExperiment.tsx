import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { TerminalPane } from "./components/TerminalPane";
import "./App.css";

// Built only for the separate experiment profile. No conductor or MCP wiring.
export function HerdrExperiment() {
  const [kind, setKind] = useState("shell");
  const [generation, setGeneration] = useState(0);
  const [status, setStatus] = useState("Connecting to private runtime");
  const [busy, setBusy] = useState(false);
  const [listening, setListening] = useState(false);
  useEffect(() => {
    let mounted = true;
    const listener = listen<string>("herdr-connection", e => setStatus(e.payload))
      .then(stop => { if (mounted) setListening(true); return stop; });
    return () => { mounted = false; void listener.then(stop => stop()); };
  }, []);
  const reconnect = async (next: string) => {
    setBusy(true);
    try {
      await invoke("detach_session", { sessionId: kind });
      setKind(next);
      setGeneration(n => n + 1);
      setStatus("Reconnecting; screen/history repaint replaces the old view");
    } catch (error) { setStatus(String(error)); }
    finally { setBusy(false); }
  };
  return <main style={{ height: "100%", display: "flex", flexDirection: "column", padding: 12 }}>
    <h2>Pantheon | Herdr runtime experiment</h2>
    <p>Private fixture. Closing this window detaches. Coordination is unavailable in this mode.</p>
    <div style={{ display: "flex", gap: 12, paddingBottom: 12 }}>
      <button disabled={busy} onClick={() => void reconnect("shell")}>Shell</button>
      <button disabled={busy} onClick={() => void reconnect("codex")}>Codex (empty profile)</button>
      <button disabled={busy} onClick={() => void reconnect(kind)}>Reattach</button>
      <button disabled={busy} onClick={() => {
        void invoke("kill_session", { sessionId: kind }).catch(e => setStatus(String(e)));
      }}>Terminate this fixture process</button>
    </div>
    <p role="status">{status}</p>
    <div style={{ flex: 1, minHeight: 0, position: "relative" }}>
      {listening && <TerminalPane key={`${kind}-${generation}`} sessionId={kind}
        type={{ id: kind, label: kind, program: kind, args: [], color: "#7aa2f7" }}
        onExit={() => setStatus("Runtime process ended. No automatic restart.")}
        onIsolationChange={() => {}}
        onSpawnError={(_, error) => setStatus(error)} />}
    </div>
  </main>;
}
