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
  telegram: TelegramSettings;
  default_sync_targets: SyncTarget[];
}

export interface TelegramSettings {
  bot_token: string | null;
  bot_token_configured: boolean;
  default_group_chat_id: string | null;
}

export type SyncTargetKind = "TelegramChat" | "TelegramForumTopic";

export interface SyncTarget {
  id: string;
  label: string;
  kind: SyncTargetKind;
  chat_id: string | null;
  message_thread_id: number | null;
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
  sync_targets: SyncTarget[];
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
  sync_targets?: SyncTarget[] | null;
}

export interface SendChatMessagePayload {
  content: string;
}

export interface UpdateChatSettingsPayload {
  settings: Record<string, string>;
}

export type FactoryPipelineMode = "Finite" | "Continuous";
export type FactoryReviewPolicy = "FinalOnly" | "PerStage";
export type FactoryPipelineStatus =
  | "Draft"
  | "Scheduled"
  | "Running"
  | "WaitingApproval"
  | "WaitingCapacity"
  | "Paused"
  | "Completed"
  | "Failed"
  | "Stopped";
export type FactoryArtifactKind = "Text" | "Script" | "Image" | "ThreeD" | "Music";
export type FactoryStageType = "Brief" | "Research" | "Plan" | "Generate" | "Refine" | "Review";
export type FactoryStageStatus =
  | "Pending"
  | "Running"
  | "WaitingApproval"
  | "Approved"
  | "NeedsChanges"
  | "Skipped"
  | "Failed";
export type FactoryEventKind = "System" | "Worker" | "Approval" | "Error";

export interface ProjectContentProfile {
  project: string;
  audience: string | null;
  language: string | null;
  tone: string | null;
  persona: string | null;
  content_pillars: string[];
  references: string[];
  constraints: string[];
  publish_channels: string[];
  updated_at: string;
}

export interface ProjectContentProfilePayload {
  audience: string | null;
  language: string | null;
  tone: string | null;
  persona: string | null;
  content_pillars: string[];
  references: string[];
  constraints: string[];
  publish_channels: string[];
}

export interface FactorySchedule {
  run_window?: string | null;
  pause_between_iterations_minutes?: number | null;
  max_iterations_per_window?: number | null;
  max_artifacts_per_run?: number | null;
}

export interface FactoryStage {
  stage_type: FactoryStageType;
  status: FactoryStageStatus;
  revision: number;
  notes: string | null;
}

export interface FactoryEvent {
  id: string;
  pipeline_id: string;
  kind: FactoryEventKind;
  content: string;
  created_at: string;
}

export interface FactoryPipeline {
  id: string;
  project: string;
  title: string;
  brief: string;
  mode: FactoryPipelineMode;
  review_policy: FactoryReviewPolicy;
  status: FactoryPipelineStatus;
  content_types: FactoryArtifactKind[];
  output_path: string | null;
  schedule: FactorySchedule;
  sync_targets: SyncTarget[];
  stages: FactoryStage[];
  artifacts: unknown[];
  review_bundles: unknown[];
  events: FactoryEvent[];
  created_at: string;
  updated_at: string;
}

export interface CreateFactoryPayload {
  project: string;
  title: string | null;
  brief: string;
  mode: FactoryPipelineMode;
  review_policy: FactoryReviewPolicy;
  content_types: FactoryArtifactKind[];
  output_path: string | null;
  sync_targets: SyncTarget[] | null;
}
