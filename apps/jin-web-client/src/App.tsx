import { useCallback, useEffect, useMemo, useState } from "react";
import { createJinApiClient, type JinApiClient } from "./api/client";
import type { ChatMessage, ChatSession } from "./api/types";
import { AppShell } from "./components/AppShell";
import { ChatView } from "./components/ChatView";
import { DocsView } from "./components/DocsView";
import { FactoriesView } from "./components/FactoriesView";
import { NewChat } from "./components/NewChat";
import { SettingsView } from "./components/SettingsView";
import { useChatProgress } from "./hooks/useChatProgress";
import { useJinData } from "./hooks/useJinData";

interface AppProps {
  api?: JinApiClient;
}

const defaultApiClient = createJinApiClient();
const CHAT_MESSAGE_PAGE_SIZE = 100;
const CHAT_MESSAGE_WINDOW_LIMIT = 500;

export function App({ api = defaultApiClient }: AppProps) {
  const [path, setPath] = useState(window.location.pathname);
  const [selectedChat, setSelectedChat] = useState<ChatSession | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [hasOlderMessages, setHasOlderMessages] = useState(false);
  const data = useJinData(api);
  const selectedChatId = useMemo(() => matchChatId(path), [path]);
  const selectedFactoryId = useMemo(() => matchFactoryId(path), [path]);
  const progressQuery = useMemo(() => ({ limit: CHAT_MESSAGE_PAGE_SIZE }), []);
  const initialNewChatProject = useMemo(() => {
    const query = path.includes("?") ? path.slice(path.indexOf("?")) : window.location.search;
    return new URLSearchParams(query).get("project");
  }, [path]);

  const navigate = useCallback((nextPath: string) => {
    window.history.pushState({}, "", nextPath);
    setPath(nextPath);
  }, []);

  useEffect(() => {
    const onPop = () => setPath(window.location.pathname);
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  const loadSelectedChat = useCallback(async () => {
    if (!selectedChatId) {
      setSelectedChat(null);
      setMessages([]);
      setHasOlderMessages(false);
      return;
    }
    const [chat, page] = await Promise.all([
      api.getChat(selectedChatId),
      api.listMessages(selectedChatId, { limit: CHAT_MESSAGE_PAGE_SIZE }),
    ]);
    setSelectedChat(chat);
    setMessages(limitMessageWindow(page.messages, "tail"));
    setHasOlderMessages(page.has_more);
  }, [api, selectedChatId]);

  useEffect(() => {
    void loadSelectedChat();
  }, [loadSelectedChat]);

  const handleProgress = useCallback(
    (progress: { chat: ChatSession; messages: ChatMessage[] }) => {
      setSelectedChat(progress.chat);
      setMessages((current) =>
        limitMessageWindow(mergeMessages(current, progress.messages), "tail"),
      );
    },
    [],
  );

  useChatProgress({
    chatId: selectedChatId,
    enabled: Boolean(selectedChatId),
    api,
    query: progressQuery,
    onProgress: handleProgress,
  });

  const refreshAfterChange = async () => {
    await Promise.all([data.refreshChats(), loadSelectedChat()]);
  };

  const loadOlderMessages = async () => {
    const firstMessage = messages[0];
    if (!selectedChatId || !firstMessage) {
      return;
    }
    const page = await api.listMessages(selectedChatId, {
      limit: CHAT_MESSAGE_PAGE_SIZE,
      before: firstMessage.id,
    });
    setMessages((current) =>
      limitMessageWindow(mergeMessages(page.messages, current), "head"),
    );
    setHasOlderMessages(page.has_more);
  };

  let content: React.ReactNode;
  if (path === "/docs") {
    content = <DocsView />;
  } else if (path === "/settings") {
    content = (
      <SettingsView
        settings={data.settings}
        api={api}
        onChanged={() => void data.refreshSettings()}
      />
    );
  } else if (path === "/factories" || selectedFactoryId) {
    content = (
      <FactoriesView
        projects={data.projects}
        factories={data.factories}
        api={api}
        selectedFactoryId={selectedFactoryId}
        defaultSyncTargets={data.settings.default_sync_targets}
        onNavigate={navigate}
        onChanged={() => void data.refreshFactories()}
      />
    );
  } else if (selectedChatId && selectedChat) {
    content = (
      <ChatView
        chat={selectedChat}
        messages={messages}
        tools={data.tools}
        api={api}
        publicHost={data.settings.public_host}
        hasOlderMessages={hasOlderMessages}
        onLoadOlder={loadOlderMessages}
        onChanged={() => void refreshAfterChange()}
      />
    );
  } else if (selectedChatId) {
    content = <section className="empty-state">Loading chat...</section>;
  } else {
    content = (
      <section className="new-chat-route">
        <h1>Start a new chat</h1>
        <NewChat
          projects={data.projects}
          tools={data.tools}
          api={api}
          defaultSyncTargets={data.settings.default_sync_targets}
          initialProject={initialNewChatProject}
          onCreated={(chatId) => {
            void data.refreshChats();
            navigate(`/chats/${chatId}`);
          }}
        />
      </section>
    );
  }

  return (
    <AppShell chats={data.chats} projects={data.projects} onNavigate={navigate}>
      {data.error ? <p className="app-error">{data.error}</p> : null}
      {data.loading ? <p className="muted">Loading Jin...</p> : content}
    </AppShell>
  );
}

function matchChatId(path: string) {
  const match = /^\/chats\/([^/]+)$/.exec(path);
  return match?.[1] ?? null;
}

function matchFactoryId(path: string) {
  const match = /^\/factories\/([^/]+)$/.exec(path);
  return match?.[1] ?? null;
}

function mergeMessages(existing: ChatMessage[], incoming: ChatMessage[]) {
  const byId = new Map<string, ChatMessage>();
  for (const message of existing) {
    byId.set(message.id, message);
  }
  for (const message of incoming) {
    byId.set(message.id, message);
  }
  return Array.from(byId.values()).sort((left, right) =>
    left.created_at.localeCompare(right.created_at),
  );
}

function limitMessageWindow(messages: ChatMessage[], edge: "head" | "tail") {
  if (messages.length <= CHAT_MESSAGE_WINDOW_LIMIT) {
    return messages;
  }
  return edge === "head"
    ? messages.slice(0, CHAT_MESSAGE_WINDOW_LIMIT)
    : messages.slice(-CHAT_MESSAGE_WINDOW_LIMIT);
}
