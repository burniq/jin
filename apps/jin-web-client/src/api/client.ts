import type {
  ChatMessage,
  ChatMessagesPage,
  ChatMessagesQuery,
  ChatProgressResponse,
  ChatSession,
  CreateChatPayload,
  JinSettings,
  ProjectRecord,
  SendChatMessagePayload,
  ToolDescriptor,
  UpdateChatSettingsPayload,
} from "./types";

export interface JinApiClient {
  getSettings(): Promise<JinSettings>;
  updateSettings(payload: JinSettings): Promise<JinSettings>;
  listTools(): Promise<ToolDescriptor[]>;
  listProjects(): Promise<ProjectRecord[]>;
  listChats(): Promise<ChatSession[]>;
  createChat(payload: CreateChatPayload): Promise<ChatSession>;
  getChat(chatId: string): Promise<ChatSession>;
  listMessages(chatId: string, query?: ChatMessagesQuery): Promise<ChatMessagesPage>;
  sendMessage(chatId: string, payload: SendChatMessagePayload): Promise<ChatMessage[]>;
  getChatProgress(chatId: string, query?: ChatMessagesQuery): Promise<ChatProgressResponse>;
  updateChatSettings(
    chatId: string,
    payload: UpdateChatSettingsPayload,
  ): Promise<ChatMessage[]>;
  stopChat(chatId: string): Promise<ChatSession>;
}

export function createJinApiClient(baseUrl = "/api"): JinApiClient {
  const request = async <T>(path: string, init?: RequestInit): Promise<T> => {
    const response = await fetch(`${baseUrl}${path}`, init);
    const text = await response.text();
    const body = parseResponseBody(text);
    if (!response.ok) {
      const backendError = responseError(body);
      throw new Error(
        backendError ??
          backendUnavailableMessage(baseUrl, response.status) ??
          `Request failed with ${response.status}`,
      );
    }
    return body as T;
  };

  const json = (method: "POST" | "PUT", payload?: unknown): RequestInit => ({
    method,
    headers: { "content-type": "application/json" },
    body: payload === undefined ? undefined : JSON.stringify(payload),
  });

  return {
    getSettings: () => request<JinSettings>("/settings"),
    updateSettings: (payload) => request<JinSettings>("/settings", json("PUT", payload)),
    listTools: () => request<ToolDescriptor[]>("/tools"),
    listProjects: () => request<ProjectRecord[]>("/projects"),
    listChats: () => request<ChatSession[]>("/chats"),
    createChat: (payload) => request<ChatSession>("/chats", json("POST", payload)),
    getChat: (chatId) => request<ChatSession>(`/chats/${chatId}`),
    listMessages: (chatId, query) =>
      request<ChatMessagesPage>(`/chats/${chatId}/messages${queryString(query)}`),
    sendMessage: (chatId, payload) =>
      request<ChatMessage[]>(`/chats/${chatId}/messages`, json("POST", payload)),
    getChatProgress: (chatId, query) =>
      request<ChatProgressResponse>(`/chats/${chatId}/progress${queryString(query)}`),
    updateChatSettings: (chatId, payload) =>
      request<ChatMessage[]>(`/chats/${chatId}/settings`, json("POST", payload)),
    stopChat: (chatId) => request<ChatSession>(`/chats/${chatId}/stop`, json("POST")),
  };
}

function queryString(query?: ChatMessagesQuery) {
  if (!query) {
    return "";
  }
  const params = new URLSearchParams();
  if (query.limit !== undefined) {
    params.set("limit", String(query.limit));
  }
  if (query.before) {
    params.set("before", query.before);
  }
  const encoded = params.toString();
  return encoded ? `?${encoded}` : "";
}

function parseResponseBody(text: string): { error?: string } | unknown {
  if (!text) {
    return null;
  }
  try {
    return JSON.parse(text);
  } catch {
    return { error: text };
  }
}

function responseError(body: unknown) {
  if (body && typeof body === "object" && "error" in body) {
    const error = (body as { error?: unknown }).error;
    return typeof error === "string" ? error : null;
  }
  return null;
}

function backendUnavailableMessage(baseUrl: string, status: number) {
  if (baseUrl === "/api" && status >= 500) {
    return "Jin backend is unavailable at http://127.0.0.1:8787. Start jin-server and refresh.";
  }
  return null;
}
