import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChatSession, ProjectRecord } from "../api/types";
import { AppShell } from "./AppShell";

const projects: ProjectRecord[] = [
  { name: "jin", root: "/tmp/jin" },
  { name: "empty", root: "/tmp/empty" },
];

const chats: ChatSession[] = [
  {
    id: "chat-1",
    title: "Existing Chat",
    project: "jin",
    tool: "codex",
    status: "Idle",
    settings: {},
    context: { supported: false, used: null, limit: null, label: "Context unavailable" },
    created_at: "2026-05-04T00:00:00Z",
    updated_at: "2026-05-04T00:00:00Z",
  },
];

describe("AppShell", () => {
  afterEach(() => {
    window.localStorage.clear();
  });

  it("groups chat sessions under their projects", () => {
    render(
      <AppShell chats={chats} projects={projects} onNavigate={vi.fn()}>
        <div>Main</div>
      </AppShell>,
    );

    expect(screen.getByText("/tmp/jin")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Existing Chat/ })).toBeInTheDocument();
    expect(screen.getByText("/tmp/empty")).toBeInTheDocument();
    expect(screen.getByText("No chats yet")).toBeInTheDocument();
  });

  it("collapses projects from the project row and persists the state", async () => {
    const { unmount } = render(
      <AppShell chats={chats} projects={projects} onNavigate={vi.fn()}>
        <div>Main</div>
      </AppShell>,
    );

    await userEvent.click(screen.getByRole("button", { name: /\/tmp\/jin/ }));

    expect(screen.queryByRole("button", { name: /Existing Chat/ })).not.toBeInTheDocument();

    unmount();
    render(
      <AppShell chats={chats} projects={projects} onNavigate={vi.fn()}>
        <div>Main</div>
      </AppShell>,
    );

    expect(screen.queryByRole("button", { name: /Existing Chat/ })).not.toBeInTheDocument();
  });

  it("opens a new-chat route for a project without toggling collapse", async () => {
    const onNavigate = vi.fn();
    render(
      <AppShell chats={chats} projects={projects} onNavigate={onNavigate}>
        <div>Main</div>
      </AppShell>,
    );

    await userEvent.click(screen.getByRole("button", { name: "New chat in jin" }));

    expect(onNavigate).toHaveBeenCalledWith("/?project=jin");
    expect(screen.getByRole("button", { name: /Existing Chat/ })).toBeInTheDocument();
  });
});
