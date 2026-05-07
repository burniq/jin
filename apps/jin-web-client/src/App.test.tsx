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
  sync_targets: [],
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

const settings = {
  public_host: null,
  telegram: {
    bot_token: null,
    bot_token_configured: false,
    default_group_chat_id: null,
  },
  default_sync_targets: [],
};

const factory = {
  id: "factory-1",
  project: "jin",
  title: "Agent content",
  brief: "Create articles",
  mode: "Finite" as const,
  review_policy: "PerStage" as const,
  status: "Draft" as const,
  content_types: ["Text" as const],
  output_path: null,
  schedule: {},
  sync_targets: [],
  stages: [],
  artifacts: [],
  review_bundles: [],
  events: [],
  created_at: "2026-05-07T00:00:00Z",
  updated_at: "2026-05-07T00:00:00Z",
};

function api() {
  return {
    listTools: vi.fn().mockResolvedValue([tool]),
    getSettings: vi.fn().mockResolvedValue(settings),
    updateSettings: vi.fn().mockResolvedValue({ ...settings, public_host: "jin.example.com" }),
    listProjects: vi.fn().mockResolvedValue([{ name: "jin", root: "/tmp/jin" }]),
    listChats: vi.fn().mockResolvedValue([chat]),
    listFactories: vi.fn().mockResolvedValue([factory]),
    createFactory: vi.fn().mockResolvedValue(factory),
    getFactory: vi.fn().mockResolvedValue(factory),
    listFactoryEvents: vi.fn().mockResolvedValue([]),
    pauseFactory: vi.fn().mockResolvedValue({ ...factory, status: "Paused" }),
    resumeFactory: vi.fn().mockResolvedValue({ ...factory, status: "Scheduled" }),
    stopFactory: vi.fn().mockResolvedValue({ ...factory, status: "Stopped" }),
    getProjectContentProfile: vi.fn(),
    updateProjectContentProfile: vi.fn(),
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
        return Promise.resolve(jsonResponse(settings));
      }
      if (url.endsWith("/projects")) {
        return Promise.resolve(jsonResponse([{ name: "jin", root: "/tmp/jin" }]));
      }
      if (url.endsWith("/chats")) {
        return Promise.resolve(jsonResponse([chat]));
      }
      if (url.endsWith("/factories")) {
        return Promise.resolve(jsonResponse([]));
      }
      return Promise.resolve(jsonResponse([]));
    });
    vi.stubGlobal("fetch", fetchMock);

    render(<App />);

    await waitFor(() => expect(screen.getByText("Start a new chat")).toBeInTheDocument());
    await new Promise((resolve) => window.setTimeout(resolve, 50));

    expect(fetchMock).toHaveBeenCalledTimes(5);
  });

  it("renders global settings route", async () => {
    window.history.pushState({}, "", "/settings");
    render(<App api={api()} />);

    await waitFor(() => expect(screen.getByRole("heading", { name: "Jin settings" })).toBeInTheDocument());
    expect(screen.getByLabelText("Public host")).toBeInTheDocument();
  });

  it("renders factories route", async () => {
    window.history.pushState({}, "", "/factories");
    render(<App api={api()} />);

    await waitFor(() => expect(screen.getByRole("heading", { name: "Factories" })).toBeInTheDocument());
    expect(screen.getByRole("button", { name: /Agent content/ })).toBeInTheDocument();
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
