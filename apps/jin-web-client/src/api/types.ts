export type ToolSettingKind = "Select" | "Text";

export interface ToolSettingDescriptor {
  id: string;
  label: string;
  kind: ToolSettingKind;
  options: string[];
  default: string | null;
}

export interface ToolDescriptor {
  id: string;
  name: string;
  supports_persistent_session: boolean;
  supports_context_meter: boolean;
  settings: ToolSettingDescriptor[];
}

export interface ProjectRecord {
  name: string;
  root: string;
}

export interface JinSettings {
  public_host: string | null;
}

export type ChatStatus =
  | "Idle"
  | "Running"
  | "WaitingApproval"
  | "WaitingUser"
  | "Stopped"
  | "Error";

export type ChatRole = "User" | "Assistant" | "Tool" | "System" | "Error";

export interface ContextSummary {
  supported: boolean;
  used: number | null;
  limit: number | null;
  label: string;
}

export interface ChatSession {
  id: string;
  title: string;
  project: string;
  tool: string;
  status: ChatStatus;
  settings: Record<string, string>;
  context: ContextSummary;
  created_at: string;
  updated_at: string;
}

export interface ChatMessage {
  id: string;
  chat_id: string;
  role: ChatRole;
  content: string;
  created_at: string;
}

export interface ChatProgressResponse {
  chat: ChatSession;
  messages: ChatMessage[];
}

export interface ChatMessagesPage {
  messages: ChatMessage[];
  has_more: boolean;
}

export interface ChatMessagesQuery {
  limit?: number;
  before?: string;
}

export interface CreateChatPayload {
  project: string;
  tool: string;
  title: string | null;
  settings: Record<string, string>;
}

export interface SendChatMessagePayload {
  content: string;
}

export interface UpdateChatSettingsPayload {
  settings: Record<string, string>;
}
