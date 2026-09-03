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