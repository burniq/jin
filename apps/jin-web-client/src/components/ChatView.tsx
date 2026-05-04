import { CircleAlert } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ChatMessage, ChatSession, ToolDescriptor } from "../api/types";

interface ChatViewApi {
  sendMessage(chatId: string, payload: { content: string }): Promise<unknown>;
  updateChatSettings(chatId: string, payload: { settings: Record<string, string> }): Promise<unknown>;
  stopChat(chatId: string): Promise<ChatSession>;
}

interface ChatViewProps {
  chat: ChatSession;
  messages: ChatMessage[];
  tools: ToolDescriptor[];
  api: ChatViewApi;
  publicHost?: string | null;
  hasOlderMessages?: boolean;
  onLoadOlder?: () => void | Promise<void>;
  onChanged?: () => void;
}

export function ChatView({
  chat,
  messages,
  tools,
  api,
  publicHost,
  hasOlderMessages = false,
  onLoadOlder,
  onChanged,
}: ChatViewProps) {
  const [draft, setDraft] = useState("");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settings, setSettings] = useState<Record<string, string>>(chat.settings);
  const [error, setError] = useState<string | null>(null);
  const [loadingOlder, setLoadingOlder] = useState(false);
  const timelineRef = useRef<HTMLDivElement | null>(null);
  const timelineBottomRef = useRef<HTMLDivElement | null>(null);
  const shouldFollowBottom = useRef(true);
  const previousTimeline = useRef<{
    chatId: string;
    firstMessageId: string | null;
    lastMessageKey: string | null;
  } | null>(null);
  const tool = useMemo(() => tools.find((item) => item.id === chat.tool), [chat.tool, tools]);
  const visibleMessages = useMemo(() => coalesceToolMessages(messages), [messages]);

  useEffect(() => {
    const firstMessageId = visibleMessages[0]?.id ?? null;
    const lastMessage = visibleMessages.at(-1);
    const lastMessageKey = lastMessage ? `${lastMessage.id}:${lastMessage.content.length}` : null;
    const previous = previousTimeline.current;
    const initialChatOpen = previous?.chatId !== chat.id;
    if (initialChatOpen) {
      shouldFollowBottom.current = true;
    }
    const prependedOlderMessages =
      previous?.chatId === chat.id &&
      previous.firstMessageId !== null &&
      firstMessageId !== null &&
      previous.firstMessageId !== firstMessageId &&
      previous.lastMessageKey === lastMessageKey;
    const timelineChanged = previous?.lastMessageKey !== lastMessageKey;
    if ((initialChatOpen || (shouldFollowBottom.current && timelineChanged)) && !prependedOlderMessages) {
      timelineBottomRef.current?.scrollIntoView?.({ block: "end" });
    }
    previousTimeline.current = {
      chatId: chat.id,
      firstMessageId,
      lastMessageKey,
    };
  }, [chat.id, visibleMessages]);

  const updateFollowBottom = () => {
    const timeline = timelineRef.current;
    if (!timeline) {
      return;
    }
    const distanceToBottom = timeline.scrollHeight - timeline.scrollTop - timeline.clientHeight;
    shouldFollowBottom.current = distanceToBottom <= 80;
  };

  const sendDraft = async () => {
    const content = draft.trim();
    if (!content) {
      return;
    }
    try {
      await api.sendMessage(chat.id, { content });
      setDraft("");
      onChanged?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to send message");
    }
  };

  const send = (event: React.FormEvent) => {
    event.preventDefault();
    void sendDraft();
  };

  const sendFromKeyboard = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.metaKey && event.key === "Enter") {
      event.preventDefault();
      void sendDraft();
    }
  };

  const updateSettings = async (event: React.FormEvent) => {
    event.preventDefault();
    try {
      await api.updateChatSettings(chat.id, { settings });
      onChanged?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to update settings");
    }
  };

  const stop = async () => {
    try {
      await api.stopChat(chat.id);
      onChanged?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to stop chat");
    }
  };

  const loadOlder = async () => {
    if (!onLoadOlder) {
      return;
    }
    setLoadingOlder(true);
    try {
      await onLoadOlder();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load older messages");
    } finally {
      setLoadingOlder(false);
    }
  };

  return (
    <section className="chat-view">
      <header className="chat-header">
        <div>
          <h1>{chat.title}</h1>
          <div className="chips">
            <span>{chat.project}</span>
            <span>{tool?.name ?? chat.tool}</span>
            {Object.entries(chat.settings).map(([key, value]) => (
              <span key={key}>
                {key}: {value}
              </span>
            ))}
            {chat.context.supported ? <span>{chat.context.label}</span> : null}
          </div>
        </div>
        <div className="header-actions">
          <button type="button" className="secondary" onClick={() => setSettingsOpen((open) => !open)}>
            Settings
          </button>
          {chat.status === "Running" ? (
            <button type="button" className="secondary" onClick={stop}>
              Stop
            </button>
          ) : null}
        </div>
      </header>

      {settingsOpen ? (
        <form className="settings-panel" onSubmit={updateSettings}>
          {tool?.settings.map((setting) => (
            <label key={setting.id}>
              <span>{setting.label}</span>
              {setting.kind === "Select" ? (
                <select
                  value={settings[setting.id] ?? setting.default ?? ""}
                  onChange={(event) =>
                    setSettings((current) => ({
                      ...current,
                      [setting.id]: event.target.value,
                    }))
                  }
                >
                  {setting.options.map((option) => (
                    <option key={option} value={option}>
                      {option}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  value={settings[setting.id] ?? setting.default ?? ""}
                  onChange={(event) =>
                    setSettings((current) => ({
                      ...current,
                      [setting.id]: event.target.value,
                    }))
                  }
                />
              )}
            </label>
          ))}
          <button type="submit">Update settings</button>
        </form>
      ) : null}

      {chat.status === "Running" ? (
        <div className="agent-progress" role="status">
          <span className="pulse" />
          <span>Agent is working</span>
        </div>
      ) : null}

      <div className="timeline" ref={timelineRef} onScroll={updateFollowBottom}>
        {hasOlderMessages ? (
          <button
            type="button"
            className="load-older-button"
            onClick={() => void loadOlder()}
            disabled={loadingOlder}
          >
            {loadingOlder ? "Loading..." : "Load older messages"}
          </button>
        ) : null}
        {visibleMessages.length === 0 ? <p className="muted">No messages yet.</p> : null}
        {visibleMessages.map((message) => (
          <article
            key={message.id}
            className={`message ${message.role.toLowerCase()}`}
            aria-label={message.role === "Error" ? "Error message" : undefined}
          >
            {message.role === "Tool" ? (
              <details className="tool-output">
                <summary>Tool output</summary>
                <pre>{renderLinkedText(message.content, publicHost)}</pre>
              </details>
            ) : (
              <>
                <strong className="message-role">
                  {message.role === "Error" ? <CircleAlert size={16} aria-hidden="true" /> : null}
                  {message.role}
                </strong>
                <pre>{renderLinkedText(message.content, publicHost)}</pre>
              </>
            )}
          </article>
        ))}
        <div ref={timelineBottomRef} />
      </div>

      <form className="chat-composer" onSubmit={send}>
        <label>
          <span>Message</span>
          <textarea
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={sendFromKeyboard}
          />
        </label>
        <button type="submit">Send</button>
      </form>
      {error ? <p className="form-error">{error}</p> : null}
    </section>
  );
}

function coalesceToolMessages(messages: ChatMessage[]): ChatMessage[] {
  return messages.reduce<ChatMessage[]>((coalesced, message) => {
    const previous = coalesced.at(-1);
    if (previous?.role === "Tool" && message.role === "Tool") {
      coalesced[coalesced.length - 1] = {
        ...previous,
        content: `${previous.content}${message.content}`,
        created_at: message.created_at,
      };
      return coalesced;
    }
    coalesced.push(message);
    return coalesced;
  }, []);
}

function renderLinkedText(content: string, publicHost?: string | null) {
  const nodes: React.ReactNode[] = [];
  const urlPattern = /https?:\/\/[^\s<>"']+/g;
  let cursor = 0;
  let match: RegExpExecArray | null;

  while ((match = urlPattern.exec(content)) !== null) {
    const [rawMatch] = match;
    const { url, suffix } = splitUrlSuffix(rawMatch);
    if (!url) {
      continue;
    }

    if (match.index > cursor) {
      nodes.push(content.slice(cursor, match.index));
    }

    const href = rewriteLoopbackUrl(url, publicHost);
    nodes.push(
      <a key={`${match.index}-${href}`} href={href} target="_blank" rel="noreferrer">
        {href}
      </a>,
    );
    if (suffix) {
      nodes.push(suffix);
    }
    cursor = match.index + rawMatch.length;
  }

  if (cursor < content.length) {
    nodes.push(content.slice(cursor));
  }

  return nodes.length > 0 ? nodes : content;
}

function splitUrlSuffix(rawUrl: string) {
  const suffixMatch = /[.,;:!?)]*$/.exec(rawUrl);
  const suffix = suffixMatch?.[0] ?? "";
  return {
    url: rawUrl.slice(0, rawUrl.length - suffix.length),
    suffix,
  };
}

function rewriteLoopbackUrl(rawUrl: string, publicHost?: string | null) {
  try {
    const url = new URL(rawUrl);
    if (isLoopbackHost(url.hostname)) {
      url.hostname = normalizedPublicHost(publicHost);
    }
    return url.toString();
  } catch {
    return rawUrl;
  }
}

function isLoopbackHost(hostname: string) {
  return ["localhost", "127.0.0.1", "::1", "[::1]"].includes(hostname.toLowerCase());
}

function normalizedPublicHost(publicHost?: string | null) {
  const fallback = window.location.hostname || "localhost";
  const rawHost = publicHost?.trim() || fallback;
  try {
    return new URL(rawHost.includes("://") ? rawHost : `http://${rawHost}`).hostname;
  } catch {
    return fallback;
  }
}
