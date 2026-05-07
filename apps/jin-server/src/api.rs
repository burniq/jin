use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use jin_core::chat::{
    built_in_tools, ChatMessage, ChatRole, ChatSession, ChatStatus, ToolDescriptor,
};
use jin_core::orchestrator::{
    CreateChatRequest, CreateTaskRequest, JinOrchestrator, OrchestratorError,
    PostChatMessageRequest, UpdateChatSettingsRequest,
};
use jin_core::store::{FileStore, JinSettings, StoreError};
use jin_core::telegram::{parse_update, TelegramCommand, TelegramUpdate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tokio::time::{sleep, Duration};

#[derive(Clone)]
pub struct AppState {
    orchestrator: Arc<Mutex<JinOrchestrator>>,
    runtime: Arc<AsyncMutex<ChatRuntimeManager>>,
    api_token: Option<String>,
}

const DEFAULT_CHAT_MESSAGE_LIMIT: usize = 100;
const MAX_CHAT_MESSAGE_LIMIT: usize = 200;

#[cfg(test)]
pub fn build_app(state_path: impl AsRef<FsPath>) -> Result<Router, StoreError> {
    build_app_with_runtime(state_path, None, ChatRuntimeManager::fake())
}

pub fn build_app_with_token(
    state_path: impl AsRef<FsPath>,
    api_token: Option<String>,
) -> Result<Router, StoreError> {
    build_app_with_runtime_and_tools(
        state_path,
        api_token,
        ChatRuntimeManager::real(),
        discover_tool_descriptors(),
    )
}

#[cfg(test)]
fn build_app_with_runtime(
    state_path: impl AsRef<FsPath>,
    api_token: Option<String>,
    runtime: ChatRuntimeManager,
) -> Result<Router, StoreError> {
    build_app_with_runtime_and_tools(state_path, api_token, runtime, built_in_tools())
}

fn build_app_with_runtime_and_tools(
    state_path: impl AsRef<FsPath>,
    api_token: Option<String>,
    runtime: ChatRuntimeManager,
    tools: Vec<ToolDescriptor>,
) -> Result<Router, StoreError> {
    let store = FileStore::open(state_path)?;
    let state = AppState {
        orchestrator: Arc::new(Mutex::new(JinOrchestrator::new_with_tools(store, tools))),
        runtime: Arc::new(AsyncMutex::new(runtime)),
        api_token,
    };

    Ok(Router::new()
        .route("/health", get(health))
        .route("/settings", get(get_settings).put(update_settings))
        .route("/tools", get(list_tools))
        .route("/projects", get(list_projects).post(register_project))
        .route(
            "/projects/{project}/content-profile",
            get(get_project_content_profile).put(update_project_content_profile),
        )
        .route("/factories", get(list_factories).post(create_factory))
        .route("/factories/{factory_id}", get(get_factory))
        .route("/factories/{factory_id}/events", get(list_factory_events))
        .route("/factories/{factory_id}/pause", post(pause_factory))
        .route("/factories/{factory_id}/resume", post(resume_factory))
        .route("/factories/{factory_id}/stop", post(stop_factory))
        .route("/chats", get(list_chats).post(create_chat))
        .route("/chats/{chat_id}", get(get_chat))
        .route("/chats/{chat_id}/settings", post(update_chat_settings))
        .route(
            "/chats/{chat_id}/messages",
            get(list_chat_messages).post(post_chat_message),
        )
        .route("/chats/{chat_id}/progress", get(chat_progress))
        .route("/chats/{chat_id}/stop", post(stop_chat))
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/{task_id}", get(get_task))
        .route("/approvals", get(list_approvals))
        .route("/approvals/{approval_id}/approve", post(approve))
        .route("/approvals/{approval_id}/reject", post(reject))
        .route("/telegram/webhook", post(telegram_webhook))
        .with_state(state))
}

fn discover_tool_descriptors() -> Vec<ToolDescriptor> {
    match discover_codex_model_options() {
        Ok(models) if !models.is_empty() => {
            tools_with_codex_model_options(built_in_tools(), models)
        }
        _ => built_in_tools(),
    }
}

fn discover_codex_model_options() -> Result<Vec<String>, RuntimeError> {
    let output = std::process::Command::new("codex")
        .args(["debug", "models"])
        .output()
        .map_err(|error| {
            RuntimeError::Failed(format!("failed to run codex debug models: {error}"))
        })?;
    if !output.status.success() {
        return Err(RuntimeError::Failed(format!(
            "codex debug models exited with {}",
            output.status
        )));
    }
    let raw = String::from_utf8(output.stdout).map_err(|error| {
        RuntimeError::Failed(format!("codex model catalog is not utf-8: {error}"))
    })?;
    parse_codex_model_options(&raw)
}

fn parse_codex_model_options(raw: &str) -> Result<Vec<String>, RuntimeError> {
    #[derive(Debug, Deserialize)]
    struct Catalog {
        models: Vec<CatalogModel>,
    }

    #[derive(Debug, Deserialize)]
    struct CatalogModel {
        slug: String,
        visibility: String,
    }

    let catalog: Catalog = serde_json::from_str(raw).map_err(|error| {
        RuntimeError::Failed(format!("failed to parse codex model catalog: {error}"))
    })?;
    Ok(catalog
        .models
        .into_iter()
        .filter(|model| model.visibility == "list")
        .map(|model| model.slug)
        .collect())
}

fn tools_with_codex_model_options(
    mut tools: Vec<ToolDescriptor>,
    models: Vec<String>,
) -> Vec<ToolDescriptor> {
    if models.is_empty() {
        return tools;
    }
    if let Some(model_setting) = tools
        .iter_mut()
        .find(|tool| tool.id == "codex")
        .and_then(|tool| {
            tool.settings
                .iter_mut()
                .find(|setting| setting.id == "model")
        })
    {
        model_setting.default = models.first().cloned();
        model_setting.options = models;
    }
    tools
}

pub async fn serve(
    addr: SocketAddr,
    state_path: PathBuf,
    api_token: Option<String>,
) -> Result<(), ServerError> {
    let app = build_app_with_token(state_path, api_token)?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        component: "jin-server".to_string(),
    })
}

async fn list_projects(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<jin_core::orchestrator::ProjectRecord>>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.list_projects()))
}

async fn get_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<JinSettings>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.settings()))
}

async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(settings): Json<JinSettings>,
) -> Result<Json<JinSettings>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.update_settings(settings)?))
}

async fn list_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ToolDescriptor>>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.list_tools()))
}

async fn register_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterProjectRequest>,
) -> Result<Json<jin_core::orchestrator::ProjectRecord>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    let project = orchestrator.register_project(request.name, request.root)?;
    Ok(Json(project))
}

async fn get_project_content_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project): Path<String>,
) -> Result<Json<jin_core::factory::ProjectContentProfile>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    orchestrator
        .get_project_content_profile(&project)
        .map(Json)
        .ok_or_else(|| ApiError::NotFound("content profile not found".to_string()))
}

async fn update_project_content_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project): Path<String>,
    Json(payload): Json<ContentProfilePayload>,
) -> Result<Json<jin_core::factory::ProjectContentProfile>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    let profile = orchestrator.update_project_content_profile(payload.into_update(project))?;
    Ok(Json(profile))
}

async fn list_factories(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<jin_core::factory::FactoryPipeline>>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.list_factory_pipelines()))
}

async fn create_factory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<jin_core::factory::CreateFactoryPipelineRequest>,
) -> Result<Json<jin_core::factory::FactoryPipeline>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.create_factory_pipeline(request)?))
}

async fn get_factory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(factory_id): Path<String>,
) -> Result<Json<jin_core::factory::FactoryPipeline>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    orchestrator
        .get_factory_pipeline(&factory_id)
        .map(Json)
        .ok_or(OrchestratorError::UnknownFactory.into())
}

async fn list_factory_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(factory_id): Path<String>,
) -> Result<Json<Vec<jin_core::factory::FactoryEvent>>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    if orchestrator.get_factory_pipeline(&factory_id).is_none() {
        return Err(OrchestratorError::UnknownFactory.into());
    }
    Ok(Json(orchestrator.list_factory_events(&factory_id)))
}

async fn pause_factory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(factory_id): Path<String>,
) -> Result<Json<jin_core::factory::FactoryPipeline>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.pause_factory_pipeline(&factory_id)?))
}

async fn resume_factory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(factory_id): Path<String>,
) -> Result<Json<jin_core::factory::FactoryPipeline>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.resume_factory_pipeline(&factory_id)?))
}

async fn stop_factory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(factory_id): Path<String>,
) -> Result<Json<jin_core::factory::FactoryPipeline>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.stop_factory_pipeline(&factory_id)?))
}

async fn list_chats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ChatSession>>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.list_chats()))
}

async fn create_chat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateChatRequest>,
) -> Result<Json<ChatSession>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    let chat = orchestrator.create_chat(request)?;
    Ok(Json(chat))
}

async fn get_chat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(chat_id): Path<String>,
) -> Result<Json<ChatSession>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    let chat = orchestrator
        .get_chat(&chat_id)
        .ok_or(OrchestratorError::UnknownChat)?;
    Ok(Json(chat))
}

async fn list_chat_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(chat_id): Path<String>,
    Query(query): Query<ChatMessagesQuery>,
) -> Result<Json<ChatMessagesPage>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    if orchestrator.get_chat(&chat_id).is_none() {
        return Err(OrchestratorError::UnknownChat.into());
    }
    let (messages, has_more) = orchestrator.list_chat_message_page(
        &chat_id,
        query.before.as_deref(),
        normalize_message_limit(query.limit),
    );
    Ok(Json(ChatMessagesPage { messages, has_more }))
}

async fn chat_progress(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(chat_id): Path<String>,
    Query(query): Query<ChatMessagesQuery>,
) -> Result<Json<ChatProgressResponse>, ApiError> {
    authorize(&state, &headers)?;
    let current_status = {
        let orchestrator = lock_orchestrator(&state)?;
        orchestrator
            .get_chat(&chat_id)
            .ok_or(OrchestratorError::UnknownChat)?
            .status
    };

    let progress = {
        let mut runtime = state.runtime.lock().await;
        runtime.drain_progress(&chat_id).await
    };

    let mut orchestrator = lock_orchestrator(&state)?;
    append_runtime_messages(&mut orchestrator, &chat_id, progress.outputs)?;
    let next_status = if progress.active {
        ChatStatus::Running
    } else if matches!(
        current_status,
        ChatStatus::Stopped
            | ChatStatus::Error
            | ChatStatus::WaitingApproval
            | ChatStatus::WaitingUser
    ) {
        current_status
    } else {
        ChatStatus::Idle
    };
    let chat = orchestrator.set_chat_status(&chat_id, next_status)?;
    let (messages, _) = orchestrator.list_chat_message_page(
        &chat_id,
        query.before.as_deref(),
        normalize_message_limit(query.limit),
    );
    Ok(Json(ChatProgressResponse { chat, messages }))
}

async fn update_chat_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(chat_id): Path<String>,
    Json(request): Json<UpdateChatSettingsPayload>,
) -> Result<Json<Vec<ChatMessage>>, ApiError> {
    authorize(&state, &headers)?;
    let (previous, updated) = {
        let mut orchestrator = lock_orchestrator(&state)?;
        let previous = orchestrator
            .get_chat(&chat_id)
            .ok_or(OrchestratorError::UnknownChat)?;
        let updated = orchestrator.update_chat_settings(UpdateChatSettingsRequest {
            chat_id: chat_id.clone(),
            settings: request.settings,
        })?;
        orchestrator.append_chat_message(PostChatMessageRequest {
            chat_id: chat_id.clone(),
            role: ChatRole::System,
            content: settings_update_summary(&previous, &updated),
        })?;
        (previous, updated)
    };

    let runtime_outputs = {
        let mut runtime = state.runtime.lock().await;
        runtime.apply_settings(&previous, &updated).await
    };

    if !runtime_outputs.is_empty() {
        let mut orchestrator = lock_orchestrator(&state)?;
        append_runtime_messages(&mut orchestrator, &chat_id, runtime_outputs)?;
        return Ok(Json(list_recent_chat_messages(&orchestrator, &chat_id)));
    }

    let orchestrator = lock_orchestrator(&state)?;
    Ok(Json(list_recent_chat_messages(&orchestrator, &chat_id)))
}

async fn post_chat_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(chat_id): Path<String>,
    Json(request): Json<SendChatMessageRequest>,
) -> Result<Json<Vec<ChatMessage>>, ApiError> {
    authorize(&state, &headers)?;
    let (chat, project_root) = {
        let mut orchestrator = lock_orchestrator(&state)?;
        let chat = orchestrator
            .get_chat(&chat_id)
            .ok_or(OrchestratorError::UnknownChat)?;
        let project_root = orchestrator
            .list_projects()
            .into_iter()
            .find(|project| project.name == chat.project)
            .map(|project| project.root)
            .ok_or(OrchestratorError::UnknownProject)?;
        orchestrator.append_chat_message(PostChatMessageRequest {
            chat_id: chat_id.clone(),
            role: ChatRole::User,
            content: request.content.clone(),
        })?;
        let chat = orchestrator.set_chat_status(&chat_id, ChatStatus::Running)?;
        (chat, project_root)
    };

    let runtime_result = {
        let mut runtime = state.runtime.lock().await;
        runtime.send(&chat, project_root, &request.content).await
    };

    {
        let mut orchestrator = lock_orchestrator(&state)?;
        match runtime_result {
            Ok(outputs) => {
                append_runtime_messages(&mut orchestrator, &chat_id, outputs)?;
                let status = if chat.tool == "codex" {
                    ChatStatus::Running
                } else {
                    ChatStatus::Idle
                };
                orchestrator.set_chat_status(&chat_id, status)?;
            }
            Err(error) => {
                orchestrator.append_chat_message(PostChatMessageRequest {
                    chat_id: chat_id.clone(),
                    role: ChatRole::Error,
                    content: error.to_string(),
                })?;
                orchestrator.set_chat_status(&chat_id, ChatStatus::Error)?;
            }
        }
        Ok(Json(list_recent_chat_messages(&orchestrator, &chat_id)))
    }
}

async fn stop_chat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(chat_id): Path<String>,
) -> Result<Json<ChatSession>, ApiError> {
    authorize(&state, &headers)?;
    {
        let mut runtime = state.runtime.lock().await;
        runtime.stop(&chat_id).await;
    }
    let mut orchestrator = lock_orchestrator(&state)?;
    let chat = orchestrator.set_chat_status(&chat_id, ChatStatus::Stopped)?;
    Ok(Json(chat))
}

async fn list_tasks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<jin_core::orchestrator::TaskRecord>>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.list_tasks()))
}

async fn create_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<jin_core::orchestrator::TaskRecord>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    let task = orchestrator.create_task(request)?;
    Ok(Json(task))
}

async fn get_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<String>,
) -> Result<Json<jin_core::orchestrator::TaskRecord>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    let task = orchestrator
        .get_task(&task_id)
        .ok_or(ApiError::NotFound("task not found".to_string()))?;
    Ok(Json(task))
}

async fn list_approvals(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<jin_core::orchestrator::ApprovalRecord>>, ApiError> {
    authorize(&state, &headers)?;
    let orchestrator = lock_orchestrator(&state)?;
    Ok(Json(orchestrator.list_approvals()))
}

async fn approve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(approval_id): Path<String>,
    Json(request): Json<ApprovalDecisionRequest>,
) -> Result<Json<jin_core::orchestrator::TaskRecord>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    let task = orchestrator.approve(&approval_id, request.actor)?;
    Ok(Json(task))
}

async fn reject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(approval_id): Path<String>,
    Json(request): Json<ApprovalDecisionRequest>,
) -> Result<Json<jin_core::orchestrator::TaskRecord>, ApiError> {
    authorize(&state, &headers)?;
    let mut orchestrator = lock_orchestrator(&state)?;
    let task = orchestrator.reject(&approval_id, request.actor)?;
    Ok(Json(task))
}

async fn telegram_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(update): Json<TelegramUpdate>,
) -> Result<Json<TelegramSendMessageResponse>, ApiError> {
    authorize(&state, &headers)?;
    let command = parse_update(update)
        .map_err(|error| ApiError::BadRequest(format!("invalid telegram command: {error:?}")))?;
    let chat_id = match &command {
        TelegramCommand::CreateTask { chat_id, .. }
        | TelegramCommand::Approve { chat_id, .. }
        | TelegramCommand::Reject { chat_id, .. } => *chat_id,
    };
    let mut orchestrator = lock_orchestrator(&state)?;
    let task = match command {
        TelegramCommand::CreateTask {
            project,
            runner,
            command,
            ..
        } => orchestrator.create_task(CreateTaskRequest {
            project,
            runner,
            command,
        })?,
        TelegramCommand::Approve {
            approval_id,
            user_id,
            ..
        } => orchestrator.approve(&approval_id, actor_from_telegram(user_id))?,
        TelegramCommand::Reject {
            approval_id,
            user_id,
            ..
        } => orchestrator.reject(&approval_id, actor_from_telegram(user_id))?,
    };
    Ok(Json(TelegramSendMessageResponse {
        method: "sendMessage".to_string(),
        chat_id,
        text: telegram_task_summary(&task),
    }))
}

fn authorize(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = &state.api_token else {
        return Ok(());
    };
    let expected = format!("Bearer {expected}");
    let actual = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    if actual == Some(expected.as_str()) {
        Ok(())
    } else {
        Err(ApiError::Unauthorized)
    }
}

fn actor_from_telegram(user_id: Option<i64>) -> String {
    user_id
        .map(|id| format!("telegram:{id}"))
        .unwrap_or_else(|| "telegram:unknown".to_string())
}

fn telegram_task_summary(task: &jin_core::orchestrator::TaskRecord) -> String {
    let approval = task
        .pending_approval_id
        .as_ref()
        .map(|id| format!("\napproval: {id}"))
        .unwrap_or_default();
    format!(
        "task: {}\nstate: {:?}\nrunner: {}{}",
        task.id, task.state, task.runner, approval
    )
}

fn lock_orchestrator(
    state: &AppState,
) -> Result<std::sync::MutexGuard<'_, JinOrchestrator>, ApiError> {
    state
        .orchestrator
        .lock()
        .map_err(|_| ApiError::Internal("orchestrator lock poisoned".to_string()))
}

#[derive(Debug, Clone, Serialize)]
struct HealthResponse {
    status: String,
    component: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RegisterProjectRequest {
    name: String,
    root: PathBuf,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ContentProfilePayload {
    audience: Option<String>,
    language: Option<String>,
    tone: Option<String>,
    persona: Option<String>,
    #[serde(default)]
    content_pillars: Vec<String>,
    #[serde(default)]
    references: Vec<String>,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    publish_channels: Vec<String>,
}

impl ContentProfilePayload {
    fn into_update(self, project: String) -> jin_core::factory::ProjectContentProfileUpdate {
        jin_core::factory::ProjectContentProfileUpdate {
            project,
            audience: self.audience,
            language: self.language,
            tone: self.tone,
            persona: self.persona,
            content_pillars: self.content_pillars,
            references: self.references,
            constraints: self.constraints,
            publish_channels: self.publish_channels,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SendChatMessageRequest {
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct UpdateChatSettingsPayload {
    #[serde(default)]
    settings: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatMessagesQuery {
    limit: Option<usize>,
    before: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessagesPage {
    messages: Vec<ChatMessage>,
    has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatProgressResponse {
    chat: ChatSession,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Deserialize)]
struct ApprovalDecisionRequest {
    actor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TelegramSendMessageResponse {
    method: String,
    chat_id: i64,
    text: String,
}

#[derive(Debug)]
pub enum ServerError {
    Store(StoreError),
    Io(std::io::Error),
}

impl From<StoreError> for ServerError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<std::io::Error> for ServerError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "server io error: {error}"),
        }
    }
}

impl std::error::Error for ServerError {}

#[derive(Debug)]
enum ApiError {
    Unauthorized,
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

impl From<OrchestratorError> for ApiError {
    fn from(error: OrchestratorError) -> Self {
        match error {
            OrchestratorError::UnknownProject
            | OrchestratorError::UnknownTool
            | OrchestratorError::UnknownRunner
            | OrchestratorError::ApprovalAlreadyDecided
            | OrchestratorError::InvalidInput(_) => Self::BadRequest(error.to_string()),
            OrchestratorError::UnknownTask
            | OrchestratorError::UnknownChat
            | OrchestratorError::UnknownApproval
            | OrchestratorError::UnknownFactory => Self::NotFound(error.to_string()),
            OrchestratorError::Io(_)
            | OrchestratorError::Runner(_)
            | OrchestratorError::Store(_) => Self::Internal(error.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "missing or invalid bearer token".to_string(),
            ),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };
        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

struct ChatRuntimeManager {
    mode: ChatRuntimeMode,
    sessions: HashMap<String, RunningChatSession>,
    #[cfg(test)]
    progress_queue: VecDeque<RuntimeMessage>,
}

impl ChatRuntimeManager {
    fn real() -> Self {
        Self {
            mode: ChatRuntimeMode::Real,
            sessions: HashMap::new(),
            #[cfg(test)]
            progress_queue: VecDeque::new(),
        }
    }

    #[cfg(test)]
    fn fake() -> Self {
        Self {
            mode: ChatRuntimeMode::Fake,
            sessions: HashMap::new(),
            progress_queue: VecDeque::new(),
        }
    }

    #[cfg(test)]
    fn fake_with_progress(outputs: Vec<RuntimeMessage>) -> Self {
        Self {
            mode: ChatRuntimeMode::Fake,
            sessions: HashMap::new(),
            progress_queue: outputs.into(),
        }
    }

    async fn send(
        &mut self,
        chat: &ChatSession,
        project_root: PathBuf,
        content: &str,
    ) -> Result<Vec<RuntimeMessage>, RuntimeError> {
        match self.mode {
            ChatRuntimeMode::Real => self.send_real(chat, project_root, content).await,
            #[cfg(test)]
            ChatRuntimeMode::Fake => Ok(vec![RuntimeMessage::append(
                ChatRole::Tool,
                format!("fake {} session received: {}", chat.tool, content),
            )]),
        }
    }

    async fn send_real(
        &mut self,
        chat: &ChatSession,
        project_root: PathBuf,
        content: &str,
    ) -> Result<Vec<RuntimeMessage>, RuntimeError> {
        if chat.tool != "codex" {
            return Ok(vec![RuntimeMessage::append(
                ChatRole::Tool,
                format!(
                    "{} sessions are tracked by jin, but persistent runtime is implemented for codex first",
                    chat.tool
                ),
            )]);
        }

        let newly_started = !self.sessions.contains_key(&chat.id);
        if newly_started {
            let session = RunningChatSession::start_codex(project_root, chat).await?;
            self.sessions.insert(chat.id.clone(), session);
        }
        let session = self
            .sessions
            .get_mut(&chat.id)
            .ok_or_else(|| RuntimeError::Failed("chat session did not start".to_string()))?;
        if newly_started {
            let initial_output = session.drain_after_delay(Duration::from_millis(500)).await;
            if !initial_output.is_empty() {
                return Ok(initial_output);
            }
        }
        session.send(content).await
    }

    async fn stop(&mut self, chat_id: &str) {
        if let Some(mut session) = self.sessions.remove(chat_id) {
            let _ = session.stop().await;
        }
    }

    async fn drain_progress(&mut self, chat_id: &str) -> RuntimeProgress {
        match self.mode {
            ChatRuntimeMode::Real => {
                let Some(session) = self.sessions.get_mut(chat_id) else {
                    return RuntimeProgress::inactive();
                };
                let outputs = session.drain_ready();
                let active = session.is_active();
                RuntimeProgress { outputs, active }
            }
            #[cfg(test)]
            ChatRuntimeMode::Fake => {
                let outputs = self.progress_queue.drain(..).collect::<Vec<_>>();
                RuntimeProgress {
                    active: !outputs.is_empty(),
                    outputs,
                }
            }
        }
    }

    async fn apply_settings(
        &mut self,
        previous: &ChatSession,
        updated: &ChatSession,
    ) -> Vec<RuntimeMessage> {
        if previous.tool != "codex" {
            return Vec::new();
        }
        let Some(session) = self.sessions.get_mut(&previous.id) else {
            return Vec::new();
        };
        if previous.settings == updated.settings {
            return Vec::new();
        }
        match session.apply_settings(updated).await {
            Ok(output) => output,
            Err(error) => vec![RuntimeMessage::append(
                ChatRole::Error,
                format!("failed to apply settings to active codex session: {error}"),
            )],
        }
    }
}

#[derive(Debug)]
enum ChatRuntimeMode {
    Real,
    #[cfg(test)]
    Fake,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeMessage {
    role: ChatRole,
    content: String,
    delivery: RuntimeMessageDelivery,
}

impl RuntimeMessage {
    fn append(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            delivery: RuntimeMessageDelivery::Append,
        }
    }

    fn upsert(role: ChatRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            delivery: RuntimeMessageDelivery::UpsertLast,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeMessageDelivery {
    Append,
    UpsertLast,
}

struct RunningChatSession {
    child: Child,
    stdin: ChildStdin,
    output: mpsc::UnboundedReceiver<Value>,
    next_request_id: u64,
    thread_id: String,
    project_root: PathBuf,
    settings: BTreeMap<String, String>,
    events: CodexAppServerEventState,
    pending: VecDeque<RuntimeMessage>,
}

impl RunningChatSession {
    async fn start_codex(project_root: PathBuf, chat: &ChatSession) -> Result<Self, RuntimeError> {
        let mut command = Command::new("codex");
        command.arg("app-server");
        command.arg("-c");
        command.arg("check_for_update_on_startup=false");
        command.current_dir(&project_root);
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            RuntimeError::Failed(format!("failed to start codex app-server: {error}"))
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            RuntimeError::Failed("failed to open codex app-server stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            RuntimeError::Failed("failed to open codex app-server stdout".to_string())
        })?;
        if let Some(stderr) = child.stderr.take() {
            spawn_stderr_drain(stderr);
        }

        let (tx, rx) = mpsc::unbounded_channel();
        spawn_json_reader(stdout, tx);

        let mut session = Self {
            child,
            stdin,
            output: rx,
            next_request_id: 0,
            thread_id: String::new(),
            project_root,
            settings: chat.settings.clone(),
            events: CodexAppServerEventState::default(),
            pending: VecDeque::new(),
        };

        session
            .request(
                "initialize",
                json!({
                    "clientInfo": {
                        "name": "jin",
                        "title": "Jin",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {
                        "experimentalApi": true
                    }
                }),
            )
            .await?;
        let result = session
            .request(
                "thread/start",
                codex_thread_start_params(&session.project_root, &session.settings),
            )
            .await?;
        session.thread_id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RuntimeError::Failed("codex app-server did not return a thread id".to_string())
            })?
            .to_string();

        Ok(session)
    }

    async fn send(&mut self, content: &str) -> Result<Vec<RuntimeMessage>, RuntimeError> {
        self.request("turn/start", self.turn_start_params(content))
            .await?;
        self.events.active = true;
        Ok(self.drain_after_delay(Duration::from_millis(300)).await)
    }

    async fn apply_settings(
        &mut self,
        updated: &ChatSession,
    ) -> Result<Vec<RuntimeMessage>, RuntimeError> {
        self.settings = updated.settings.clone();
        Ok(Vec::new())
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, RuntimeError> {
        self.next_request_id += 1;
        let id = self.next_request_id;
        let request = json!({
            "id": id,
            "method": method,
            "params": params
        });
        let payload = serde_json::to_vec(&request).map_err(|error| {
            RuntimeError::Failed(format!(
                "failed to encode codex app-server request: {error}"
            ))
        })?;
        self.stdin.write_all(&payload).await.map_err(|error| {
            RuntimeError::Failed(format!("failed to write to codex app-server: {error}"))
        })?;
        self.stdin.write_all(b"\n").await.map_err(|error| {
            RuntimeError::Failed(format!("failed to write to codex app-server: {error}"))
        })?;
        self.stdin.flush().await.map_err(|error| {
            RuntimeError::Failed(format!("failed to flush codex app-server stdin: {error}"))
        })?;

        loop {
            let value = tokio::time::timeout(Duration::from_secs(30), self.output.recv())
                .await
                .map_err(|_| {
                    RuntimeError::Failed(format!("codex app-server timed out waiting for {method}"))
                })?
                .ok_or_else(|| {
                    RuntimeError::Failed("codex app-server closed stdout".to_string())
                })?;

            if response_id(&value) == Some(id) {
                if let Some(error) = value.get("error") {
                    return Err(RuntimeError::Failed(app_server_error_message(error)));
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
            self.pending.extend(self.events.handle_notification(&value));
        }
    }

    fn turn_start_params(&self, content: &str) -> Value {
        let mut params = json!({
            "threadId": self.thread_id,
            "input": [{
                "type": "text",
                "text": content,
                "text_elements": []
            }],
            "cwd": self.project_root,
            "approvalPolicy": codex_approval_policy(&self.settings),
            "model": self.settings.get("model").cloned()
        });
        if let Some(reasoning) = self.settings.get("reasoning") {
            params["effort"] = json!(reasoning);
        }
        params
    }

    async fn drain_after_delay(&mut self, delay: Duration) -> Vec<RuntimeMessage> {
        sleep(delay).await;
        self.drain_ready()
    }

    fn drain_ready(&mut self) -> Vec<RuntimeMessage> {
        while let Ok(value) = self.output.try_recv() {
            self.pending.extend(self.events.handle_notification(&value));
        }
        self.pending.drain(..).collect()
    }

    fn is_active(&self) -> bool {
        self.events.active
    }

    async fn stop(&mut self) -> Result<(), RuntimeError> {
        self.child.kill().await.map_err(|error| {
            RuntimeError::Failed(format!("failed to stop codex app-server: {error}"))
        })
    }
}

#[derive(Debug, Default)]
struct CodexAppServerEventState {
    active: bool,
    agent_messages: HashMap<String, String>,
}

impl CodexAppServerEventState {
    fn handle_notification(&mut self, value: &Value) -> Vec<RuntimeMessage> {
        let Some(method) = value.get("method").and_then(Value::as_str) else {
            return Vec::new();
        };
        let params = value.get("params").unwrap_or(&Value::Null);
        match method {
            "turn/started" => {
                self.active = true;
                Vec::new()
            }
            "turn/completed" => {
                self.active = false;
                Vec::new()
            }
            "thread/status/changed" => {
                self.active =
                    params.pointer("/status/type").and_then(Value::as_str) == Some("active");
                Vec::new()
            }
            "item/agentMessage/delta" => self.handle_agent_delta(params),
            "item/started" => self.handle_item_started(params),
            "item/completed" => self.handle_item_completed(params),
            "error" => vec![RuntimeMessage::append(
                ChatRole::Error,
                params
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("codex app-server reported an error"),
            )],
            "warning" | "guardianWarning" | "configWarning" => params
                .get("message")
                .and_then(Value::as_str)
                .map(|message| vec![RuntimeMessage::append(ChatRole::System, message)])
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn handle_agent_delta(&mut self, params: &Value) -> Vec<RuntimeMessage> {
        let Some(item_id) = params.get("itemId").and_then(Value::as_str) else {
            return Vec::new();
        };
        let Some(delta) = params.get("delta").and_then(Value::as_str) else {
            return Vec::new();
        };
        let text = self.agent_messages.entry(item_id.to_string()).or_default();
        text.push_str(delta);
        vec![RuntimeMessage::upsert(ChatRole::Assistant, text.clone())]
    }

    fn handle_item_started(&mut self, params: &Value) -> Vec<RuntimeMessage> {
        let Some(item) = params.get("item") else {
            return Vec::new();
        };
        if item.get("type").and_then(Value::as_str) != Some("commandExecution") {
            return Vec::new();
        }
        let Some(command) = item.get("command").and_then(Value::as_str) else {
            return Vec::new();
        };
        vec![RuntimeMessage::append(
            ChatRole::Tool,
            format!("Running command: {command}"),
        )]
    }

    fn handle_item_completed(&mut self, params: &Value) -> Vec<RuntimeMessage> {
        let Some(item) = params.get("item") else {
            return Vec::new();
        };
        match item.get("type").and_then(Value::as_str) {
            Some("commandExecution") => command_execution_summary(item)
                .map(|summary| vec![RuntimeMessage::append(ChatRole::Tool, summary)])
                .unwrap_or_default(),
            Some("mcpToolCall") => mcp_tool_call_summary(item)
                .map(|summary| vec![RuntimeMessage::append(ChatRole::Tool, summary)])
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}

struct RuntimeProgress {
    outputs: Vec<RuntimeMessage>,
    active: bool,
}

impl RuntimeProgress {
    fn inactive() -> Self {
        Self {
            outputs: Vec::new(),
            active: false,
        }
    }
}

fn append_runtime_messages(
    orchestrator: &mut JinOrchestrator,
    chat_id: &str,
    messages: Vec<RuntimeMessage>,
) -> Result<(), OrchestratorError> {
    for message in coalesce_runtime_messages(messages) {
        let content = message.content.trim();
        if content.is_empty() || is_successful_acceptance_placeholder(content) {
            continue;
        }
        let request = PostChatMessageRequest {
            chat_id: chat_id.to_string(),
            role: message.role,
            content: content.to_string(),
        };
        match message.delivery {
            RuntimeMessageDelivery::Append => {
                orchestrator.append_chat_message(request)?;
            }
            RuntimeMessageDelivery::UpsertLast => {
                orchestrator.upsert_last_chat_message(request)?;
            }
        }
    }
    Ok(())
}

fn normalize_message_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_CHAT_MESSAGE_LIMIT)
        .clamp(1, MAX_CHAT_MESSAGE_LIMIT)
}

fn list_recent_chat_messages(orchestrator: &JinOrchestrator, chat_id: &str) -> Vec<ChatMessage> {
    orchestrator
        .list_chat_message_page(chat_id, None, DEFAULT_CHAT_MESSAGE_LIMIT)
        .0
}

fn coalesce_runtime_messages(messages: Vec<RuntimeMessage>) -> Vec<RuntimeMessage> {
    messages
        .into_iter()
        .fold(Vec::<RuntimeMessage>::new(), |mut coalesced, message| {
            if let Some(previous) = coalesced.last_mut() {
                if previous.delivery == RuntimeMessageDelivery::Append
                    && message.delivery == RuntimeMessageDelivery::Append
                    && previous.role == message.role
                {
                    previous.content.push_str(&message.content);
                    return coalesced;
                }
            }
            coalesced.push(message);
            coalesced
        })
}

fn response_id(value: &Value) -> Option<u64> {
    value.get("id").and_then(Value::as_u64)
}

fn app_server_error_message(error: &Value) -> String {
    error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("codex app-server request failed")
        .to_string()
}

fn codex_thread_start_params(project_root: &FsPath, settings: &BTreeMap<String, String>) -> Value {
    let mut params = json!({
        "cwd": project_root,
        "approvalPolicy": codex_approval_policy(settings),
        "approvalsReviewer": "user",
        "sandbox": codex_sandbox_mode(settings),
        "serviceName": "jin",
        "ephemeral": false,
        "model": settings.get("model").cloned()
    });
    if let Some(reasoning) = settings.get("reasoning") {
        params["config"] = json!({
            "model_reasoning_effort": reasoning
        });
    }
    params
}

fn codex_approval_policy(settings: &BTreeMap<String, String>) -> Value {
    match settings.get("approval_mode").map(String::as_str) {
        Some("auto-edit") => json!("on-failure"),
        Some("read-only") => json!("on-request"),
        _ => json!("on-request"),
    }
}

fn codex_sandbox_mode(settings: &BTreeMap<String, String>) -> Value {
    match settings.get("approval_mode").map(String::as_str) {
        Some("read-only") => json!("read-only"),
        _ => json!("workspace-write"),
    }
}

fn command_execution_summary(item: &Value) -> Option<String> {
    let command = item.get("command").and_then(Value::as_str)?;
    let status = item
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("completed");
    let mut summary = format!("Command {status}: {command}");
    if let Some(exit_code) = item.get("exitCode").and_then(Value::as_i64) {
        summary.push_str(&format!("\nexit code: {exit_code}"));
    }
    if let Some(output) = item.get("aggregatedOutput").and_then(Value::as_str) {
        if !output.trim().is_empty() {
            summary.push_str("\n\n");
            summary.push_str(output.trim());
        }
    }
    Some(summary)
}

fn mcp_tool_call_summary(item: &Value) -> Option<String> {
    let server = item.get("server").and_then(Value::as_str)?;
    let tool = item.get("tool").and_then(Value::as_str)?;
    let status = item
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("completed");
    Some(format!("MCP {status}: {server}.{tool}"))
}

fn spawn_json_reader<R>(reader: R, tx: mpsc::UnboundedSender<Value>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                let _ = tx.send(value);
            }
        }
    });
}

fn spawn_stderr_drain<R>(reader: R)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(_line)) = lines.next_line().await {}
    });
}

fn settings_update_summary(previous: &ChatSession, updated: &ChatSession) -> String {
    let mut changes = Vec::new();
    for key in ["model", "reasoning", "approval_mode"] {
        let before = previous
            .settings
            .get(key)
            .map(String::as_str)
            .unwrap_or("-");
        let after = updated.settings.get(key).map(String::as_str).unwrap_or("-");
        if before != after {
            changes.push(format!("{key}: {before} -> {after}"));
        }
    }
    if changes.is_empty() {
        "chat settings updated".to_string()
    } else {
        format!("chat settings updated: {}", changes.join(", "))
    }
}

#[cfg(test)]
fn coalesce_runtime_outputs(outputs: Vec<String>) -> Vec<String> {
    let output = normalize_runtime_output(&outputs.concat());
    if output.trim().is_empty() || is_successful_acceptance_placeholder(&output) {
        Vec::new()
    } else {
        vec![output]
    }
}

fn is_successful_acceptance_placeholder(output: &str) -> bool {
    output.trim() == "codex session accepted the message; waiting for output"
}

#[cfg(test)]
fn normalize_runtime_output(input: &str) -> String {
    let mut lines = vec![String::new()];
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' => {
                if let Some(line) = lines.last_mut() {
                    line.clear();
                }
            }
            '\n' => lines.push(String::new()),
            '\t' => {
                if let Some(line) = lines.last_mut() {
                    line.push('\t');
                }
            }
            ch if ch.is_control() => {}
            ch => {
                if let Some(line) = lines.last_mut() {
                    line.push(ch);
                }
            }
        }
    }
    lines.join("\n")
}

#[cfg(test)]
fn codex_settings_command(previous: &ChatSession, updated: &ChatSession) -> Option<String> {
    let model_changed = previous.settings.get("model") != updated.settings.get("model");
    let reasoning_changed = previous.settings.get("reasoning") != updated.settings.get("reasoning");
    if !model_changed && !reasoning_changed {
        return None;
    }
    match (
        updated.settings.get("model"),
        updated.settings.get("reasoning"),
    ) {
        (Some(model), Some(reasoning)) => Some(format!("/model {model} {reasoning}")),
        (Some(model), None) => Some(format!("/model {model}")),
        (None, Some(reasoning)) => Some(format!("/model {reasoning}")),
        (None, None) => None,
    }
}

#[derive(Debug)]
enum RuntimeError {
    Failed(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(message) => write!(f, "{message}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_route_returns_ok() {
        let temp = tempfile::tempdir().expect("tempdir");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn bearer_token_is_required_when_configured() {
        let temp = tempfile::tempdir().expect("tempdir");
        let app = build_app_with_token(temp.path().join("state.json"), Some("secret".to_string()))
            .expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/projects")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/projects")
                    .header("authorization", "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn tools_endpoint_uses_available_codex_models() {
        let temp = tempfile::tempdir().expect("tempdir");
        let tools = tools_with_codex_model_options(
            built_in_tools(),
            vec!["gpt-5.5".to_string(), "gpt-5.4".to_string()],
        );
        let app = build_app_with_runtime_and_tools(
            temp.path().join("state.json"),
            None,
            ChatRuntimeManager::fake(),
            tools,
        )
        .expect("app builds");

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let tools: Vec<ToolDescriptor> = read_json(response).await;
        let model = tools
            .iter()
            .find(|tool| tool.id == "codex")
            .and_then(|tool| tool.settings.iter().find(|setting| setting.id == "model"))
            .expect("codex model setting exists");
        assert_eq!(model.options, vec!["gpt-5.5", "gpt-5.4"]);
        assert_eq!(model.default.as_deref(), Some("gpt-5.5"));
    }

    #[tokio::test]
    async fn settings_endpoint_persists_public_host() {
        let temp = tempfile::tempdir().expect("tempdir");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let settings: jin_core::store::JinSettings = read_json(response).await;
        assert_eq!(settings.public_host, None);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"public_host":"jin.example.com"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let settings: jin_core::store::JinSettings = read_json(response).await;
        assert_eq!(settings.public_host.as_deref(), Some("jin.example.com"));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let settings: jin_core::store::JinSettings = read_json(response).await;
        assert_eq!(settings.public_host.as_deref(), Some("jin.example.com"));
    }

    #[tokio::test]
    async fn settings_endpoint_redacts_telegram_token_and_keeps_sync_defaults() {
        let temp = tempfile::tempdir().expect("tempdir");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/settings")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "telegram": {
                                "bot_token": "secret-token",
                                "default_group_chat_id": "-10010"
                            },
                            "default_sync_targets": [{
                                "id": "tg-project",
                                "label": "Project topic",
                                "kind": "TelegramForumTopic",
                                "chat_id": "-10010",
                                "message_thread_id": 42
                            }]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let settings: jin_core::store::JinSettings = read_json(response).await;
        assert!(settings.telegram.bot_token.is_none());
        assert!(settings.telegram.bot_token_configured);
        assert_eq!(settings.default_sync_targets.len(), 1);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/settings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let settings: jin_core::store::JinSettings = read_json(response).await;
        assert!(settings.telegram.bot_token.is_none());
        assert!(settings.telegram.bot_token_configured);
        assert_eq!(
            settings.default_sync_targets[0].kind,
            jin_core::sync::SyncTargetKind::TelegramForumTopic
        );
    }

    #[tokio::test]
    async fn content_profile_and_factory_routes_create_project_scoped_pipeline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/projects/jin/content-profile")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "audience": "founders",
                            "language": "ru",
                            "tone": "pragmatic",
                            "persona": "technical founder",
                            "content_pillars": ["agents", "developer tools"],
                            "references": ["https://example.com/ref"],
                            "constraints": ["no hype"],
                            "publish_channels": ["telegram"]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let profile: jin_core::factory::ProjectContentProfile = read_json(response).await;
        assert_eq!(profile.project, "jin");
        assert_eq!(profile.audience.as_deref(), Some("founders"));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/projects/jin/content-profile")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let profile: jin_core::factory::ProjectContentProfile = read_json(response).await;
        assert_eq!(profile.tone.as_deref(), Some("pragmatic"));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/factories")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "title": "Agent content",
                            "brief": "Generate article drafts and image concepts",
                            "mode": "Finite",
                            "review_policy": "PerStage",
                            "content_types": ["Text", "Image"],
                            "sync_targets": []
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let pipeline: jin_core::factory::FactoryPipeline = read_json(response).await;
        assert_eq!(pipeline.project, "jin");
        assert_eq!(
            pipeline.status,
            jin_core::factory::FactoryPipelineStatus::Draft
        );
        assert_eq!(pipeline.stages.len(), 6);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/factories/{}/resume", pipeline.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let resumed: jin_core::factory::FactoryPipeline = read_json(response).await;
        assert_eq!(
            resumed.status,
            jin_core::factory::FactoryPipelineStatus::Scheduled
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/factories/{}/events", pipeline.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let events: Vec<jin_core::factory::FactoryEvent> = read_json(response).await;
        assert!(events
            .iter()
            .any(|event| event.content == "factory pipeline resumed"));
    }

    #[test]
    fn parses_visible_codex_models_from_catalog_json() {
        let raw = r#"{
            "models": [
                {"slug": "gpt-5.5", "visibility": "list"},
                {"slug": "hidden-model", "visibility": "hide"},
                {"slug": "gpt-5.4", "visibility": "list"}
            ]
        }"#;

        let models = parse_codex_model_options(raw).expect("catalog parses");

        assert_eq!(models, vec!["gpt-5.5", "gpt-5.4"]);
    }

    #[tokio::test]
    async fn chat_settings_endpoint_updates_session_and_timeline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app_with_runtime_and_tools(
            temp.path().join("state.json"),
            None,
            ChatRuntimeManager::fake(),
            tools_with_codex_model_options(
                built_in_tools(),
                vec!["gpt-5.5".to_string(), "gpt-5.4".to_string()],
            ),
        )
        .expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/chats")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "tool": "codex",
                            "title": "Mutable Codex",
                            "settings": { "model": "gpt-5.4", "reasoning": "medium" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let chat: ChatSession = read_json(response).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/chats/{}/settings", chat.id))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "settings": {
                                "model": "gpt-5.5",
                                "reasoning": "high"
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let messages: Vec<ChatMessage> = read_json(response).await;
        assert!(messages.iter().any(|message| {
            message.role == ChatRole::System
                && message.content.contains("model: gpt-5.4 -> gpt-5.5")
                && message.content.contains("reasoning: medium -> high")
        }));

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/chats/{}", chat.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let updated: ChatSession = read_json(response).await;
        assert_eq!(
            updated.settings.get("model").map(String::as_str),
            Some("gpt-5.5")
        );
        assert_eq!(
            updated.settings.get("reasoning").map(String::as_str),
            Some("high")
        );
    }

    #[tokio::test]
    async fn chat_progress_endpoint_drains_runtime_output_into_timeline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app_with_runtime(
            temp.path().join("state.json"),
            None,
            ChatRuntimeManager::fake_with_progress(vec![
                RuntimeMessage::append(ChatRole::Tool, "thinking about workspace\n"),
                RuntimeMessage::append(ChatRole::Tool, "running tests"),
            ]),
        )
        .expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/chats")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "tool": "codex",
                            "title": "Progress",
                            "settings": {}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let chat: ChatSession = read_json(response).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/chats/{}/progress", chat.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let progress: ChatProgressResponse = read_json(response).await;
        assert_eq!(progress.chat.status, ChatStatus::Running);
        assert!(progress.messages.iter().any(|message| {
            message.role == ChatRole::Tool
                && message.content == "thinking about workspace\nrunning tests"
        }));
    }

    #[tokio::test]
    async fn chat_progress_endpoint_coalesces_chunked_runtime_output() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app_with_runtime(
            temp.path().join("state.json"),
            None,
            ChatRuntimeManager::fake_with_progress(vec![
                RuntimeMessage::append(ChatRole::Tool, "Ак"),
                RuntimeMessage::append(ChatRole::Tool, "ту"),
                RuntimeMessage::append(ChatRole::Tool, "альный ответ"),
            ]),
        )
        .expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/chats")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "tool": "codex",
                            "title": "Chunked",
                            "settings": {}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let chat: ChatSession = read_json(response).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/chats/{}/progress", chat.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let progress: ChatProgressResponse = read_json(response).await;
        let tool_messages = progress
            .messages
            .iter()
            .filter(|message| message.role == ChatRole::Tool)
            .collect::<Vec<_>>();
        assert_eq!(tool_messages.len(), 1);
        assert_eq!(tool_messages[0].content, "Актуальный ответ");
    }

    #[tokio::test]
    async fn chat_progress_endpoint_preserves_stopped_status_without_runtime_output() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/chats")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "tool": "codex",
                            "title": "Stopped",
                            "settings": {}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let chat: ChatSession = read_json(response).await;

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/chats/{}/stop", chat.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/chats/{}/progress", chat.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let progress: ChatProgressResponse = read_json(response).await;
        assert_eq!(progress.chat.status, ChatStatus::Stopped);
    }

    #[test]
    fn builds_codex_model_command_for_changed_settings() {
        let mut before = sample_chat_session("chat-1");
        before
            .settings
            .insert("model".to_string(), "gpt-5.4".to_string());
        before
            .settings
            .insert("reasoning".to_string(), "medium".to_string());
        let mut after = before.clone();
        after
            .settings
            .insert("model".to_string(), "gpt-5.5".to_string());
        after
            .settings
            .insert("reasoning".to_string(), "high".to_string());

        assert_eq!(
            codex_settings_command(&before, &after).as_deref(),
            Some("/model gpt-5.5 high")
        );
    }

    #[test]
    fn normalize_runtime_output_handles_terminal_line_rewrites() {
        assert_eq!(normalize_runtime_output("thinking\rfinal"), "final");
        assert_eq!(
            normalize_runtime_output("line one\r\nline two"),
            "line one\nline two"
        );
    }

    #[test]
    fn coalesce_runtime_outputs_drops_successful_acceptance_placeholder() {
        assert!(coalesce_runtime_outputs(vec![
            "codex session accepted the message; waiting for output".to_string()
        ])
        .is_empty());
    }

    #[test]
    fn codex_app_server_agent_delta_maps_to_assistant_progress() {
        let mut state = CodexAppServerEventState::default();

        let messages = state.handle_notification(&serde_json::json!({
            "method": "item/agentMessage/delta",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "message-1",
                "delta": "Готово"
            }
        }));

        assert_eq!(
            messages,
            vec![RuntimeMessage::upsert(ChatRole::Assistant, "Готово")]
        );
    }

    #[tokio::test]
    async fn chat_progress_endpoint_updates_single_assistant_stream_message() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app_with_runtime(
            temp.path().join("state.json"),
            None,
            ChatRuntimeManager::fake_with_progress(vec![
                RuntimeMessage::upsert(ChatRole::Assistant, "Ак"),
                RuntimeMessage::upsert(ChatRole::Assistant, "Актуальный ответ"),
            ]),
        )
        .expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/chats")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "tool": "codex",
                            "title": "Streaming",
                            "settings": {}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let chat: ChatSession = read_json(response).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/chats/{}/progress", chat.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let progress: ChatProgressResponse = read_json(response).await;
        let assistant_messages = progress
            .messages
            .iter()
            .filter(|message| message.role == ChatRole::Assistant)
            .collect::<Vec<_>>();
        assert_eq!(assistant_messages.len(), 1);
        assert_eq!(assistant_messages[0].content, "Актуальный ответ");
    }

    #[tokio::test]
    async fn chat_api_lists_tools_creates_chat_posts_messages_and_stops_session() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/tools")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let tools: Vec<jin_core::chat::ToolDescriptor> = read_json(response).await;
        assert!(tools.iter().any(|tool| tool.id == "codex"));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/chats")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "tool": "codex",
                            "title": "Phone Codex",
                            "settings": { "reasoning": "high" }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let chat: jin_core::chat::ChatSession = read_json(response).await;
        assert_eq!(chat.title, "Phone Codex");
        assert_eq!(
            chat.settings.get("reasoning").map(String::as_str),
            Some("high")
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/chats/{}/messages", chat.id))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"content":"status please"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let messages: Vec<jin_core::chat::ChatMessage> = read_json(response).await;
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, jin_core::chat::ChatRole::User);
        assert_eq!(messages[1].role, jin_core::chat::ChatRole::Tool);
        assert!(messages[1].content.contains("status please"));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/chats/{}/messages", chat.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let page: ChatMessagesPage = read_json(response).await;
        assert_eq!(page.messages.len(), 2);
        assert!(!page.has_more);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/chats/{}/stop", chat.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let stopped: jin_core::chat::ChatSession = read_json(response).await;
        assert_eq!(stopped.status, jin_core::chat::ChatStatus::Stopped);
    }

    #[tokio::test]
    async fn chat_messages_endpoint_returns_bounded_pages() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/chats")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "tool": "shell",
                            "title": "Long Chat",
                            "settings": {}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let chat: jin_core::chat::ChatSession = read_json(response).await;

        for index in 0..6 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/chats/{}/messages", chat.id))
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::json!({
                                "content": format!("message {index}")
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/chats/{}/messages?limit=3", chat.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let page: serde_json::Value = read_json(response).await;
        let messages = page["messages"].as_array().expect("messages array");
        assert_eq!(messages.len(), 3);
        assert_eq!(page["has_more"], true);
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .contains("message 4"));
        let before = messages[0]["id"].as_str().expect("message id");

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/chats/{}/messages?limit=2&before={before}",
                        chat.id
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let page: serde_json::Value = read_json(response).await;
        let messages = page["messages"].as_array().expect("older messages array");
        assert_eq!(messages.len(), 2);
        assert_ne!(messages[0]["id"].as_str(), Some(before));
        assert_ne!(messages[1]["id"].as_str(), Some(before));
        assert!(messages[0]["content"]
            .as_str()
            .unwrap()
            .contains("message 3"));
        assert!(messages[1]["content"]
            .as_str()
            .unwrap()
            .contains("message 4"));
    }

    #[tokio::test]
    async fn post_chat_message_returns_bounded_tail() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/chats")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "tool": "shell",
                            "title": "Bounded Post",
                            "settings": {}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let chat: jin_core::chat::ChatSession = read_json(response).await;

        let mut latest_response = None;
        for index in 0..101 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/chats/{}/messages", chat.id))
                        .header("content-type", "application/json")
                        .body(Body::from(
                            serde_json::json!({
                                "content": format!("message {index}")
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            latest_response = Some(response);
        }

        let messages: Vec<jin_core::chat::ChatMessage> =
            read_json(latest_response.expect("latest response")).await;
        assert_eq!(messages.len(), DEFAULT_CHAT_MESSAGE_LIMIT);
        assert!(!messages[0].content.contains("message 0"));
        assert!(messages
            .last()
            .expect("latest message")
            .content
            .contains("message 100"));
    }

    #[tokio::test]
    async fn project_registration_rejects_missing_root_with_400() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing_root = temp.path().join("missing-project");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": missing_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: serde_json::Value = read_json(response).await;
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("project root does not exist"),
            "{body}"
        );
    }

    #[tokio::test]
    async fn project_task_and_approval_flow_works_over_http() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/tasks")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "project": "jin",
                            "runner": "shell",
                            "command": "printf http-approved"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let task: jin_core::orchestrator::TaskRecord = read_json(response).await;
        let approval_id = task.pending_approval_id.expect("approval id");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/approvals")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let approvals: Vec<jin_core::orchestrator::ApprovalRecord> = read_json(response).await;
        assert_eq!(approvals.len(), 1);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/approvals/{approval_id}/approve"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"actor":"nikita"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let task: jin_core::orchestrator::TaskRecord = read_json(response).await;
        assert_eq!(task.state, jin_core::task::TaskState::Completed);
        assert!(task.output.contains("http-approved"));
    }

    #[tokio::test]
    async fn telegram_webhook_creates_shell_task() {
        let temp = tempfile::tempdir().expect("tempdir");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let app = build_app(temp.path().join("state.json")).expect("app builds");

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "jin",
                            "root": project_root,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/telegram/webhook")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "message": {
                                "chat": { "id": 10 },
                                "from": { "id": 20 },
                                "text": "/shell jin printf from-telegram"
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let reply: TelegramSendMessageResponse = read_json(response).await;
        assert_eq!(reply.method, "sendMessage");
        assert_eq!(reply.chat_id, 10);
        assert!(reply.text.contains("state: WaitingApproval"));
        assert!(reply.text.contains("approval:"));
    }

    async fn read_json<T: serde::de::DeserializeOwned>(response: axum::response::Response) -> T {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn sample_chat_session(id: &str) -> ChatSession {
        ChatSession {
            id: id.to_string(),
            title: "Sample".to_string(),
            project: "jin".to_string(),
            tool: "codex".to_string(),
            status: ChatStatus::Idle,
            settings: Default::default(),
            sync_targets: Vec::new(),
            context: jin_core::chat::ContextSummary {
                supported: true,
                used: None,
                limit: None,
                label: "Context pending".to_string(),
            },
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}
