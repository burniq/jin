import { afterEach, describe, expect, it, vi } from "vitest";
import { createJinApiClient } from "./client";

const sampleChat = {
  id: "chat-1",
  title: "Jin",
  project: "jin",
  tool: "codex",
  status: "Idle",
  settings: { model: "gpt-5.5" },
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
      new Response(JSON.stringify({ public_host: "jin.example.com" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    const client = createJinApiClient("/api");
    const settings = await client.updateSettings({ public_host: "jin.example.com" });

    expect(settings.public_host).toBe("jin.example.com");
    expect(fetchMock).toHaveBeenCalledWith("/api/settings", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ public_host: "jin.example.com" }),
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
