import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ChatMessage, ChatSession, ToolDescriptor } from "../api/types";
import { ChatView } from "./ChatView";

const codex: ToolDescriptor = {
  id: "codex",
  name: "Codex",
  supports_persistent_session: true,
  supports_context_meter: true,
  settings: [
    { id: "model", label: "Model", kind: "Select", options: ["gpt-5.5"], default: "gpt-5.5" },
    { id: "reasoning", label: "Reasoning", kind: "Select", options: ["medium", "high"], default: "medium" },
  ],
};
const chat = (status: ChatSession["status"]): ChatSession => ({
  id: "chat-1",
  title: "Fix UI",
  project: "jin",
  tool: "codex",
  status,
  settings: { model: "gpt-5.5", reasoning: "medium" },
  context: { supported: true, used: 100, limit: 1000, label: "100 / 1000" },
  sync_targets: [],
  created_at: "2026-05-04T00:00:00Z",
  updated_at: "2026-05-04T00:00:00Z",
});
const messages: ChatMessage[] = [
  {
    id: "message-1",
    chat_id: "chat-1",
    role: "User",
    content: "Fix it",
    created_at: "2026-05-04T00:00:01Z",
  },
];

describe("ChatView", () => {
  it("shows running progress only for running chats", () => {
    const { rerender } = render(
      <ChatView chat={chat("Running")} messages={messages} tools={[codex]} api={fakeApi()} />,
    );

    expect(screen.getByText("Agent is working")).toBeInTheDocument();

    rerender(<ChatView chat={chat("Stopped")} messages={messages} tools={[codex]} api={fakeApi()} />);

    expect(screen.queryByText("Agent is working")).not.toBeInTheDocument();
  });

  it("sends messages through the API", async () => {
    const api = fakeApi();
    render(<ChatView chat={chat("Idle")} messages={messages} tools={[codex]} api={api} />);

    await userEvent.type(screen.getByLabelText("Message"), "Run tests");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));

    await waitFor(() =>
      expect(api.sendMessage).toHaveBeenCalledWith("chat-1", { content: "Run tests" }),
    );
  });

  it("sends messages with command enter from the composer", async () => {
    const api = fakeApi();
    render(<ChatView chat={chat("Idle")} messages={messages} tools={[codex]} api={api} />);

    await userEvent.type(screen.getByLabelText("Message"), "Run tests");
    await userEvent.keyboard("{Meta>}{Enter}{/Meta}");

    await waitFor(() =>
      expect(api.sendMessage).toHaveBeenCalledWith("chat-1", { content: "Run tests" }),
    );
  });

  it("updates mutable chat settings", async () => {
    const api = fakeApi();
    render(<ChatView chat={chat("Idle")} messages={messages} tools={[codex]} api={api} />);

    await userEvent.click(screen.getByText("Settings"));
    await userEvent.selectOptions(screen.getByLabelText("Reasoning"), "high");
    await userEvent.click(screen.getByRole("button", { name: "Update settings" }));

    await waitFor(() =>
      expect(api.updateChatSettings).toHaveBeenCalledWith("chat-1", {
        settings: { model: "gpt-5.5", reasoning: "high" },
      }),
    );
  });

  it("renders consecutive tool chunks as one current answer", () => {
    render(
      <ChatView
        chat={chat("Running")}
        messages={[
          ...messages,
          {
            id: "tool-1",
            chat_id: "chat-1",
            role: "Tool",
            content: "Ак",
            created_at: "2026-05-04T00:00:02Z",
          },
          {
            id: "tool-2",
            chat_id: "chat-1",
            role: "Tool",
            content: "туальный ответ",
            created_at: "2026-05-04T00:00:03Z",
          },
        ]}
        tools={[codex]}
        api={fakeApi()}
      />,
    );

    expect(screen.getByText("Актуальный ответ")).toBeInTheDocument();
    expect(screen.getAllByText("Tool output")).toHaveLength(1);
  });

  it("offers loading older messages above the timeline", async () => {
    const onLoadOlder = vi.fn().mockResolvedValue(undefined);
    render(
      <ChatView
        chat={chat("Idle")}
        messages={messages}
        tools={[codex]}
        api={fakeApi()}
        hasOlderMessages
        onLoadOlder={onLoadOlder}
      />,
    );

    await userEvent.click(screen.getByRole("button", { name: "Load older messages" }));

    expect(onLoadOlder).toHaveBeenCalledTimes(1);
  });

  it("scrolls to the latest message when a chat opens", () => {
    const scrollIntoView = vi.fn();
    const original = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = scrollIntoView;

    render(
      <ChatView
        chat={chat("Idle")}
        messages={[
          ...messages,
          {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "Assistant",
            content: "Latest answer",
            created_at: "2026-05-04T00:00:02Z",
          },
        ]}
        tools={[codex]}
        api={fakeApi()}
      />,
    );

    expect(scrollIntoView).toHaveBeenCalled();
    Element.prototype.scrollIntoView = original;
  });

  it("does not pull the user back to the bottom after they scroll away", () => {
    const scrollIntoView = vi.fn();
    const original = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = scrollIntoView;
    const { container, rerender } = render(
      <ChatView
        chat={chat("Running")}
        messages={messages}
        tools={[codex]}
        api={fakeApi()}
      />,
    );
    expect(scrollIntoView).toHaveBeenCalledTimes(1);

    const timeline = container.querySelector(".timeline") as HTMLDivElement;
    Object.defineProperty(timeline, "scrollHeight", { configurable: true, value: 1000 });
    Object.defineProperty(timeline, "clientHeight", { configurable: true, value: 300 });
    Object.defineProperty(timeline, "scrollTop", { configurable: true, value: 100 });
    fireEvent.scroll(timeline);

    rerender(
      <ChatView
        chat={chat("Running")}
        messages={[...messages]}
        tools={[codex]}
        api={fakeApi()}
      />,
    );

    expect(scrollIntoView).toHaveBeenCalledTimes(1);
    Element.prototype.scrollIntoView = original;
  });

  it("renders tool output collapsed by default", async () => {
    render(
      <ChatView
        chat={chat("Idle")}
        messages={[
          ...messages,
          {
            id: "tool-1",
            chat_id: "chat-1",
            role: "Tool",
            content: "Command completed\noutput",
            created_at: "2026-05-04T00:00:02Z",
          },
        ]}
        tools={[codex]}
        api={fakeApi()}
      />,
    );

    const details = screen.getByText("Tool output").closest("details");
    expect(details).not.toHaveAttribute("open");

    await userEvent.click(screen.getByText("Tool output"));

    expect(details).toHaveAttribute("open");
  });

  it("rewrites localhost links through the configured public host", () => {
    render(
      <ChatView
        chat={chat("Idle")}
        messages={[
          {
            id: "assistant-1",
            chat_id: "chat-1",
            role: "Assistant",
            content: "Open http://localhost:54418/content/design.html",
            created_at: "2026-05-04T00:00:01Z",
          },
        ]}
        tools={[codex]}
        api={fakeApi()}
        publicHost="jin.example.com"
      />,
    );

    expect(screen.getByRole("link", { name: "http://jin.example.com:54418/content/design.html" }))
      .toHaveAttribute("href", "http://jin.example.com:54418/content/design.html");
  });

  it("renders error messages with a clear alert label", () => {
    render(
      <ChatView
        chat={chat("Error")}
        messages={[
          ...messages,
          {
            id: "error-1",
            chat_id: "chat-1",
            role: "Error",
            content: "failed to write to codex",
            created_at: "2026-05-04T00:00:02Z",
          },
        ]}
        tools={[codex]}
        api={fakeApi()}
      />,
    );

    expect(screen.getByLabelText("Error message")).toBeInTheDocument();
    expect(screen.getByText("failed to write to codex")).toBeInTheDocument();
  });
});

function fakeApi() {
  return {
    sendMessage: vi.fn().mockResolvedValue([]),
    updateChatSettings: vi.fn().mockResolvedValue([]),
    stopChat: vi.fn().mockResolvedValue(chat("Stopped")),
  };
}
