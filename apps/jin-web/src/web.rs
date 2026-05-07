use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Json, Router};
use jin_core::chat::{ChatMessage, ChatRole, ChatSession, ToolDescriptor};
use jin_core::orchestrator::{ApprovalRecord, CreateTaskRequest, ProjectRecord, TaskRecord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct WebConfig {
    api_base: String,
    api_token: Option<String>,
    client: reqwest::Client,
}

impl WebConfig {
    pub fn new(api_base: impl Into<String>, api_token: Option<String>) -> Self {
        Self {
            api_base: api_base.into().trim_end_matches('/').to_string(),
            api_token,
            client: reqwest::Client::new(),
        }
    }
}

pub fn build_app(config: WebConfig) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/chats", post(create_chat))
        .route("/chats/{chat_id}", get(chat_page))
        .route("/chats/{chat_id}/progress", get(chat_progress))
        .route("/chats/{chat_id}/settings", post(update_chat_settings))
        .route("/chats/{chat_id}/messages", post(send_chat_message))
        .route("/chats/{chat_id}/stop", post(stop_chat))
        .route("/projects", get(projects_page).post(register_project))
        .route("/tasks", get(tasks_page).post(create_task))
        .route("/tasks/{task_id}", get(task_detail_page))
        .route("/approvals", get(approvals_page))
        .route("/approvals/{approval_id}/approve", post(approve))
        .route("/approvals/{approval_id}/reject", post(reject))
        .route("/docs", get(docs_index))
        .route("/docs/{topic}", get(docs_topic))
        .with_state(config)
}

pub async fn serve(
    addr: SocketAddr,
    api_base: String,
    api_token: Option<String>,
) -> Result<(), WebError> {
    let app = build_app(WebConfig::new(api_base, api_token));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn dashboard(State(config): State<WebConfig>) -> Html<String> {
    chat_workspace(config, None).await
}

async fn chat_page(State(config): State<WebConfig>, Path(chat_id): Path<String>) -> Html<String> {
    chat_workspace(config, Some(chat_id)).await
}

async fn chat_workspace(config: WebConfig, selected_chat_id: Option<String>) -> Html<String> {
    let projects = config
        .get_json::<Vec<ProjectRecord>>("/projects")
        .await
        .unwrap_or_default();
    let tools = config
        .get_json::<Vec<ToolDescriptor>>("/tools")
        .await
        .unwrap_or_default();
    let chats = config
        .get_json::<Vec<ChatSession>>("/chats")
        .await
        .unwrap_or_default();
    let selected = if let Some(chat_id) = selected_chat_id {
        if let Ok(chat) = config
            .get_json::<ChatSession>(&format!("/chats/{chat_id}"))
            .await
        {
            let messages = config
                .get_json::<Vec<ChatMessage>>(&format!("/chats/{}/messages", chat.id))
                .await
                .unwrap_or_default();
            Some((chat, messages))
        } else {
            None
        }
    } else {
        None
    };

    page(
        "Chats",
        &format!(
            r#"
            <section class="chat-shell">
              <aside class="chat-sidebar">
                <h2>Chats</h2>
                <a class="new-chat-link" href="/">New Chat</a>
                {}
              </aside>
              <section class="chat-main">
                {}
              </section>
            </section>
            "#,
            render_chat_list(&chats),
            render_selected_chat(selected.as_ref(), &tools, &projects)
        ),
    )
}

async fn projects_page(State(config): State<WebConfig>) -> Html<String> {
    let projects = config
        .get_json::<Vec<ProjectRecord>>("/projects")
        .await
        .unwrap_or_default();
    page(
        "Projects",
        &format!(
            r#"
            <section>
              <h2>Register Local Project</h2>
              <form method="post" action="/projects" class="stack">
                <label>Name <input name="name" required></label>
                <label>Root path <input name="root" required></label>
                <button type="submit">Register Project</button>
              </form>
            </section>
            <section>
              <h2>Registered Projects</h2>
              {}
            </section>
            "#,
            render_project_table(&projects)
        ),
    )
}

async fn tasks_page(State(config): State<WebConfig>) -> Html<String> {
    let projects = config
        .get_json::<Vec<ProjectRecord>>("/projects")
        .await
        .unwrap_or_default();
    let tasks = config
        .get_json::<Vec<TaskRecord>>("/tasks")
        .await
        .unwrap_or_default();
    page(
        "Tasks",
        &format!(
            r#"
            <section>
              <h2>Create Task</h2>
              <form method="post" action="/tasks" class="stack">
                <label>Project {}</label>
                <label>Runner
                  <select name="runner">
                    <option value="shell">shell</option>
                    <option value="codex">codex</option>
                  </select>
                </label>
                <label>Command or prompt <textarea name="command" required></textarea></label>
                <button type="submit">Create Task</button>
              </form>
            </section>
            <section>
              <h2>Tasks</h2>
              {}
            </section>
            "#,
            render_project_select(&projects),
            render_task_table(&tasks)
        ),
    )
}

async fn task_detail_page(
    State(config): State<WebConfig>,
    Path(task_id): Path<String>,
) -> Html<String> {
    match config
        .get_json::<TaskRecord>(&format!("/tasks/{task_id}"))
        .await
    {
        Ok(task) => page(
            "Task Detail",
            &format!(
                r#"
                <section>
                  <h2>{}</h2>
                  <dl>
                    <dt>State</dt><dd>{:?}</dd>
                    <dt>Project</dt><dd>{}</dd>
                    <dt>Runner</dt><dd>{}</dd>
                    <dt>Command</dt><dd><code>{}</code></dd>
                    <dt>Exit</dt><dd>{}</dd>
                  </dl>
                  <h3>Output</h3>
                  <pre>{}</pre>
                </section>
                "#,
                escape(&task.id),
                task.state,
                escape(&task.project),
                escape(&task.runner),
                escape(&task.command),
                task.exit_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                escape(&task.output)
            ),
        ),
        Err(error) => error_page("Task not available", &error.to_string()),
    }
}

async fn approvals_page(State(config): State<WebConfig>) -> Html<String> {
    let approvals = config
        .get_json::<Vec<ApprovalRecord>>("/approvals")
        .await
        .unwrap_or_default();
    page("Approvals", &render_approval_table(&approvals))
}

async fn create_chat(
    State(config): State<WebConfig>,
    Form(form): Form<NewChatForm>,
) -> Result<Redirect, WebFormError> {
    let initial_message = form.initial_message.trim().to_string();
    let payload = form.into_payload();
    let chat = config
        .post_json_result::<_, ChatSession>("/chats", &payload)
        .await?;
    if !initial_message.is_empty() {
        config
            .post_json(
                &format!("/chats/{}/messages", chat.id),
                &SendChatMessagePayload {
                    content: initial_message,
                },
            )
            .await?;
    }
    Ok(Redirect::to(&format!("/chats/{}", chat.id)))
}

async fn chat_progress(
    State(config): State<WebConfig>,
    Path(chat_id): Path<String>,
) -> Result<Json<ChatProgressResponse>, WebFormError> {
    let progress = config
        .get_json::<ChatProgressResponse>(&format!("/chats/{chat_id}/progress"))
        .await?;
    Ok(Json(progress))
}

async fn send_chat_message(
    State(config): State<WebConfig>,
    Path(chat_id): Path<String>,
    Form(form): Form<ChatMessageForm>,
) -> Result<Redirect, WebFormError> {
    config
        .post_json(
            &format!("/chats/{chat_id}/messages"),
            &SendChatMessagePayload {
                content: form.content,
            },
        )
        .await?;
    Ok(Redirect::to(&format!("/chats/{chat_id}")))
}

async fn update_chat_settings(
    State(config): State<WebConfig>,
    Path(chat_id): Path<String>,
    Form(form): Form<ChatSettingsForm>,
) -> Result<Redirect, WebFormError> {
    config
        .post_json(&format!("/chats/{chat_id}/settings"), &form.into_payload())
        .await?;
    Ok(Redirect::to(&format!("/chats/{chat_id}")))
}

async fn stop_chat(
    State(config): State<WebConfig>,
    Path(chat_id): Path<String>,
) -> Result<Redirect, WebFormError> {
    config
        .post_json(&format!("/chats/{chat_id}/stop"), &serde_json::json!({}))
        .await?;
    Ok(Redirect::to(&format!("/chats/{chat_id}")))
}

async fn register_project(
    State(config): State<WebConfig>,
    Form(form): Form<ProjectForm>,
) -> Result<Redirect, WebFormError> {
    config.post_json("/projects", &form).await?;
    Ok(Redirect::to("/projects"))
}

async fn create_task(
    State(config): State<WebConfig>,
    Form(form): Form<CreateTaskRequest>,
) -> Result<Redirect, WebFormError> {
    config.post_json("/tasks", &form).await?;
    Ok(Redirect::to("/tasks"))
}

async fn approve(
    State(config): State<WebConfig>,
    Path(approval_id): Path<String>,
) -> Result<Redirect, WebFormError> {
    config
        .post_json(
            &format!("/approvals/{approval_id}/approve"),
            &ApprovalForm {
                actor: "jin-web".to_string(),
            },
        )
        .await?;
    Ok(Redirect::to("/approvals"))
}

async fn reject(
    State(config): State<WebConfig>,
    Path(approval_id): Path<String>,
) -> Result<Redirect, WebFormError> {
    config
        .post_json(
            &format!("/approvals/{approval_id}/reject"),
            &ApprovalForm {
                actor: "jin-web".to_string(),
            },
        )
        .await?;
    Ok(Redirect::to("/approvals"))
}

async fn docs_index() -> Html<String> {
    page(
        "Documentation",
        r#"
        <section>
          <h2>System Docs</h2>
          <div class="doc-grid">
            <a href="/docs/http-api">HTTP API</a>
            <a href="/docs/chats">Chats</a>
            <a href="/docs/telegram">Telegram</a>
            <a href="/docs/runners">Runners</a>
            <a href="/docs/supervisor">Supervisor</a>
            <a href="/docs/state">State</a>
            <a href="/docs/security">Security</a>
          </div>
        </section>
        "#,
    )
}

async fn docs_topic(Path(topic): Path<String>) -> Html<String> {
    let content = match topic.as_str() {
        "http-api" => docs_http_api(),
        "chats" => docs_chats(),
        "telegram" => docs_telegram(),
        "runners" => docs_runners(),
        "supervisor" => docs_supervisor(),
        "state" => docs_state(),
        "security" => docs_security(),
        _ => return error_page("Documentation not found", "Unknown documentation topic."),
    };
    page(content.0, content.1)
}

impl WebConfig {
    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, WebError> {
        let response = self.request(reqwest::Method::GET, path).send().await?;
        Ok(response.error_for_status()?.json::<T>().await?)
    }

    async fn post_json<T: Serialize + ?Sized>(&self, path: &str, body: &T) -> Result<(), WebError> {
        self.post_json_result::<T, serde_json::Value>(path, body)
            .await
            .map(|_| ())
    }

    async fn post_json_result<T: Serialize + ?Sized, R: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R, WebError> {
        let response = self
            .request(reqwest::Method::POST, path)
            .json(body)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|error| format!("failed to read backend error body: {error}"));
            return Err(WebError::Backend {
                status: StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                body,
            });
        }
        Ok(response.json::<R>().await?)
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let builder = self
            .client
            .request(method, format!("{}{}", self.api_base, path));
        if let Some(token) = &self.api_token {
            builder.bearer_auth(token)
        } else {
            builder
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProjectForm {
    name: String,
    root: String,
}

#[derive(Debug, Clone, Deserialize)]
struct NewChatForm {
    #[serde(default)]
    title: String,
    project: String,
    tool: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    reasoning: String,
    #[serde(default)]
    initial_message: String,
}

impl NewChatForm {
    fn into_payload(self) -> CreateChatPayload {
        let mut settings = BTreeMap::new();
        if self.tool == "codex" {
            if !self.model.trim().is_empty() {
                settings.insert("model".to_string(), self.model.trim().to_string());
            }
            if !self.reasoning.trim().is_empty() {
                settings.insert("reasoning".to_string(), self.reasoning);
            }
        }
        CreateChatPayload {
            project: self.project,
            tool: self.tool,
            title: if self.title.trim().is_empty() {
                None
            } else {
                Some(self.title.trim().to_string())
            },
            settings,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct CreateChatPayload {
    project: String,
    tool: String,
    title: Option<String>,
    settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatMessageForm {
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatSettingsForm {
    tool: String,
    model: String,
    reasoning: String,
}

impl ChatSettingsForm {
    fn into_payload(self) -> UpdateChatSettingsPayload {
        let mut settings = BTreeMap::new();
        if self.tool == "codex" {
            if !self.model.trim().is_empty() {
                settings.insert("model".to_string(), self.model.trim().to_string());
            }
            if !self.reasoning.trim().is_empty() {
                settings.insert("reasoning".to_string(), self.reasoning);
            }
        }
        UpdateChatSettingsPayload { settings }
    }
}

#[derive(Debug, Clone, Serialize)]
struct UpdateChatSettingsPayload {
    settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
struct SendChatMessagePayload {
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatProgressResponse {
    chat: ChatSession,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize)]
struct ApprovalForm {
    actor: String,
}

#[derive(Debug)]
pub enum WebError {
    Backend { status: StatusCode, body: String },
    Http(reqwest::Error),
    Io(std::io::Error),
}

impl WebError {
    fn form_status_code(&self) -> StatusCode {
        match self {
            Self::Backend { status, .. } if status.is_client_error() => *status,
            _ => StatusCode::BAD_GATEWAY,
        }
    }
}

impl From<reqwest::Error> for WebError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error)
    }
}

impl From<std::io::Error> for WebError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backend { status, body } => {
                write!(
                    f,
                    "backend request failed: HTTP {status}: {}",
                    backend_error_message(body)
                )
            }
            Self::Http(error) => write!(f, "backend request failed: {error}"),
            Self::Io(error) => write!(f, "web server io error: {error}"),
        }
    }
}

impl std::error::Error for WebError {}

fn backend_error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.to_string())
}

#[derive(Debug)]
pub struct WebFormError(WebError);

impl From<WebError> for WebFormError {
    fn from(error: WebError) -> Self {
        Self(error)
    }
}

impl IntoResponse for WebFormError {
    fn into_response(self) -> Response {
        let status = self.0.form_status_code();
        (
            status,
            Html(error_page_string(
                "Backend request failed",
                &self.0.to_string(),
            )),
        )
            .into_response()
    }
}

fn render_chat_list(chats: &[ChatSession]) -> String {
    if chats.is_empty() {
        return "<p class=\"muted\">No chats yet.</p>".to_string();
    }
    let links = chats
        .iter()
        .rev()
        .map(|chat| {
            format!(
                r#"<a class="chat-link" href="/chats/{}"><strong>{}</strong><span>{} · {} · {:?}</span></a>"#,
                escape(&chat.id),
                escape(&chat.title),
                escape(&chat.project),
                escape(&chat.tool),
                chat.status
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<div class=\"chat-list\">{links}</div>")
}

fn render_new_chat_form(projects: &[ProjectRecord], tools: &[ToolDescriptor]) -> String {
    let project_options = projects
        .iter()
        .map(|project| {
            format!(
                "<option value=\"{}\">{}</option>",
                escape(&project.name),
                escape(&project.name)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let tool_options = tools
        .iter()
        .map(|tool| {
            format!(
                "<option value=\"{}\">{}</option>",
                escape(&tool.id),
                escape(&tool.name)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let project_select = if project_options.is_empty() {
        "<select name=\"project\" required><option value=\"\">Register a project first</option></select>"
            .to_string()
    } else {
        format!("<select name=\"project\" required>{project_options}</select>")
    };
    let tool_select = if tool_options.is_empty() {
        "<select name=\"tool\" required><option value=\"codex\">Codex</option></select>".to_string()
    } else {
        format!("<select name=\"tool\" required>{tool_options}</select>")
    };
    let codex_settings = render_new_chat_settings(tools);

    format!(
        r#"<form method="post" action="/chats" class="new-chat-composer">
          <textarea name="initial_message" placeholder="Ask jin to work on something..." autofocus></textarea>
          <div class="composer-toolbar">
            <label>Project {project_select}</label>
            <label>Tool {tool_select}</label>
            {codex_settings}
            <label class="title-field">Title <input name="title" placeholder="Optional"></label>
            <button type="submit">Start</button>
          </div>
        </form>"#
    )
}

fn render_new_chat_settings(tools: &[ToolDescriptor]) -> String {
    let Some(codex) = tools.iter().find(|tool| tool.id == "codex") else {
        return String::new();
    };
    codex
        .settings
        .iter()
        .filter(|setting| setting.id == "model" || setting.id == "reasoning")
        .map(render_setting_control)
        .collect::<Vec<_>>()
        .join("")
}

fn render_setting_control(setting: &jin_core::chat::ToolSettingDescriptor) -> String {
    match setting.kind {
        jin_core::chat::ToolSettingKind::Select => {
            let options = setting
                .options
                .iter()
                .map(|option| {
                    let selected = if setting.default.as_deref() == Some(option.as_str()) {
                        " selected"
                    } else {
                        ""
                    };
                    format!(
                        r#"<option value="{}"{}>{}</option>"#,
                        escape(option),
                        selected,
                        escape(option)
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            format!(
                r#"<label>{} <select name="{}">{}</select></label>"#,
                escape(&setting.label),
                escape(&setting.id),
                options
            )
        }
        jin_core::chat::ToolSettingKind::Text => format!(
            r#"<label>{} <input name="{}" placeholder="{}"></label>"#,
            escape(&setting.label),
            escape(&setting.id),
            escape(setting.default.as_deref().unwrap_or("default"))
        ),
    }
}

fn render_selected_chat(
    selected: Option<&(ChatSession, Vec<ChatMessage>)>,
    tools: &[ToolDescriptor],
    projects: &[ProjectRecord],
) -> String {
    let Some((chat, messages)) = selected else {
        return format!(
            r#"<div class="empty-chat">
          <h2>Start a new chat</h2>
          <p class="muted">Choose a project and send the first prompt. Details stay in the toolbar.</p>
          {}
        </div>"#,
            render_new_chat_form(projects, tools)
        );
    };

    format!(
        r#"
        <header class="chat-header">
          <div>
            <h2>{}</h2>
            <div class="chips">
              <span>{}</span>
              <span>{}</span>
              <span>{:?}</span>
              {}
            </div>
          </div>
          <form method="post" action="/chats/{}/stop"><button class="secondary" type="submit">Stop</button></form>
        </header>
        <details class="chat-settings">
          <summary>Settings</summary>
          {}
        </details>
        <div class="agent-progress {}">
          <span class="pulse"></span>
          <span data-progress-label>{}</span>
        </div>
        <div class="timeline" data-timeline>
          {}
        </div>
        <form method="post" action="/chats/{}/messages" class="chat-composer" data-chat-form data-progress-url="/chats/{}/progress">
          <textarea name="content" required placeholder="Message {}"></textarea>
          <button type="submit">Send</button>
        </form>
        "#,
        escape(&chat.title),
        escape(&chat.project),
        escape(&chat.tool),
        chat.status,
        render_context_chip(chat),
        escape(&chat.id),
        render_chat_settings_form(chat, tools),
        if chat.status == jin_core::chat::ChatStatus::Running {
            "active"
        } else {
            ""
        },
        if chat.status == jin_core::chat::ChatStatus::Running {
            "Agent is working"
        } else {
            "Agent is idle"
        },
        render_chat_messages(messages),
        escape(&chat.id),
        escape(&chat.id),
        escape(&chat.tool)
    )
}

fn render_context_chip(chat: &ChatSession) -> String {
    if !chat.context.supported {
        return String::new();
    }
    let label = match (chat.context.used, chat.context.limit) {
        (Some(used), Some(limit)) => format!("Context {used}/{limit}"),
        _ => "Context pending".to_string(),
    };
    format!("<span>{}</span>", escape(&label))
}

fn render_chat_settings_form(chat: &ChatSession, tools: &[ToolDescriptor]) -> String {
    let Some(tool) = tools.iter().find(|tool| tool.id == chat.tool) else {
        return render_chat_settings(chat);
    };
    let controls = tool
        .settings
        .iter()
        .filter(|setting| setting.id == "model" || setting.id == "reasoning")
        .map(|setting| render_chat_setting_control(setting, chat.settings.get(&setting.id)))
        .collect::<Vec<_>>()
        .join("");
    if controls.is_empty() {
        return render_chat_settings(chat);
    }
    format!(
        r#"<form method="post" action="/chats/{}/settings" class="stack">
          <input type="hidden" name="tool" value="{}">
          {}
          <button type="submit">Update Settings</button>
        </form>"#,
        escape(&chat.id),
        escape(&chat.tool),
        controls
    )
}

fn render_chat_setting_control(
    setting: &jin_core::chat::ToolSettingDescriptor,
    current: Option<&String>,
) -> String {
    match setting.kind {
        jin_core::chat::ToolSettingKind::Select => {
            let selected_value = current
                .map(String::as_str)
                .or(setting.default.as_deref())
                .unwrap_or_default();
            let options = setting
                .options
                .iter()
                .map(|option| {
                    let selected = if option == selected_value {
                        " selected"
                    } else {
                        ""
                    };
                    format!(
                        r#"<option value="{}"{}>{}</option>"#,
                        escape(option),
                        selected,
                        escape(option)
                    )
                })
                .collect::<Vec<_>>()
                .join("");
            format!(
                r#"<label>{} <select name="{}">{}</select></label>"#,
                escape(&setting.label),
                escape(&setting.id),
                options
            )
        }
        jin_core::chat::ToolSettingKind::Text => format!(
            r#"<label>{} <input name="{}" value="{}"></label>"#,
            escape(&setting.label),
            escape(&setting.id),
            escape(current.map(String::as_str).unwrap_or_default())
        ),
    }
}

fn render_chat_settings(chat: &ChatSession) -> String {
    if chat.settings.is_empty() {
        return "<p class=\"muted\">No configurable settings for this tool.</p>".to_string();
    }
    let rows = chat
        .settings
        .iter()
        .map(|(key, value)| {
            format!(
                "<tr><th>{}</th><td>{}</td></tr>",
                escape(&setting_label(key)),
                escape(value)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<table><tbody>{rows}</tbody></table>")
}

fn setting_label(key: &str) -> String {
    key.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_chat_messages(messages: &[ChatMessage]) -> String {
    if messages.is_empty() {
        return "<p class=\"muted\">No messages yet.</p>".to_string();
    }
    messages
        .iter()
        .map(|message| {
            let class = match message.role {
                ChatRole::User => "message user",
                ChatRole::Assistant => "message assistant",
                ChatRole::Tool => "message tool",
                ChatRole::System => "message system",
                ChatRole::Error => "message error",
            };
            format!(
                r#"<article class="{class}"><strong>{:?}</strong><pre>{}</pre></article>"#,
                message.role,
                escape(&message.content)
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn render_project_table(projects: &[ProjectRecord]) -> String {
    if projects.is_empty() {
        return "<p class=\"muted\">No projects registered.</p>".to_string();
    }
    let rows = projects
        .iter()
        .map(|project| {
            format!(
                "<tr><td>{}</td><td><code>{}</code></td></tr>",
                escape(&project.name),
                escape(&project.root.display().to_string())
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(
        "<table><thead><tr><th>Name</th><th>Root</th></tr></thead><tbody>{rows}</tbody></table>"
    )
}

fn render_task_table(tasks: &[TaskRecord]) -> String {
    if tasks.is_empty() {
        return "<p class=\"muted\">No tasks yet.</p>".to_string();
    }
    let rows = tasks
        .iter()
        .rev()
        .map(|task| {
            format!(
                "<tr><td><a href=\"/tasks/{}\">{}</a></td><td>{:?}</td><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
                escape(&task.id),
                short_id(&task.id),
                task.state,
                escape(&task.project),
                escape(&task.runner),
                escape(&task.command)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<table><thead><tr><th>Task</th><th>State</th><th>Project</th><th>Runner</th><th>Command</th></tr></thead><tbody>{rows}</tbody></table>")
}

fn render_approval_table(approvals: &[ApprovalRecord]) -> String {
    if approvals.is_empty() {
        return "<p class=\"muted\">No approvals yet.</p>".to_string();
    }
    let rows = approvals
        .iter()
        .rev()
        .map(|approval| {
            let actions = if approval.decision.is_none() {
                format!(
                    r#"<form method="post" action="/approvals/{}/approve"><button type="submit">Approve</button></form>
                       <form method="post" action="/approvals/{}/reject"><button class="danger" type="submit">Reject</button></form>"#,
                    escape(&approval.id),
                    escape(&approval.id)
                )
            } else {
                escape(approval.decision.as_deref().unwrap_or("-"))
            };
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                short_id(&approval.id),
                escape(&approval.operation),
                escape(&approval.reason),
                actions
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<table><thead><tr><th>Approval</th><th>Operation</th><th>Reason</th><th>Decision</th></tr></thead><tbody>{rows}</tbody></table>")
}

fn render_project_select(projects: &[ProjectRecord]) -> String {
    let options = projects
        .iter()
        .map(|project| {
            format!(
                "<option value=\"{}\">{}</option>",
                escape(&project.name),
                escape(&project.name)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<select name=\"project\" required>{options}</select>")
}

fn docs_http_api() -> (&'static str, &'static str) {
    (
        "HTTP API",
        r#"<section><h2>HTTP API</h2><p>The backend listens on <code>jin-server</code> and exposes JSON endpoints for health, tools, chats, projects, tasks, approvals, and Telegram webhook input.</p><ul><li><code>GET /health</code></li><li><code>GET /tools</code></li><li><code>GET/POST /chats</code></li><li><code>GET /chats/{chat_id}</code></li><li><code>GET/POST /chats/{chat_id}/messages</code></li><li><code>GET /chats/{chat_id}/progress</code></li><li><code>POST /chats/{chat_id}/settings</code></li><li><code>POST /chats/{chat_id}/stop</code></li><li><code>GET/POST /projects</code></li><li><code>GET/POST /tasks</code></li><li><code>GET /tasks/{task_id}</code></li><li><code>GET /approvals</code></li><li><code>POST /approvals/{approval_id}/approve</code></li><li><code>POST /approvals/{approval_id}/reject</code></li></ul></section>"#,
    )
}

fn docs_chats() -> (&'static str, &'static str) {
    (
        "Chats",
        r#"<section><h2>Chats</h2><p>The root web page is a phone-friendly chat workspace with a focused new-chat composer. Choose a project and native tool from the compact toolbar, then send the first prompt. Tool-specific controls such as Codex model and reasoning are shown only when the selected tool exposes that capability.</p><p>Codex chats are backed by a persistent CLI session owned by <code>jin-server</code>, so the browser can reconnect to the durable timeline. The web UI polls <code>/chats/{chat_id}/progress</code> while a native session is active and appends new CLI output to the chat timeline.</p></section>"#,
    )
}

fn docs_telegram() -> (&'static str, &'static str) {
    (
        "Telegram",
        r#"<section><h2>Telegram</h2><p>The Telegram adapter accepts webhook updates at <code>/telegram/webhook</code>.</p><ul><li><code>/shell &lt;project&gt; &lt;command&gt;</code></li><li><code>/codex &lt;project&gt; &lt;prompt&gt;</code></li><li><code>/approve &lt;approval-id&gt;</code></li><li><code>/reject &lt;approval-id&gt;</code></li></ul></section>"#,
    )
}

fn docs_runners() -> (&'static str, &'static str) {
    (
        "Runners",
        r#"<section><h2>Runners</h2><p><code>shell</code> executes controlled commands in a registered local project. Commands outside the allowlist require approval. <code>codex</code> invokes <code>codex exec</code> and is approval-gated in this MVP.</p></section>"#,
    )
}

fn docs_supervisor() -> (&'static str, &'static str) {
    (
        "Supervisor",
        r#"<section><h2>Supervisor</h2><p><code>jin-supervisor</code> owns stable/candidate version state and rollback. Use <code>init-stable</code>, <code>set-candidate</code>, <code>promote</code>, and <code>rollback</code>.</p></section>"#,
    )
}

fn docs_state() -> (&'static str, &'static str) {
    (
        "State",
        r#"<section><h2>State</h2><p>The MVP stores durable state as JSON under <code>.jin/state</code>: registered projects, tasks, approvals, and version registry files.</p></section>"#,
    )
}

fn docs_security() -> (&'static str, &'static str) {
    (
        "Security",
        r#"<section><h2>Security</h2><p>Use <code>JIN_API_TOKEN</code> for backend API protection. The frontend keeps that token server-side. Risky operations become durable approvals before execution.</p></section>"#,
    )
}

fn page(title: &str, body: &str) -> Html<String> {
    Html(page_string(title, body))
}

fn error_page(title: &str, message: &str) -> Html<String> {
    Html(error_page_string(title, message))
}

fn error_page_string(title: &str, message: &str) -> String {
    page_string(
        title,
        &format!("<section><p>{}</p></section>", escape(message)),
    )
}

fn page_string(title: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{} - jin</title>
  <style>
    :root {{ color-scheme: light; --text:#1f2328; --muted:#59636e; --line:#d0d7de; --bg:#f6f8fa; --accent:#0969da; --danger:#cf222e; }}
    * {{ box-sizing: border-box; }}
    body {{ margin:0; font:14px/1.45 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; color:var(--text); background:white; }}
    header {{ border-bottom:1px solid var(--line); background:var(--bg); }}
    .wrap {{ max-width:1180px; margin:0 auto; padding:18px 20px; }}
    nav {{ display:flex; gap:14px; flex-wrap:wrap; align-items:center; }}
    nav a {{ color:var(--accent); text-decoration:none; font-weight:600; }}
    main {{ display:grid; gap:18px; }}
    section, article {{ border:1px solid var(--line); border-radius:8px; padding:16px; background:#fff; }}
    h1 {{ font-size:28px; margin:0 0 14px; }}
    h2 {{ font-size:18px; margin:0 0 12px; }}
    h3 {{ font-size:15px; margin:16px 0 8px; }}
    table {{ width:100%; border-collapse:collapse; }}
    th, td {{ text-align:left; border-bottom:1px solid var(--line); padding:8px; vertical-align:top; }}
    input, select, textarea {{ width:100%; border:1px solid var(--line); border-radius:6px; padding:8px; font:inherit; }}
    textarea {{ min-height:96px; resize:vertical; }}
    button {{ border:0; border-radius:6px; background:var(--accent); color:white; padding:8px 12px; font-weight:700; cursor:pointer; }}
    button.secondary {{ background:#59636e; }}
    button.danger {{ background:var(--danger); }}
    code, pre {{ background:var(--bg); border-radius:6px; }}
    code {{ padding:2px 4px; }}
    pre {{ padding:12px; overflow:auto; white-space:pre-wrap; }}
    form {{ margin:0; }}
    .stack {{ display:grid; gap:12px; max-width:680px; }}
    .grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(180px,1fr)); gap:12px; }}
    .metric {{ display:block; font-size:30px; font-weight:800; }}
    .muted {{ color:var(--muted); }}
    .doc-grid {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(180px,1fr)); gap:10px; }}
    .doc-grid a {{ display:block; border:1px solid var(--line); border-radius:8px; padding:14px; text-decoration:none; color:var(--accent); font-weight:700; }}
    dl {{ display:grid; grid-template-columns:120px 1fr; gap:8px 14px; }}
    dt {{ font-weight:700; }}
    .chat-shell {{ display:grid; grid-template-columns:minmax(240px,320px) 1fr; gap:16px; min-height:calc(100vh - 150px); }}
    .chat-shell, .chat-main, .chat-sidebar {{ border:0; padding:0; background:transparent; }}
    .chat-sidebar {{ align-self:start; display:grid; gap:12px; }}
    .chat-main {{ display:grid; grid-template-rows:auto auto auto 1fr auto; gap:12px; min-height:620px; }}
    .new-chat-link {{ display:block; border:1px solid var(--line); border-radius:8px; padding:10px; color:var(--accent); text-decoration:none; font-weight:700; background:#fff; }}
    .chat-list {{ display:grid; gap:8px; margin-bottom:18px; }}
    .chat-link {{ display:grid; gap:2px; border:1px solid var(--line); border-radius:8px; padding:10px; color:var(--text); text-decoration:none; }}
    .chat-link span, .chips span {{ color:var(--muted); font-size:12px; }}
    .chat-header {{ display:flex; justify-content:space-between; gap:12px; align-items:flex-start; border:0; border-bottom:1px solid var(--line); border-radius:0; padding:0 0 12px; }}
    .chips {{ display:flex; flex-wrap:wrap; gap:8px; margin-top:6px; }}
    .chips span {{ border:1px solid var(--line); border-radius:999px; padding:3px 8px; }}
    .chat-settings {{ border:1px solid var(--line); border-radius:8px; padding:10px; }}
    .timeline {{ display:grid; align-content:start; gap:10px; min-height:280px; overflow:auto; }}
    .agent-progress {{ display:none; align-items:center; gap:8px; color:var(--muted); border:1px solid var(--line); border-radius:8px; padding:8px 10px; background:#fff; }}
    .agent-progress.active {{ display:flex; }}
    .pulse {{ width:8px; height:8px; border-radius:50%; background:var(--accent); animation:pulse 1.2s infinite ease-in-out; }}
    @keyframes pulse {{ 0%,100% {{ opacity:.35; transform:scale(.8); }} 50% {{ opacity:1; transform:scale(1.15); }} }}
    .message {{ max-width:88%; border:1px solid var(--line); border-radius:8px; padding:10px; }}
    .message.user {{ justify-self:end; background:#f6f8fa; }}
    .message.tool, .message.assistant, .message.system {{ justify-self:start; }}
    .message.error {{ justify-self:start; border-color:var(--danger); }}
    .message pre {{ margin:6px 0 0; background:transparent; padding:0; }}
    .chat-composer {{ display:grid; grid-template-columns:1fr auto; gap:10px; align-items:end; position:sticky; bottom:0; background:white; padding-top:8px; }}
    .chat-composer textarea {{ min-height:74px; }}
    .empty-chat {{ align-self:center; justify-self:center; width:min(100%,680px); display:grid; gap:12px; }}
    .new-chat-composer {{ display:grid; gap:10px; width:100%; }}
    .new-chat-composer textarea {{ min-height:132px; }}
    .composer-toolbar {{ display:grid; grid-template-columns:repeat(4,minmax(120px,1fr)) auto; gap:8px; align-items:end; }}
    .composer-toolbar label {{ font-size:12px; color:var(--muted); }}
    .composer-toolbar input, .composer-toolbar select {{ margin-top:3px; }}
    @media (max-width: 760px) {{
      .chat-shell {{ grid-template-columns:1fr; }}
      .chat-main {{ min-height:520px; }}
      .chat-composer {{ grid-template-columns:1fr; }}
      .composer-toolbar {{ grid-template-columns:1fr; }}
      .message {{ max-width:100%; }}
    }}
  </style>
</head>
<body>
  <header><div class="wrap"><nav><strong>jin</strong><a href="/">Chats</a><a href="/projects">Projects</a><a href="/tasks">Tasks</a><a href="/approvals">Approvals</a><a href="/docs">Docs</a></nav></div></header>
  <div class="wrap"><main><h1>{}</h1>{}</main></div>
  <script>
    (() => {{
      const form = document.querySelector('[data-chat-form]');
      if (!form) return;
      const progressUrl = form.dataset.progressUrl;
      const timeline = document.querySelector('[data-timeline]');
      const progress = document.querySelector('.agent-progress');
      const label = document.querySelector('[data-progress-label]');
      const escapeHtml = (value) => String(value)
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#39;');
      const messageClass = (role) => {{
        const normalized = String(role).toLowerCase();
        if (normalized === 'user') return 'message user';
        if (normalized === 'assistant') return 'message assistant';
        if (normalized === 'tool') return 'message tool';
        if (normalized === 'system') return 'message system';
        return 'message error';
      }};
      const renderMessages = (messages) => {{
        timeline.innerHTML = messages.map((message) => (
          `<article class="${{messageClass(message.role)}}"><strong>${{escapeHtml(message.role)}}</strong><pre>${{escapeHtml(message.content)}}</pre></article>`
        )).join('');
      }};
      const setProgress = (status) => {{
        const active = status === 'Running';
        progress.classList.toggle('active', active);
        label.textContent = active ? 'Agent is working' : 'Agent is idle';
      }};
      const poll = async () => {{
        try {{
          const response = await fetch(progressUrl, {{ headers: {{ 'accept': 'application/json' }} }});
          if (!response.ok) return;
          const data = await response.json();
          renderMessages(data.messages || []);
          setProgress(data.chat && data.chat.status);
        }} catch (_) {{}}
      }};
      poll();
      window.setInterval(poll, 1200);
    }})();
  </script>
</body>
</html>"#,
        escape(title),
        escape(title),
        body
    )
}

fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn root_renders_chat_workspace_instead_of_dashboard() {
        let app = build_app(WebConfig::new("http://127.0.0.1:9", None));
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_body(response).await;
        assert!(body.contains("New Chat"));
        assert!(body.contains("chat-composer"));
        assert!(!body.contains("Recent Tasks"));
    }

    #[tokio::test]
    async fn root_renders_new_chat_composer_without_autoselecting_existing_chat() {
        let backend = Router::new()
            .route(
                "/tools",
                get(|| async { axum::Json(Vec::<jin_core::chat::ToolDescriptor>::new()) }),
            )
            .route(
                "/projects",
                get(|| async {
                    axum::Json(vec![ProjectRecord {
                        name: "jin".to_string(),
                        root: "/tmp/jin".into(),
                    }])
                }),
            )
            .route(
                "/chats",
                get(|| async {
                    axum::Json(vec![jin_core::chat::ChatSession {
                        id: "chat-1".to_string(),
                        title: "Existing Chat".to_string(),
                        project: "jin".to_string(),
                        tool: "codex".to_string(),
                        status: jin_core::chat::ChatStatus::Idle,
                        settings: Default::default(),
                        context: jin_core::chat::ContextSummary {
                            supported: true,
                            used: None,
                            limit: None,
                            label: "Context pending".to_string(),
                        },
                        sync_targets: Vec::new(),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    }])
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener binds");
        let addr = listener.local_addr().expect("listener address");
        let server = tokio::spawn(async move {
            axum::serve(listener, backend)
                .await
                .expect("backend serves");
        });

        let app = build_app(WebConfig::new(format!("http://{addr}"), None));
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        server.abort();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_body(response).await;
        assert!(body.contains("Start a new chat"), "{body}");
        assert!(
            body.contains(r#"<form method="post" action="/chats" class="new-chat-composer""#),
            "{body}"
        );
        assert!(!body.contains(r#"<h2>Existing Chat</h2>"#), "{body}");
        assert!(!body.contains("<h2>New Chat</h2>"), "{body}");
    }

    #[test]
    fn new_chat_form_renders_model_select_from_tool_descriptor() {
        let tools = vec![jin_core::chat::ToolDescriptor {
            id: "codex".to_string(),
            name: "Codex".to_string(),
            supports_persistent_session: true,
            supports_context_meter: true,
            settings: vec![jin_core::chat::ToolSettingDescriptor {
                id: "model".to_string(),
                label: "Model".to_string(),
                kind: jin_core::chat::ToolSettingKind::Select,
                options: vec!["gpt-5.4".to_string(), "gpt-5.3-codex".to_string()],
                default: Some("gpt-5.4".to_string()),
            }],
        }];
        let projects = vec![ProjectRecord {
            name: "jin".to_string(),
            root: "/tmp/jin".into(),
        }];

        let html = render_new_chat_form(&projects, &tools);

        assert!(html.contains(r#"<select name="model""#), "{html}");
        assert!(
            html.contains(r#"<option value="gpt-5.4" selected>gpt-5.4</option>"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<option value="gpt-5.3-codex">gpt-5.3-codex</option>"#),
            "{html}"
        );
        assert!(!html.contains(r#"<input name="model""#), "{html}");
    }

    #[test]
    fn shell_chat_payload_drops_codex_only_settings() {
        let payload = NewChatForm {
            title: "Shell".to_string(),
            project: "jin".to_string(),
            tool: "shell".to_string(),
            model: "gpt-5.4".to_string(),
            reasoning: "high".to_string(),
            initial_message: String::new(),
        }
        .into_payload();

        assert_eq!(payload.tool, "shell");
        assert!(payload.settings.is_empty());
    }

    #[tokio::test]
    async fn chat_detail_renders_header_timeline_settings_and_composer() {
        let backend = Router::new()
            .route(
                "/tools",
                get(|| async {
                    axum::Json(vec![jin_core::chat::ToolDescriptor {
                        id: "codex".to_string(),
                        name: "Codex".to_string(),
                        supports_persistent_session: true,
                        supports_context_meter: true,
                        settings: vec![
                            jin_core::chat::ToolSettingDescriptor {
                                id: "model".to_string(),
                                label: "Model".to_string(),
                                kind: jin_core::chat::ToolSettingKind::Select,
                                options: vec!["gpt-5.5".to_string(), "gpt-5.4".to_string()],
                                default: Some("gpt-5.5".to_string()),
                            },
                            jin_core::chat::ToolSettingDescriptor {
                                id: "reasoning".to_string(),
                                label: "Reasoning".to_string(),
                                kind: jin_core::chat::ToolSettingKind::Select,
                                options: vec![
                                    "low".to_string(),
                                    "medium".to_string(),
                                    "high".to_string(),
                                    "xhigh".to_string(),
                                ],
                                default: Some("medium".to_string()),
                            },
                        ],
                    }])
                }),
            )
            .route(
                "/projects",
                get(|| async {
                    axum::Json(vec![ProjectRecord {
                        name: "jin".to_string(),
                        root: "/tmp/jin".into(),
                    }])
                }),
            )
            .route(
                "/chats",
                get(|| async {
                    axum::Json(vec![jin_core::chat::ChatSession {
                        id: "chat-1".to_string(),
                        title: "Phone Codex".to_string(),
                        project: "jin".to_string(),
                        tool: "codex".to_string(),
                        status: jin_core::chat::ChatStatus::Idle,
                        settings: std::collections::BTreeMap::from([
                            ("model".to_string(), "gpt-5.5".to_string()),
                            ("reasoning".to_string(), "high".to_string()),
                        ]),
                        context: jin_core::chat::ContextSummary {
                            supported: true,
                            used: Some(128),
                            limit: Some(1000),
                            label: "128 / 1000".to_string(),
                        },
                        sync_targets: Vec::new(),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    }])
                }),
            )
            .route(
                "/chats/chat-1",
                get(|| async {
                    axum::Json(jin_core::chat::ChatSession {
                        id: "chat-1".to_string(),
                        title: "Phone Codex".to_string(),
                        project: "jin".to_string(),
                        tool: "codex".to_string(),
                        status: jin_core::chat::ChatStatus::Idle,
                        settings: std::collections::BTreeMap::from([
                            ("model".to_string(), "gpt-5.5".to_string()),
                            ("reasoning".to_string(), "high".to_string()),
                        ]),
                        context: jin_core::chat::ContextSummary {
                            supported: true,
                            used: Some(128),
                            limit: Some(1000),
                            label: "128 / 1000".to_string(),
                        },
                        sync_targets: Vec::new(),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    })
                }),
            )
            .route(
                "/chats/chat-1/messages",
                get(|| async {
                    axum::Json(vec![jin_core::chat::ChatMessage {
                        id: "message-1".to_string(),
                        chat_id: "chat-1".to_string(),
                        role: jin_core::chat::ChatRole::User,
                        content: "ship it".to_string(),
                        created_at: chrono::Utc::now(),
                    }])
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener binds");
        let addr = listener.local_addr().expect("listener address");
        let server = tokio::spawn(async move {
            axum::serve(listener, backend)
                .await
                .expect("backend serves");
        });

        let app = build_app(WebConfig::new(format!("http://{addr}"), None));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/chats/chat-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        server.abort();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_body(response).await;
        assert!(body.contains("Phone Codex"));
        assert!(body.contains(r#"/chats/chat-1/settings"#));
        assert!(body.contains("Update Settings"));
        assert!(body.contains(r#"<option value="gpt-5.5" selected>gpt-5.5</option>"#));
        assert!(body.contains("Reasoning"));
        assert!(body.contains("high"));
        assert!(body.contains("ship it"));
        assert!(body.contains("chat-composer"));
        assert!(body.contains(r#"data-progress-url="/chats/chat-1/progress""#));
        assert!(body.contains("agent-progress"));
    }

    #[tokio::test]
    async fn dashboard_is_served_at_root_without_ui_prefix() {
        let app = build_app(WebConfig::new("http://127.0.0.1:9", None));
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_body(response).await;
        assert!(body.contains("jin"));
        assert!(body.contains("/projects"));
        assert!(!body.contains("/ui"));
    }

    #[tokio::test]
    async fn form_errors_preserve_backend_client_error_status_and_message() {
        let backend = Router::new().route(
            "/projects",
            post(|| async {
                (
                    StatusCode::BAD_REQUEST,
                    axum::Json(serde_json::json!({
                        "error": "project root does not exist: /missing"
                    })),
                )
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener binds");
        let addr = listener.local_addr().expect("listener address");
        let server = tokio::spawn(async move {
            axum::serve(listener, backend)
                .await
                .expect("backend serves");
        });

        let app = build_app(WebConfig::new(format!("http://{addr}"), None));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("name=bad&root=%2Fmissing"))
                    .unwrap(),
            )
            .await
            .unwrap();
        server.abort();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = read_body(response).await;
        assert!(body.contains("project root does not exist"), "{body}");
        assert!(!body.contains("{&quot;error&quot;"), "{body}");
    }

    #[tokio::test]
    async fn docs_index_links_to_all_system_docs() {
        let app = build_app(WebConfig::new("http://127.0.0.1:9", None));
        let response = app
            .oneshot(Request::builder().uri("/docs").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_body(response).await;
        for path in [
            "/docs/http-api",
            "/docs/chats",
            "/docs/telegram",
            "/docs/runners",
            "/docs/supervisor",
            "/docs/state",
            "/docs/security",
        ] {
            assert!(body.contains(path), "missing {path}");
        }
    }

    #[tokio::test]
    async fn docs_pages_describe_core_functions() {
        let app = build_app(WebConfig::new("http://127.0.0.1:9", None));
        for (path, expected) in [
            ("/docs/http-api", "GET/POST /tasks"),
            ("/docs/chats", "phone-friendly chat workspace"),
            ("/docs/telegram", "/approve"),
            ("/docs/runners", "codex exec"),
            ("/docs/supervisor", "rollback"),
            ("/docs/state", ".jin/state"),
            ("/docs/security", "JIN_API_TOKEN"),
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = read_body(response).await;
            assert!(body.contains(expected), "{path} missing {expected}");
        }
    }

    async fn read_body(response: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }
}
