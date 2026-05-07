import { afterEach, describe, expect, it, vi } from "vitest";
import { createJinApiClient } from "./client";

const sampleChat = {
  id: "chat-1",
  title: "Jin",
  project: "jin",
  tool: "codex",
  status: "Idle",
  settings: { model: "gpt-5.5" },
  sync_targets: [],
  context: {
    supported: true,
    used: null,
    limit: null,
    label: "Context pending",
  },
  created_at: "2026-05-04T00:00:00Z",
  updated_at: "2026-05-04T00:00:00Z",
};

describe("createJinApiClient", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("posts chat creation payload as JSON", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(sampleChat), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const client = createJinApiClient("/api");
    const chat = await client.createChat({
      project: "jin",
      tool: "codex",
      title: null,
      settings: { model: "gpt-5.5", reasoning: "high" },
    });

    expect(chat.id).toBe("chat-1");
    expect(fetchMock).toHaveBeenCalledWith("/api/chats", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        project: "jin",
        tool: "codex",
        title: null,
        settings: { model: "gpt-5.5", reasoning: "high" },
      }),
    });
  });

  it("throws backend error messages", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ error: "bad project" }), {
          status: 400,
          headers: { "content-type": "application/json" },
        }),
      ),
    );

    const client = createJinApiClient("/api");

    await expect(client.listProjects()).rejects.toThrow("bad project");
  });

  it("updates global Jin settings", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          public_host: "jin.example.com",
          telegram: {
            bot_token: null,
            bot_token_configured: true,
            default_group_chat_id: "-10010",
          },
          default_sync_targets: [],
        }),
        {
        status: 200,
        headers: { "content-type": "application/json" },
        },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    const client = createJinApiClient("/api");
    const settings = await client.updateSettings({
      public_host: "jin.example.com",
      telegram: {
        bot_token: "secret-token",
        bot_token_configured: false,
        default_group_chat_id: "-10010",
      },
      default_sync_targets: [],
    });

    expect(settings.public_host).toBe("jin.example.com");
    expect(settings.telegram.bot_token).toBeNull();
    expect(settings.telegram.bot_token_configured).toBe(true);
    expect(fetchMock).toHaveBeenCalledWith("/api/settings", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        public_host: "jin.example.com",
        telegram: {
          bot_token: "secret-token",
          bot_token_configured: false,
          default_group_chat_id: "-10010",
        },
        default_sync_targets: [],
      }),
    });
  });

  it("creates and controls factory pipelines", async () => {
    const pipeline = {
      id: "factory-1",
      project: "jin",
      title: "Agent content",
      brief: "Create article drafts",
      mode: "Finite",
      review_policy: "PerStage",
      status: "Draft",
      content_types: ["Text", "Image"],
      output_path: "/tmp/jin",
      schedule: {},
      sync_targets: [],
      stages: [],
      artifacts: [],
      review_bundles: [],
      events: [],
      created_at: "2026-05-07T00:00:00Z",
      updated_at: "2026-05-07T00:00:00Z",
    };
    const fetchMock = vi.fn().mockImplementation(() =>
      Promise.resolve(
        new Response(JSON.stringify(pipeline), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    const client = createJinApiClient("/api");
    const created = await client.createFactory({
      project: "jin",
      title: "Agent content",
      brief: "Create article drafts",
      mode: "Finite",
      review_policy: "PerStage",
      content_types: ["Text", "Image"],
      output_path: null,
      sync_targets: [],
    });
    await client.resumeFactory(created.id);

    expect(created.id).toBe("factory-1");
    expect(fetchMock).toHaveBeenNthCalledWith(1, "/api/factories", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        project: "jin",
        title: "Agent content",
        brief: "Create article drafts",
        mode: "Finite",
        review_policy: "PerStage",
        content_types: ["Text", "Image"],
        output_path: null,
        sync_targets: [],
      }),
    });
    expect(fetchMock).toHaveBeenNthCalledWith(2, "/api/factories/factory-1/resume", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: undefined,
    });
  });

  it("requests bounded chat message pages", async () => {
    const fetchMock = vi.fn().mockImplementation(() =>
      Promise.resolve(new Response(JSON.stringify({ messages: [], has_more: false }), {
        status: 200,
        headers: { "content-type": "application/json" },
      })),
    );
    vi.stubGlobal("fetch", fetchMock);

    const client = createJinApiClient("/api");
    await client.listMessages("chat-1", { limit: 100, before: "message-1" });
    await client.getChatProgress("chat-1", { limit: 100 });

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "/api/chats/chat-1/messages?limit=100&before=message-1",
      undefined,
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/chats/chat-1/progress?limit=100",
      undefined,
    );
  });

  it("explains when the backend proxy returns an empty server error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response("", { status: 500 })),
    );

    const client = createJinApiClient("/api");

    await expect(client.listTools()).rejects.toThrow(
      "Jin backend is unavailable at http://127.0.0.1:8787",
    );
  });
});
