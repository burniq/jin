import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ProjectRecord, ToolDescriptor } from "../api/types";
import { NewChat } from "./NewChat";

const projects: ProjectRecord[] = [
  { name: "jin", root: "/tmp/jin" },
  { name: "content", root: "/tmp/content" },
];
const codex: ToolDescriptor = {
  id: "codex",
  name: "Codex",
  supports_persistent_session: true,
  supports_context_meter: true,
  settings: [
    {
      id: "model",
      label: "Model",
      kind: "Select",
      options: ["gpt-5.5", "gpt-5.4"],
      default: "gpt-5.5",
    },
    {
      id: "reasoning",
      label: "Reasoning",
      kind: "Select",
      options: ["medium", "high"],
      default: "medium",
    },
  ],
};
const shell: ToolDescriptor = {
  id: "shell",
  name: "Shell",
  supports_persistent_session: false,
  supports_context_meter: false,
  settings: [],
};

describe("NewChat", () => {
  it("renders tool-specific model and reasoning controls for Codex", () => {
    render(
      <NewChat
        projects={projects}
        tools={[codex, shell]}
        api={{ createChat: vi.fn(), sendMessage: vi.fn() }}
        onCreated={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Project")).toBeInTheDocument();
    expect(screen.getByLabelText("Tool")).toBeInTheDocument();
    expect(screen.getByLabelText("Model")).toBeInTheDocument();
    expect(screen.getByLabelText("Reasoning")).toBeInTheDocument();
  });

  it("hides Codex-only controls for shell tools", async () => {
    render(
      <NewChat
        projects={projects}
        tools={[shell, codex]}
        api={{ createChat: vi.fn(), sendMessage: vi.fn() }}
        onCreated={vi.fn()}
      />,
    );

    await userEvent.selectOptions(screen.getByLabelText("Tool"), "shell");

    expect(screen.queryByLabelText("Model")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Reasoning")).not.toBeInTheDocument();
  });

  it("creates a chat and sends the first prompt", async () => {
    const api = {
      createChat: vi.fn().mockResolvedValue({
        id: "chat-1",
        title: "New",
        project: "jin",
        tool: "codex",
        status: "Idle",
        settings: {},
        sync_targets: [],
        context: { supported: true, used: null, limit: null, label: "Context pending" },
        created_at: "2026-05-04T00:00:00Z",
        updated_at: "2026-05-04T00:00:00Z",
      }),
      sendMessage: vi.fn().mockResolvedValue([]),
    };
    const onCreated = vi.fn();
    render(<NewChat projects={projects} tools={[codex]} api={api} onCreated={onCreated} />);

    await userEvent.type(screen.getByLabelText("First prompt"), "Fix the UI");
    await userEvent.click(screen.getByRole("button", { name: "Start" }));

    await waitFor(() => expect(api.createChat).toHaveBeenCalled());
    expect(api.createChat).toHaveBeenCalledWith({
      project: "jin",
      tool: "codex",
      title: null,
      settings: { model: "gpt-5.5", reasoning: "medium" },
      sync_targets: [],
    });
    expect(api.sendMessage).toHaveBeenCalledWith("chat-1", { content: "Fix the UI" });
    expect(onCreated).toHaveBeenCalledWith("chat-1");
  });

  it("uses the initial project when creating a chat", async () => {
    const api = {
      createChat: vi.fn().mockResolvedValue({
        id: "chat-1",
        title: "New",
        project: "content",
        tool: "codex",
        status: "Idle",
        settings: {},
        sync_targets: [],
        context: { supported: true, used: null, limit: null, label: "Context pending" },
        created_at: "2026-05-04T00:00:00Z",
        updated_at: "2026-05-04T00:00:00Z",
      }),
      sendMessage: vi.fn().mockResolvedValue([]),
    };
    render(
      <NewChat
        projects={projects}
        tools={[codex]}
        api={api}
        initialProject="content"
        onCreated={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Project")).toHaveValue("content");

    await userEvent.click(screen.getByRole("button", { name: "Start" }));

    await waitFor(() => expect(api.createChat).toHaveBeenCalled());
    expect(api.createChat).toHaveBeenCalledWith(
      expect.objectContaining({ project: "content" }),
    );
  });
});
