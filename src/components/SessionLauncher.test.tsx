import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { SessionLauncher } from "./SessionLauncher";
import { listModels } from "../lib/ipc";

// vi.mock's factory is hoisted above any top-level `let`, so the reference it
// closes over has to be created with vi.hoisted instead, or assigning into it
// below throws a TDZ error at import time.
const hoisted = vi.hoisted(() => ({
  actualListModels: undefined as unknown as typeof listModels,
}));

vi.mock("../lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/ipc")>();
  hoisted.actualListModels = actual.listModels;
  return {
    ...actual,
    listModels: vi.fn(actual.listModels),
  };
});

// codex and opencode are both dynamic CLIs, so every render of the launcher
// calls listModels for both of them. mockResolvedValueOnce/mockRejectedValueOnce
// queue against whichever call happens *next*, with no regard for its
// argument, and codex's row always mounts (and so calls listModels) before
// opencode's, so a plain "once" set up for one program can be consumed by
// the other's call instead. Routing by argument avoids that, and resetting
// between tests keeps one test's queued behaviour from leaking into the next
// (there is no global clearMocks/restoreMocks in vitest.config.ts).
function mockListModelsFor(program: string, result: string[] | string) {
  vi.mocked(listModels).mockImplementation((p) => {
    if (p !== program) return hoisted.actualListModels(p);
    return typeof result === "string"
      ? Promise.reject(result)
      : Promise.resolve(result);
  });
}

afterEach(() => {
  vi.mocked(listModels).mockReset();
  vi.mocked(listModels).mockImplementation(hoisted.actualListModels);
});

describe("<SessionLauncher>", () => {
  it("passes the typed model to onPick when a CLI has a model flag", () => {
    const onPick = vi.fn();
    render(
      <SessionLauncher onPick={onPick} onClose={() => {}} project={null} />,
    );
    const claudeItem = screen.getByText("Claude Code").closest(".launcher-item")!;
    const input = claudeItem.querySelector("input") as HTMLInputElement;
    expect(input).toBeTruthy();
    fireEvent.change(input, { target: { value: "openrouter/free" } });
    fireEvent.click(claudeItem);
    expect(onPick).toHaveBeenCalledWith(
      expect.objectContaining({ id: "claude" }),
      expect.anything(),
      "openrouter/free",
    );
  });

  it("refuses to pick opencode without a model and says why", () => {
    const onPick = vi.fn();
    render(
      <SessionLauncher onPick={onPick} onClose={() => {}} project={null} />,
    );
    const item = screen
      .getByText("opencode", { selector: ".ll-label" })
      .closest(".launcher-item")!;
    const input = item.querySelector("input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "  " } });
    fireEvent.click(item);
    expect(onPick).not.toHaveBeenCalled();
    expect(screen.getByRole("alert").textContent).toContain("needs a model");
    // The hotkey path goes through the same check.
    fireEvent.keyDown(window, { key: "4" });
    expect(onPick).not.toHaveBeenCalled();
    // With a model it launches, and the model is trimmed.
    fireEvent.change(input, { target: { value: " openrouter/free " } });
    fireEvent.click(item);
    expect(onPick).toHaveBeenCalledWith(
      expect.objectContaining({ id: "opencode" }),
      expect.anything(),
      "openrouter/free",
    );
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("does not render a model input for a CLI without a model flag", () => {
    const onPick = vi.fn();
    render(
      <SessionLauncher onPick={onPick} onClose={() => {}} project={null} />,
    );
    const shellItem = screen.getByText("Shell").closest(".launcher-item")!;
    expect(shellItem.querySelector("input")).toBeNull();
    fireEvent.click(shellItem);
    expect(onPick).toHaveBeenCalledWith(
      expect.objectContaining({ id: "shell" }),
      expect.anything(),
      undefined,
    );
  });

  it("renders the documented static options for claude", () => {
    render(
      <SessionLauncher onPick={vi.fn()} onClose={() => {}} project={null} />,
    );
    const claudeItem = screen.getByText("Claude Code").closest(".launcher-item")!;
    const claudeSelect = claudeItem.querySelector("select") as HTMLSelectElement;
    expect(Array.from(claudeSelect.options).map((o) => o.textContent)).toEqual([
      "Fable 5.1",
      "Opus 5",
      "Sonnet 5",
      "Custom…",
    ]);
  });

  it("shows Config default and Custom for codex before its model fetch resolves", () => {
    // codex is a dynamic CLI like opencode, but its leading options
    // ("Config default") must be there immediately, not only once the fetch
    // resolves, so the select is never empty while codex debug models runs.
    render(
      <SessionLauncher onPick={vi.fn()} onClose={() => {}} project={null} />,
    );
    const codexItem = screen.getByText("Codex").closest(".launcher-item")!;
    const codexSelect = codexItem.querySelector("select") as HTMLSelectElement;
    expect(Array.from(codexSelect.options).map((o) => o.textContent)).toEqual([
      "Config default",
      "Custom…",
    ]);
    // Config default is the empty-value option, and nothing is remembered
    // yet, so it's preselected rather than falling to Custom.
    expect(codexSelect.value).toBe("");
    expect(codexItem.querySelector("input")).toBeNull();
  });

  it("populates the codex select from the mocked list_models result", async () => {
    mockListModelsFor("codex", ["gpt-6-astra", "gpt-5.6-sol"]);
    render(
      <SessionLauncher onPick={vi.fn()} onClose={() => {}} project={null} />,
    );
    const codexItem = screen
      .getByText("Codex")
      .closest(".launcher-item") as HTMLElement;
    const codexCalls = vi
      .mocked(listModels)
      .mock.calls.filter(([program]) => program === "codex");
    expect(codexCalls).toHaveLength(1);

    const select = codexItem.querySelector("select") as HTMLSelectElement;
    await within(codexItem).findByText("gpt-6-astra", { selector: "option" });
    expect(Array.from(select.options).map((o) => o.textContent)).toEqual([
      "Config default",
      "gpt-6-astra",
      "gpt-5.6-sol",
      "Custom…",
    ]);
    // Codex slugs have no "/", so they render as plain options, not grouped
    // under an <optgroup> the way opencode's ids are.
    expect(select.querySelector("optgroup")).toBeNull();
  });

  it("still shows Config default and Custom for codex when the model fetch fails", async () => {
    mockListModelsFor("codex", "codex debug models failed");
    render(
      <SessionLauncher onPick={vi.fn()} onClose={() => {}} project={null} />,
    );
    const codexItem = screen
      .getByText("Codex")
      .closest(".launcher-item") as HTMLElement;
    const select = codexItem.querySelector("select") as HTMLSelectElement;
    // Present immediately, and still present once the rejection lands.
    expect(Array.from(select.options).map((o) => o.textContent)).toEqual([
      "Config default",
      "Custom…",
    ]);
    const alert = await within(codexItem).findByRole("alert");
    expect(alert.textContent).toBe("codex debug models failed");
    expect(Array.from(select.options).map((o) => o.textContent)).toEqual([
      "Config default",
      "Custom…",
    ]);
  });

  it("clicking or editing the model controls does not launch the row", () => {
    // The row's label area is a <button onClick={pick}>, so a click that
    // opens the select, or lands on the custom input, must not bubble up
    // and launch the session before the operator has finished choosing.
    const onPick = vi.fn();
    render(
      <SessionLauncher onPick={onPick} onClose={() => {}} project={null} />,
    );
    const codexItem = screen.getByText("Codex").closest(".launcher-item")!;
    const select = codexItem.querySelector("select") as HTMLSelectElement;

    fireEvent.click(select);
    expect(onPick).not.toHaveBeenCalled();

    const customValue = Array.from(select.options).find(
      (o) => o.textContent === "Custom…",
    )!.value;
    fireEvent.change(select, { target: { value: customValue } });
    expect(onPick).not.toHaveBeenCalled();

    const input = codexItem.querySelector("input") as HTMLInputElement;
    fireEvent.click(input);
    fireEvent.change(input, { target: { value: "gpt-4o-custom" } });
    expect(onPick).not.toHaveBeenCalled();

    // An explicit click on the rest of the row still launches, with the
    // value picked while none of the above fired it.
    fireEvent.click(screen.getByText("Codex"));
    expect(onPick).toHaveBeenCalledWith(
      expect.objectContaining({ id: "codex" }),
      expect.anything(),
      "gpt-4o-custom",
    );
  });

  it("choosing Custom reveals the text input", () => {
    render(
      <SessionLauncher onPick={vi.fn()} onClose={() => {}} project={null} />,
    );
    const codexItem = screen.getByText("Codex").closest(".launcher-item")!;
    const codexSelect = codexItem.querySelector("select") as HTMLSelectElement;
    expect(codexItem.querySelector("input")).toBeNull();

    const customValue = Array.from(codexSelect.options).find(
      (o) => o.textContent === "Custom…",
    )!.value;
    fireEvent.change(codexSelect, { target: { value: customValue } });

    const input = codexItem.querySelector("input") as HTMLInputElement;
    expect(input).toBeTruthy();
    fireEvent.change(input, { target: { value: "gpt-5.6-sol" } });
    expect(input).toHaveValue("gpt-5.6-sol");
  });

  it("populates the opencode select from the mocked list_models result", async () => {
    mockListModelsFor("opencode", [
      "openrouter/free",
      "opencode/gpt-oss-120b-free",
      "llama-3.1-8b",
    ]);
    render(
      <SessionLauncher onPick={vi.fn()} onClose={() => {}} project={null} />,
    );
    const item = screen
      .getByText("opencode", { selector: ".ll-label" })
      .closest(".launcher-item") as HTMLElement;
    expect(item.querySelector("select")).toBeNull();
    expect(listModels).toHaveBeenCalledWith("opencode");

    const select = await within(item).findByRole("combobox");
    const groups = Array.from((select as HTMLSelectElement).children).filter(
      (el): el is HTMLOptGroupElement => el.tagName === "OPTGROUP",
    );
    expect(groups.map((g) => g.label).sort()).toEqual(
      ["other", "opencode", "openrouter"].sort(),
    );
    const allIds = groups.flatMap((g) =>
      Array.from(g.children).map((o) => (o as HTMLOptionElement).value),
    );
    expect(allIds.sort()).toEqual(
      ["openrouter/free", "opencode/gpt-oss-120b-free", "llama-3.1-8b"].sort(),
    );
  });
});
