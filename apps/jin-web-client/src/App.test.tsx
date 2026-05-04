import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

const chat = {
  id: "chat-1",
  title: "Existing Chat",
  project: "jin",
  tool: "codex",
  status: "Idle" as const,
  settings: { model: "gpt-5.5" },
  context: { supported: true, used: null, limit: null, label: "Context pending" },
  created_at: "2026-05-04T00:00:00Z",
  updated_at: "2026-05-04T00:00:00Z",
};

const tool = {
  id: "codex",
  name: "Codex",
  supports_persistent_session: true,
  supports_context_meter: true,
  settings: [{ id: "model", label: "Model", kind: "Select" as const, options: ["gpt-5.5"], default: "gpt-5.5" }],
};

function api() {
  return {
    listTools: vi.fn().mockResolvedValue([tool]),
    getSettings: vi.fn().mockResolvedValue({ public_host: null }),
    updateSettings: vi.fn().mockResolvedValue({ public_host: "jin.example.com" }),
    listProjects: vi.fn().mockResolvedValue([{ name: "jin", root: "/tmp/jin" }]),
    listChats: vi.fn().mockResolvedValue([chat]),
    createChat: vi.fn(),
    getChat: vi.fn().mockResolvedValue(chat),
    listMessages: vi.fn().mockResolvedValue({ messages: [], has_more: false }),
    sendMessage: vi.fn().mockResolvedValue([]),
    getChatProgress: vi.fn().mockResolvedValue({ chat, messages: [] }),
    updateChatSettings: vi.fn().mockResolvedValue([]),
    stopChat: vi.fn().mockResolvedValue({ ...chat, status: "Stopped" }),
  };
}

describe("App", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders the new chat route without auto-opening the latest chat", async () => {
    window.history.pushState({}, "", "/");
    render(<App api={api()} />);

    await waitFor(() => expect(screen.getByText("Start a new chat")).toBeInTheDocument());

    expect(screen.queryByRole("heading", { name: "Existing Chat" })).not.toBeInTheDocument();
  });

  it("renders selected chat route", async () => {
    window.history.pushState({}, "", "/chats/chat-1");
    const testApi = api();
    render(<App api={testApi} />);

    await waitFor(() =>
      expect(screen.getByRole("heading", { name: "Existing Chat" })).toBeInTheDocument(),
    );
    expect(testApi.listMessages).toHaveBeenCalledWith("chat-1", { limit: 100 });
    expect(testApi.getChatProgress).toHaveBeenCalledWith("chat-1", { limit: 100 });
  });

  it("renders mobile drawer controls", async () => {
    window.history.pushState({}, "", "/");
    render(<App api={api()} />);

    await waitFor(() => expect(screen.getByLabelText("Open chats")).toBeInTheDocument());
    expect(screen.getByLabelText("Open navigation")).toBeInTheDocument();
  });

  it("does not refetch bootstrap data in a render loop with the default API client", async () => {
    window.history.pushState({}, "", "/");
    const fetchMock = vi.fn((url: string) => {
      if (url.endsWith("/tools")) {
        return Promise.resolve(jsonResponse([tool]));
      }
      if (url.endsWith("/settings")) {
        return Promise.resolve(jsonResponse({ public_host: null }));
      }
      if (url.endsWith("/projects")) {
        return Promise.resolve(jsonResponse([{ name: "jin", root: "/tmp/jin" }]));
      }
      if (url.endsWith("/chats")) {
        return Promise.resolve(jsonResponse([chat]));
      }
      return Promise.resolve(jsonResponse([]));
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);

    await waitFor(() => expect(screen.getByText("Start a new chat")).toBeInTheDocument());
    await new Promise((resolve) => window.setTimeout(resolve, 50));

    expect(fetchMock).toHaveBeenCalledTimes(4);
  });

  it("renders global settings route", async () => {
    window.history.pushState({}, "", "/settings");
    render(<App api={api()} />);

    await waitFor(() => expect(screen.getByRole("heading", { name: "Jin settings" })).toBeInTheDocument());
    expect(screen.getByLabelText("Public host")).toBeInTheDocument();
  });

  it("prefills the new chat project from the route query", async () => {
    window.history.pushState({}, "", "/?project=content");
    const testApi = api();
    testApi.listProjects.mockResolvedValue([
      { name: "jin", root: "/tmp/jin" },
      { name: "content", root: "/tmp/content" },
    ]);

    render(<App api={testApi} />);

    await waitFor(() => expect(screen.getByText("Start a new chat")).toBeInTheDocument());
    expect(screen.getByLabelText("Project")).toHaveValue("content");
  });
});

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}
