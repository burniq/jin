import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChatProgressResponse } from "../api/types";
import { useChatProgress } from "./useChatProgress";

const progress: ChatProgressResponse = {
  chat: {
    id: "chat-1",
    title: "Progress",
    project: "jin",
    tool: "codex",
    status: "Running",
    settings: {},
    context: {
      supported: true,
      used: null,
      limit: null,
      label: "Context pending",
    },
    created_at: "2026-05-04T00:00:00Z",
    updated_at: "2026-05-04T00:00:00Z",
  },
  messages: [
    {
      id: "message-1",
      chat_id: "chat-1",
      role: "Tool",
      content: "running tests",
      created_at: "2026-05-04T00:00:01Z",
    },
  ],
};

function Probe() {
  const state = useChatProgress({
    chatId: "chat-1",
    enabled: true,
    pollMs: 20,
    api: {
      getChatProgress: vi.fn().mockResolvedValue(progress),
    },
  });

  return <div>{state.progress?.messages[0]?.content ?? "empty"}</div>;
}

describe("useChatProgress", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("polls chat progress and exposes returned messages", async () => {
    render(<Probe />);

    await waitFor(() => expect(screen.getByText("running tests")).toBeInTheDocument());
  });
});
