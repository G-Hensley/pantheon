import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { SessionLauncher } from "./SessionLauncher";

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
});