use crate::chat::{
    built_in_tools, ChatMessage, ChatRole, ChatSession, ChatStatus, ContextSummary, ToolDescriptor,
};
use crate::factory::{
    default_factory_stages, CreateFactoryPipelineRequest, FactoryEvent, FactoryEventKind,
    FactoryPipeline, FactoryPipelineStatus, ProjectContentProfile, ProjectContentProfileUpdate,
};
use crate::policy::{GuardedAction, PolicyConfig, PolicyDecision, PolicyEngine};
use crate::runner::{CodexRunner, RunnerAdapter, RunnerRequest, ShellRunner};
use crate::store::{FileStore, JinSettings, StoreError};
use crate::sync::{normalize_sync_targets, redacted_telegram_settings, SyncTarget};
use crate::task::TaskState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub name: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub project: String,
    pub runner: String,
    pub command: String,
    pub state: TaskState,
    pub pending_approval_id: Option<String>,
    pub output: String,
    pub exit_code: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub id: String,
    pub task_id: String,
    pub operation: String,
    pub reason: String,
    pub decided_by: Option<String>,
    pub decision: Option<String>,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub project: String,
    pub runner: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateChatRequest {
    pub project: String,
    pub tool: String,
    pub title: Option<String>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
    #[serde(default)]
    pub sync_targets: Option<Vec<SyncTarget>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostChatMessageRequest {
    pub chat_id: String,
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateChatSettingsRequest {
    pub chat_id: String,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
}

#[derive(Debug)]
pub struct JinOrchestrator {
    store: FileStore,
    policy: PolicyEngine,
    tools: Vec<ToolDescriptor>,
}

impl JinOrchestrator {
    pub fn new(store: FileStore) -> Self {
        Self {
            store,
            policy: PolicyEngine::new(PolicyConfig {
                allowed_shell_commands: vec!["git status".to_string()],
            }),
            tools: built_in_tools(),
        }
    }

    pub fn new_with_tools(store: FileStore, tools: Vec<ToolDescriptor>) -> Self {
        Self {
            store,
            policy: PolicyEngine::new(PolicyConfig {
                allowed_shell_commands: vec!["git status".to_string()],
            }),
            tools,
        }
    }

    pub fn register_project(
        &mut self,
        name: impl Into<String>,
        root: PathBuf,
    ) -> Result<ProjectRecord, OrchestratorError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(OrchestratorError::InvalidInput(
                "project name is blank".to_string(),
            ));
        }
        let root = canonicalize_project_root(&root)?;
        let project = ProjectRecord { name, root };

        self.store
            .state_mut()
            .projects
            .retain(|existing| existing.name != project.name);
        self.store.state_mut().projects.push(project.clone());
        self.store.save()?;
        Ok(project)
    }

    pub fn list_projects(&self) -> Vec<ProjectRecord> {
        self.store.state().projects.clone()
    }

    pub fn settings(&self) -> JinSettings {
        redact_settings(&self.store.state().settings)
    }

    pub fn update_settings(
        &mut self,
        settings: JinSettings,
    ) -> Result<JinSettings, OrchestratorError> {
        let existing_token = self.store.state().settings.telegram.bot_token.clone();
        let bot_token = match settings.telegram.bot_token {
            Some(token) => normalize_optional_string(&token),
            None => existing_token,
        };
        let bot_token_configured = bot_token.is_some();
        let settings = JinSettings {
            public_host: settings
                .public_host
                .and_then(|host| normalize_optional_string(&host)),
            telegram: crate::sync::TelegramSettings {
                bot_token,
                bot_token_configured,
                default_group_chat_id: settings
                    .telegram
                    .default_group_chat_id
                    .and_then(|chat_id| normalize_optional_string(&chat_id)),
            },
            default_sync_targets: normalize_sync_targets(settings.default_sync_targets),
        };
        self.store.state_mut().settings = settings.clone();
        self.store.save()?;
        Ok(redact_settings(&settings))
    }

    pub fn list_tools(&self) -> Vec<ToolDescriptor> {
        self.tools.clone()
    }

    pub fn list_chats(&self) -> Vec<ChatSession> {
        self.store.state().chats.clone()
    }

    pub fn get_chat(&self, chat_id: &str) -> Option<ChatSession> {
        self.store
            .state()
            .chats
            .iter()
            .find(|chat| chat.id == chat_id)
            .cloned()
    }

    pub fn list_chat_messages(&self, chat_id: &str) -> Vec<ChatMessage> {
        self.store
            .state()
            .chat_messages
            .iter()
            .filter(|message| message.chat_id == chat_id)
            .cloned()
            .collect()
    }

    pub fn list_chat_message_page(
        &self,
        chat_id: &str,
        before: Option<&str>,
        limit: usize,
    ) -> (Vec<ChatMessage>, bool) {
        let messages = self.list_chat_messages(chat_id);
        let end = before
            .and_then(|message_id| messages.iter().position(|message| message.id == message_id))
            .unwrap_or(messages.len());
        let start = end.saturating_sub(limit);
        (messages[start..end].to_vec(), start > 0)
    }

    pub fn list_tasks(&self) -> Vec<TaskRecord> {
        self.store.state().tasks.clone()
    }

    pub fn get_project_content_profile(&self, project: &str) -> Option<ProjectContentProfile> {
        self.store
            .state()
            .project_content_profiles
            .iter()
            .find(|profile| profile.project == project)
            .cloned()
    }

    pub fn update_project_content_profile(
        &mut self,
        request: ProjectContentProfileUpdate,
    ) -> Result<ProjectContentProfile, OrchestratorError> {
        if !self
            .store
            .state()
            .projects
            .iter()
            .any(|project| project.name == request.project)
        {
            return Err(OrchestratorError::UnknownProject);
        }

        let profile = ProjectContentProfile {
            project: request.project,
            audience: request
                .audience
                .and_then(|value| normalize_optional_string(&value)),
            language: request
                .language
                .and_then(|value| normalize_optional_string(&value)),
            tone: request
                .tone
                .and_then(|value| normalize_optional_string(&value)),
            persona: request
                .persona
                .and_then(|value| normalize_optional_string(&value)),
            content_pillars: normalize_string_vec(request.content_pillars),
            references: normalize_string_vec(request.references),
            constraints: normalize_string_vec(request.constraints),
            publish_channels: normalize_string_vec(request.publish_channels),
            updated_at: Utc::now(),
        };

        self.store
            .state_mut()
            .project_content_profiles
            .retain(|existing| existing.project != profile.project);
        self.store
            .state_mut()
            .project_content_profiles
            .push(profile.clone());
        self.store.save()?;
        Ok(profile)
    }

    pub fn list_factory_pipelines(&self) -> Vec<FactoryPipeline> {
        self.store.state().factory_pipelines.clone()
    }

    pub fn get_factory_pipeline(&self, pipeline_id: &str) -> Option<FactoryPipeline> {
        self.store
            .state()
            .factory_pipelines
            .iter()
            .find(|pipeline| pipeline.id == pipeline_id)
            .cloned()
    }

    pub fn create_factory_pipeline(
        &mut self,
        request: CreateFactoryPipelineRequest,
    ) -> Result<FactoryPipeline, OrchestratorError> {
        if request.brief.trim().is_empty() {
            return Err(OrchestratorError::InvalidInput(
                "factory brief is blank".to_string(),
            ));
        }
        if request.content_types.is_empty() {
            return Err(OrchestratorError::InvalidInput(
                "factory content types are empty".to_string(),
            ));
        }

        let project = self
            .store
            .state()
            .projects
            .iter()
            .find(|project| project.name == request.project)
            .cloned()
            .ok_or(OrchestratorError::UnknownProject)?;
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let title = request
            .title
            .and_then(|title| normalize_optional_string(&title))
            .unwrap_or_else(|| format!("Factory / {}", project.name));
        let output_path = match request.output_path {
            Some(path) if path.is_absolute() => Some(path),
            Some(path) => Some(project.root.join(path)),
            None => Some(project.root.join(".jin/factories").join(&id)),
        };
        let sync_targets = match request.sync_targets {
            Some(targets) => normalize_sync_targets(targets),
            None => self.store.state().settings.default_sync_targets.clone(),
        };
        let created_event = FactoryEvent {
            id: Uuid::new_v4().to_string(),
            pipeline_id: id.clone(),
            kind: FactoryEventKind::System,
            content: "factory pipeline created".to_string(),
            created_at: now,
        };
        let pipeline = FactoryPipeline {
            id,
            project: project.name,
            title,
            brief: request.brief.trim().to_string(),
            mode: request.mode,
            review_policy: request.review_policy,
            status: FactoryPipelineStatus::Draft,
            content_types: request.content_types,
            output_path,
            schedule: Default::default(),
            sync_targets,
            stages: default_factory_stages(),
            artifacts: Vec::new(),
            review_bundles: Vec::new(),
            events: vec![created_event],
            created_at: now,
            updated_at: now,
        };

        self.store
            .state_mut()
            .factory_pipelines
            .push(pipeline.clone());
        self.store.save()?;
        Ok(pipeline)
    }

    pub fn list_factory_events(&self, pipeline_id: &str) -> Vec<FactoryEvent> {
        self.get_factory_pipeline(pipeline_id)
            .map(|pipeline| pipeline.events)
            .unwrap_or_default()
    }

    pub fn pause_factory_pipeline(
        &mut self,
        pipeline_id: &str,
    ) -> Result<FactoryPipeline, OrchestratorError> {
        self.set_factory_pipeline_status(
            pipeline_id,
            FactoryPipelineStatus::Paused,
            "factory pipeline paused",
        )
    }

    pub fn resume_factory_pipeline(
        &mut self,
        pipeline_id: &str,
    ) -> Result<FactoryPipeline, OrchestratorError> {
        self.set_factory_pipeline_status(
            pipeline_id,
            FactoryPipelineStatus::Scheduled,
            "factory pipeline resumed",
        )
    }

    pub fn stop_factory_pipeline(
        &mut self,
        pipeline_id: &str,
    ) -> Result<FactoryPipeline, OrchestratorError> {
        self.set_factory_pipeline_status(
            pipeline_id,
            FactoryPipelineStatus::Stopped,
            "factory pipeline stopped",
        )
    }

    fn set_factory_pipeline_status(
        &mut self,
        pipeline_id: &str,
        status: FactoryPipelineStatus,
        event: &str,
    ) -> Result<FactoryPipeline, OrchestratorError> {
        let now = Utc::now();
        let pipeline = self
            .store
            .state_mut()
            .factory_pipelines
            .iter_mut()
            .find(|pipeline| pipeline.id == pipeline_id)
            .ok_or(OrchestratorError::UnknownFactory)?;
        pipeline.status = status;
        pipeline.updated_at = now;
        pipeline.events.push(FactoryEvent {
            id: Uuid::new_v4().to_string(),
            pipeline_id: pipeline.id.clone(),
            kind: FactoryEventKind::System,
            content: event.to_string(),
            created_at: now,
        });
        let pipeline = pipeline.clone();
        self.store.save()?;
        Ok(pipeline)
    }

    pub fn list_approvals(&self) -> Vec<ApprovalRecord> {
        self.store.state().approvals.clone()
    }

    pub fn get_task(&self, task_id: &str) -> Option<TaskRecord> {
        self.store
            .state()
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
    }

    pub fn create_chat(
        &mut self,
        request: CreateChatRequest,
    ) -> Result<ChatSession, OrchestratorError> {
        if !self
            .store
            .state()
            .projects
            .iter()
            .any(|project| project.name == request.project)
        {
            return Err(OrchestratorError::UnknownProject);
        }
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.id == request.tool)
            .cloned()
            .ok_or(OrchestratorError::UnknownTool)?;
        let settings = normalize_chat_settings(&tool, request.settings)?;
        let now = Utc::now();
        let title = request
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| format!("{} / {}", tool.name, request.project));
        let chat = ChatSession {
            id: Uuid::new_v4().to_string(),
            title,
            project: request.project,
            tool: tool.id,
            status: ChatStatus::Idle,
            settings,
            sync_targets: request
                .sync_targets
                .map(normalize_sync_targets)
                .unwrap_or_else(|| self.store.state().settings.default_sync_targets.clone()),
            context: ContextSummary {
                supported: tool.supports_context_meter,
                used: None,
                limit: None,
                label: if tool.supports_context_meter {
                    "Context available when the tool reports usage".to_string()
                } else {
                    "Context meter is not supported by this tool".to_string()
                },
            },
            created_at: now,
            updated_at: now,
        };

        self.store.state_mut().chats.push(chat.clone());
        self.store.save()?;
        Ok(chat)
    }

    pub fn append_chat_message(
        &mut self,
        request: PostChatMessageRequest,
    ) -> Result<ChatMessage, OrchestratorError> {
        if request.content.trim().is_empty() {
            return Err(OrchestratorError::InvalidInput(
                "chat message is blank".to_string(),
            ));
        }
        let now = Utc::now();
        let chat = self
            .store
            .state_mut()
            .chats
            .iter_mut()
            .find(|chat| chat.id == request.chat_id)
            .ok_or(OrchestratorError::UnknownChat)?;
        chat.updated_at = now;

        let message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            chat_id: request.chat_id,
            role: request.role,
            content: request.content.trim().to_string(),
            created_at: now,
        };
        self.store.state_mut().chat_messages.push(message.clone());
        self.store.save()?;
        Ok(message)
    }

    pub fn upsert_last_chat_message(
        &mut self,
        request: PostChatMessageRequest,
    ) -> Result<ChatMessage, OrchestratorError> {
        if request.content.trim().is_empty() {
            return Err(OrchestratorError::InvalidInput(
                "chat message is blank".to_string(),
            ));
        }
        let now = Utc::now();
        let chat = self
            .store
            .state_mut()
            .chats
            .iter_mut()
            .find(|chat| chat.id == request.chat_id)
            .ok_or(OrchestratorError::UnknownChat)?;
        chat.updated_at = now;

        if let Some(message) = self
            .store
            .state_mut()
            .chat_messages
            .iter_mut()
            .rev()
            .find(|message| message.chat_id == request.chat_id)
        {
            if message.role == request.role {
                message.content = request.content.trim().to_string();
                let message = message.clone();
                self.store.save()?;
                return Ok(message);
            }
        }

        let message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            chat_id: request.chat_id,
            role: request.role,
            content: request.content.trim().to_string(),
            created_at: now,
        };
        self.store.state_mut().chat_messages.push(message.clone());
        self.store.save()?;
        Ok(message)
    }

    pub fn update_chat_settings(
        &mut self,
        request: UpdateChatSettingsRequest,
    ) -> Result<ChatSession, OrchestratorError> {
        let existing = self
            .store
            .state()
            .chats
            .iter()
            .find(|chat| chat.id == request.chat_id)
            .cloned()
            .ok_or(OrchestratorError::UnknownChat)?;
        let tool = self
            .tools
            .iter()
            .find(|tool| tool.id == existing.tool)
            .cloned()
            .ok_or(OrchestratorError::UnknownTool)?;
        let mut settings = existing.settings.clone();
        settings.extend(request.settings);
        let settings = normalize_chat_settings(&tool, settings)?;
        let chat = self
            .store
            .state_mut()
            .chats
            .iter_mut()
            .find(|chat| chat.id == request.chat_id)
            .ok_or(OrchestratorError::UnknownChat)?;
        chat.settings = settings;
        chat.updated_at = Utc::now();
        let chat = chat.clone();
        self.store.save()?;
        Ok(chat)
    }

    pub fn set_chat_status(
        &mut self,
        chat_id: &str,
        status: ChatStatus,
    ) -> Result<ChatSession, OrchestratorError> {
        let chat = self
            .store
            .state_mut()
            .chats
            .iter_mut()
            .find(|chat| chat.id == chat_id)
            .ok_or(OrchestratorError::UnknownChat)?;
        chat.status = status;
        chat.updated_at = Utc::now();
        let chat = chat.clone();
        self.store.save()?;
        Ok(chat)
    }

    pub fn create_task(
        &mut self,
        request: CreateTaskRequest,
    ) -> Result<TaskRecord, OrchestratorError> {
        let project = self
            .store
            .state()
            .projects
            .iter()
            .find(|project| project.name == request.project)
            .cloned()
            .ok_or(OrchestratorError::UnknownProject)?;

        let now = Utc::now();
        let mut task = TaskRecord {
            id: Uuid::new_v4().to_string(),
            project: request.project,
            runner: request.runner,
            command: request.command,
            state: TaskState::Queued,
            pending_approval_id: None,
            output: String::new(),
            exit_code: None,
            created_at: now,
            updated_at: now,
        };

        if task.runner == "shell" {
            match self
                .policy
                .evaluate(&GuardedAction::ShellCommand(task.command.clone()))
            {
                PolicyDecision::Allow => {
                    task = run_shell_task(task, project.root)?;
                }
                PolicyDecision::RequireApproval { reason } => {
                    let approval_id = Uuid::new_v4().to_string();
                    task.state = TaskState::WaitingApproval;
                    task.pending_approval_id = Some(approval_id.clone());
                    task.updated_at = Utc::now();
                    self.store.state_mut().approvals.push(ApprovalRecord {
                        id: approval_id,
                        task_id: task.id.clone(),
                        operation: format!("shell: {}", task.command),
                        reason,
                        decided_by: None,
                        decision: None,
                        created_at: Utc::now(),
                        decided_at: None,
                    });
                }
            }
        } else if task.runner == "codex" {
            task.state = TaskState::WaitingApproval;
            let approval_id = Uuid::new_v4().to_string();
            task.pending_approval_id = Some(approval_id.clone());
            self.store.state_mut().approvals.push(ApprovalRecord {
                id: approval_id,
                task_id: task.id.clone(),
                operation: format!("codex: {}", task.command),
                reason: "codex runner requires approval in the local MVP".to_string(),
                decided_by: None,
                decision: None,
                created_at: Utc::now(),
                decided_at: None,
            });
        } else {
            return Err(OrchestratorError::UnknownRunner);
        }

        self.store.state_mut().tasks.push(task.clone());
        self.store.save()?;
        Ok(task)
    }

    pub fn approve(
        &mut self,
        approval_id: &str,
        actor: impl Into<String>,
    ) -> Result<TaskRecord, OrchestratorError> {
        let actor = actor.into();
        let approval = self
            .store
            .state_mut()
            .approvals
            .iter_mut()
            .find(|approval| approval.id == approval_id)
            .ok_or(OrchestratorError::UnknownApproval)?;

        if approval.decision.is_some() {
            return Err(OrchestratorError::ApprovalAlreadyDecided);
        }

        approval.decision = Some("approved".to_string());
        approval.decided_by = Some(actor);
        approval.decided_at = Some(Utc::now());
        let task_id = approval.task_id.clone();

        let task = self
            .store
            .state()
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
            .ok_or(OrchestratorError::UnknownTask)?;
        let project = self
            .store
            .state()
            .projects
            .iter()
            .find(|project| project.name == task.project)
            .cloned()
            .ok_or(OrchestratorError::UnknownProject)?;

        let updated = if task.runner == "shell" {
            run_shell_task(task, project.root)?
        } else {
            run_codex_task(task, project.root)?
        };

        replace_task(self.store.state_mut(), updated.clone());
        self.store.save()?;
        Ok(updated)
    }

    pub fn reject(
        &mut self,
        approval_id: &str,
        actor: impl Into<String>,
    ) -> Result<TaskRecord, OrchestratorError> {
        let actor = actor.into();
        let approval = self
            .store
            .state_mut()
            .approvals
            .iter_mut()
            .find(|approval| approval.id == approval_id)
            .ok_or(OrchestratorError::UnknownApproval)?;
        approval.decision = Some("rejected".to_string());
        approval.decided_by = Some(actor);
        approval.decided_at = Some(Utc::now());

        let task_id = approval.task_id.clone();
        let mut task = self
            .store
            .state()
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
            .ok_or(OrchestratorError::UnknownTask)?;
        task.state = TaskState::Cancelled;
        task.pending_approval_id = None;
        task.updated_at = Utc::now();
        replace_task(self.store.state_mut(), task.clone());
        self.store.save()?;
        Ok(task)
    }
}

fn replace_task(state: &mut crate::store::JinState, task: TaskRecord) {
    if let Some(existing) = state
        .tasks
        .iter_mut()
        .find(|existing| existing.id == task.id)
    {
        *existing = task;
    } else {
        state.tasks.push(task);
    }
}

fn redact_settings(settings: &JinSettings) -> JinSettings {
    JinSettings {
        public_host: settings.public_host.clone(),
        telegram: redacted_telegram_settings(&settings.telegram),
        default_sync_targets: settings.default_sync_targets.clone(),
    }
}

fn normalize_chat_settings(
    tool: &ToolDescriptor,
    requested: BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, OrchestratorError> {
    let mut settings = BTreeMap::new();
    for descriptor in &tool.settings {
        if let Some(default) = &descriptor.default {
            settings.insert(descriptor.id.clone(), default.clone());
        }
    }

    for (key, value) in requested {
        let descriptor = tool
            .settings
            .iter()
            .find(|setting| setting.id == key)
            .ok_or_else(|| {
                OrchestratorError::InvalidInput(format!(
                    "tool {} does not support setting {key}",
                    tool.id
                ))
            })?;
        if !descriptor.options.is_empty() && !descriptor.options.contains(&value) {
            return Err(OrchestratorError::InvalidInput(format!(
                "setting {key} does not support value {value}"
            )));
        }
        settings.insert(key, value);
    }

    Ok(settings)
}

fn normalize_optional_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn normalize_string_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| normalize_optional_string(&value))
        .collect()
}

fn canonicalize_project_root(root: &Path) -> Result<PathBuf, OrchestratorError> {
    match root.try_exists() {
        Ok(true) => {}
        Ok(false) => {
            return Err(OrchestratorError::InvalidInput(format!(
                "project root does not exist: {}",
                root.display()
            )));
        }
        Err(error) => {
            return Err(OrchestratorError::InvalidInput(format!(
                "project root is not accessible: {} ({error})",
                root.display()
            )));
        }
    }

    if !root.is_dir() {
        return Err(OrchestratorError::InvalidInput(format!(
            "project root is not a directory: {}",
            root.display()
        )));
    }

    root.canonicalize().map_err(|error| {
        OrchestratorError::InvalidInput(format!(
            "project root is not accessible: {} ({error})",
            root.display()
        ))
    })
}

fn run_shell_task(mut task: TaskRecord, root: PathBuf) -> Result<TaskRecord, OrchestratorError> {
    task.state = TaskState::Running;
    task.updated_at = Utc::now();

    let mut runner = ShellRunner::new();
    let result = runner.run(RunnerRequest {
        task_id: task.id.clone(),
        working_directory: root,
        prompt: task.command.clone(),
    })?;

    task.output = result
        .events
        .iter()
        .map(|event| event.message.as_str())
        .collect::<Vec<_>>()
        .join("");
    task.exit_code = Some(result.exit_code);
    task.state = if result.exit_code == 0 {
        TaskState::Completed
    } else {
        TaskState::Failed
    };
    task.pending_approval_id = None;
    task.updated_at = Utc::now();
    Ok(task)
}

fn run_codex_task(mut task: TaskRecord, root: PathBuf) -> Result<TaskRecord, OrchestratorError> {
    task.state = TaskState::Running;
    task.updated_at = Utc::now();

    let mut runner = CodexRunner::new();
    let result = runner.run(RunnerRequest {
        task_id: task.id.clone(),
        working_directory: root,
        prompt: task.command.clone(),
    })?;

    task.output = result
        .events
        .iter()
        .map(|event| event.message.as_str())
        .collect::<Vec<_>>()
        .join("");
    task.exit_code = Some(result.exit_code);
    task.state = if result.exit_code == 0 {
        TaskState::Completed
    } else {
        TaskState::Failed
    };
    task.pending_approval_id = None;
    task.updated_at = Utc::now();
    Ok(task)
}

#[derive(Debug)]
pub enum OrchestratorError {
    InvalidInput(String),
    UnknownProject,
    UnknownTool,
    UnknownChat,
    UnknownFactory,
    UnknownRunner,
    UnknownTask,
    UnknownApproval,
    ApprovalAlreadyDecided,
    Io(std::io::Error),
    Runner(crate::runner::RunnerError),
    Store(StoreError),
}

impl From<crate::runner::RunnerError> for OrchestratorError {
    fn from(error: crate::runner::RunnerError) -> Self {
        Self::Runner(error)
    }
}

impl From<StoreError> for OrchestratorError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl std::fmt::Display for OrchestratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "{message}"),
            Self::UnknownProject => write!(f, "unknown project"),
            Self::UnknownTool => write!(f, "unknown tool"),
            Self::UnknownChat => write!(f, "unknown chat"),
            Self::UnknownFactory => write!(f, "unknown factory"),
            Self::UnknownRunner => write!(f, "unknown runner"),
            Self::UnknownTask => write!(f, "unknown task"),
            Self::UnknownApproval => write!(f, "unknown approval"),
            Self::ApprovalAlreadyDecided => write!(f, "approval already decided"),
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Runner(error) => write!(f, "runner error: {error}"),
            Self::Store(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for OrchestratorError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{ChatRole, ChatStatus};
    use crate::store::FileStore;
    use crate::task::TaskState;
    use std::collections::BTreeMap;

    #[test]
    fn register_project_rejects_missing_root_as_invalid_input() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let missing_root = temp.path().join("missing-project");

        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        let error = orchestrator
            .register_project("jin", missing_root)
            .expect_err("missing project root should be rejected");

        match error {
            OrchestratorError::InvalidInput(message) => {
                assert!(message.contains("project root does not exist"), "{message}");
            }
            other => panic!("expected invalid input, got {other:?}"),
        }
    }

    #[test]
    fn create_chat_validates_project_and_tool_and_persists_defaults() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");

        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        orchestrator
            .register_project("jin", project_root)
            .expect("project registers");

        let chat = orchestrator
            .create_chat(CreateChatRequest {
                project: "jin".to_string(),
                tool: "codex".to_string(),
                title: None,
                settings: BTreeMap::new(),
                sync_targets: None,
            })
            .expect("chat is created");

        assert_eq!(chat.project, "jin");
        assert_eq!(chat.tool, "codex");
        assert_eq!(chat.status, ChatStatus::Idle);
        assert_eq!(
            chat.settings.get("reasoning").map(String::as_str),
            Some("medium")
        );
        assert!(chat.context.supported);
        assert_eq!(orchestrator.list_chats().len(), 1);

        let reloaded = JinOrchestrator::new(FileStore::open(&state_path).expect("store reloads"));
        assert_eq!(reloaded.list_chats().len(), 1);
    }

    #[test]
    fn create_chat_accepts_model_options_from_configured_tool_descriptor() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let mut tools = built_in_tools();
        let codex = tools
            .iter_mut()
            .find(|tool| tool.id == "codex")
            .expect("codex tool exists");
        let model = codex
            .settings
            .iter_mut()
            .find(|setting| setting.id == "model")
            .expect("model setting exists");
        model.options = vec!["gpt-5.5".to_string()];
        model.default = Some("gpt-5.5".to_string());

        let mut orchestrator = JinOrchestrator::new_with_tools(
            FileStore::open(&state_path).expect("store opens"),
            tools,
        );
        orchestrator
            .register_project("jin", project_root)
            .expect("project registers");

        let chat = orchestrator
            .create_chat(CreateChatRequest {
                project: "jin".to_string(),
                tool: "codex".to_string(),
                title: None,
                settings: BTreeMap::from([("model".to_string(), "gpt-5.5".to_string())]),
                sync_targets: None,
            })
            .expect("dynamic model option is accepted");

        assert_eq!(
            chat.settings.get("model").map(String::as_str),
            Some("gpt-5.5")
        );
    }

    #[test]
    fn create_chat_rejects_unknown_project_or_tool() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");

        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        orchestrator
            .register_project("jin", project_root)
            .expect("project registers");

        let unknown_project = orchestrator
            .create_chat(CreateChatRequest {
                project: "missing".to_string(),
                tool: "codex".to_string(),
                title: None,
                settings: BTreeMap::new(),
                sync_targets: None,
            })
            .expect_err("unknown project is rejected");
        assert!(matches!(unknown_project, OrchestratorError::UnknownProject));

        let unknown_tool = orchestrator
            .create_chat(CreateChatRequest {
                project: "jin".to_string(),
                tool: "missing".to_string(),
                title: None,
                settings: BTreeMap::new(),
                sync_targets: None,
            })
            .expect_err("unknown tool is rejected");
        assert!(matches!(unknown_tool, OrchestratorError::UnknownTool));
    }

    #[test]
    fn append_chat_message_rejects_blank_and_persists_timeline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");

        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        orchestrator
            .register_project("jin", project_root)
            .expect("project registers");
        let chat = orchestrator
            .create_chat(CreateChatRequest {
                project: "jin".to_string(),
                tool: "codex".to_string(),
                title: None,
                settings: BTreeMap::new(),
                sync_targets: None,
            })
            .expect("chat is created");

        let blank = orchestrator
            .append_chat_message(PostChatMessageRequest {
                chat_id: chat.id.clone(),
                role: ChatRole::User,
                content: "   ".to_string(),
            })
            .expect_err("blank message is rejected");
        assert!(matches!(blank, OrchestratorError::InvalidInput(_)));

        let message = orchestrator
            .append_chat_message(PostChatMessageRequest {
                chat_id: chat.id.clone(),
                role: ChatRole::User,
                content: "implement this".to_string(),
            })
            .expect("message appends");

        assert_eq!(message.role, ChatRole::User);
        assert_eq!(message.content, "implement this");
        assert_eq!(orchestrator.list_chat_messages(&chat.id).len(), 1);
    }

    #[test]
    fn update_chat_settings_validates_and_persists_values() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        orchestrator
            .register_project("jin", project_root)
            .expect("project registers");
        let chat = orchestrator
            .create_chat(CreateChatRequest {
                project: "jin".to_string(),
                tool: "codex".to_string(),
                title: None,
                settings: BTreeMap::new(),
                sync_targets: None,
            })
            .expect("chat is created");

        let updated = orchestrator
            .update_chat_settings(UpdateChatSettingsRequest {
                chat_id: chat.id.clone(),
                settings: BTreeMap::from([
                    ("model".to_string(), "gpt-5.4".to_string()),
                    ("reasoning".to_string(), "high".to_string()),
                ]),
            })
            .expect("settings update");

        assert_eq!(
            updated.settings.get("model").map(String::as_str),
            Some("gpt-5.4")
        );
        assert_eq!(
            updated.settings.get("reasoning").map(String::as_str),
            Some("high")
        );

        let updated = orchestrator
            .update_chat_settings(UpdateChatSettingsRequest {
                chat_id: chat.id.clone(),
                settings: BTreeMap::from([("model".to_string(), "gpt-5.2".to_string())]),
            })
            .expect("partial settings update");

        assert_eq!(
            updated.settings.get("model").map(String::as_str),
            Some("gpt-5.2")
        );
        assert_eq!(
            updated.settings.get("reasoning").map(String::as_str),
            Some("high")
        );

        let rejected = orchestrator
            .update_chat_settings(UpdateChatSettingsRequest {
                chat_id: chat.id,
                settings: BTreeMap::from([("model".to_string(), "not-a-model".to_string())]),
            })
            .expect_err("unsupported model is rejected");
        assert!(matches!(rejected, OrchestratorError::InvalidInput(_)));
    }

    #[test]
    fn approving_shell_task_runs_pending_command_and_records_output() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        std::process::Command::new("git")
            .arg("init")
            .current_dir(&project_root)
            .output()
            .expect("git init");

        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        orchestrator
            .register_project("jin", project_root)
            .expect("project registers");

        let task = orchestrator
            .create_task(CreateTaskRequest {
                project: "jin".to_string(),
                runner: "shell".to_string(),
                command: "printf approved".to_string(),
            })
            .expect("task is created");

        assert_eq!(task.state, TaskState::WaitingApproval);
        let approval_id = task.pending_approval_id.expect("approval id");

        let completed = orchestrator
            .approve(&approval_id, "nikita")
            .expect("approval executes task");

        assert_eq!(completed.state, TaskState::Completed);
        assert_eq!(completed.exit_code, Some(0));
        assert!(completed.output.contains("approved"));
    }

    #[test]
    fn allowlisted_shell_task_runs_without_approval() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");
        std::process::Command::new("git")
            .arg("init")
            .current_dir(&project_root)
            .output()
            .expect("git init");

        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        orchestrator
            .register_project("jin", project_root)
            .expect("project registers");

        let task = orchestrator
            .create_task(CreateTaskRequest {
                project: "jin".to_string(),
                runner: "shell".to_string(),
                command: "git status".to_string(),
            })
            .expect("task is created");

        assert_eq!(task.state, TaskState::Completed);
        assert!(task.pending_approval_id.is_none());
    }

    #[test]
    fn project_content_profile_persists_and_factory_inherits_sync_defaults() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");

        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        orchestrator
            .register_project("jin", project_root)
            .expect("project registers");
        orchestrator
            .update_settings(crate::store::JinSettings {
                public_host: Some("jin.example.com".to_string()),
                telegram: crate::sync::TelegramSettings {
                    bot_token: Some("secret-token".to_string()),
                    bot_token_configured: false,
                    default_group_chat_id: Some("-10010".to_string()),
                },
                default_sync_targets: vec![crate::sync::SyncTarget {
                    id: "tg-project".to_string(),
                    label: "Jin project topic".to_string(),
                    kind: crate::sync::SyncTargetKind::TelegramForumTopic,
                    chat_id: Some("-10010".to_string()),
                    message_thread_id: Some(42),
                }],
            })
            .expect("settings update");

        let public_settings = orchestrator.settings();
        assert!(public_settings.telegram.bot_token.is_none());
        assert!(public_settings.telegram.bot_token_configured);
        assert_eq!(public_settings.default_sync_targets.len(), 1);

        let profile = orchestrator
            .update_project_content_profile(crate::factory::ProjectContentProfileUpdate {
                project: "jin".to_string(),
                audience: Some("founders".to_string()),
                language: Some("ru".to_string()),
                tone: Some("pragmatic".to_string()),
                persona: Some("technical founder".to_string()),
                content_pillars: vec!["ai agents".to_string(), "dev tools".to_string()],
                references: vec!["https://example.com/ref".to_string()],
                constraints: vec!["no hype".to_string()],
                publish_channels: vec!["telegram".to_string()],
            })
            .expect("profile update");

        assert_eq!(profile.project, "jin");
        assert_eq!(profile.audience.as_deref(), Some("founders"));

        let pipeline = orchestrator
            .create_factory_pipeline(crate::factory::CreateFactoryPipelineRequest {
                project: "jin".to_string(),
                title: Some("Weekly agent content".to_string()),
                brief: "Prepare article drafts and icon concepts".to_string(),
                mode: crate::factory::FactoryPipelineMode::Finite,
                review_policy: crate::factory::FactoryReviewPolicy::PerStage,
                content_types: vec![
                    crate::factory::FactoryArtifactKind::Text,
                    crate::factory::FactoryArtifactKind::Image,
                ],
                output_path: None,
                sync_targets: None,
            })
            .expect("factory creates");

        assert_eq!(pipeline.project, "jin");
        assert_eq!(
            pipeline.status,
            crate::factory::FactoryPipelineStatus::Draft
        );
        assert_eq!(pipeline.sync_targets.len(), 1);
        assert_eq!(pipeline.stages.len(), 6);
        assert_eq!(
            pipeline
                .events
                .first()
                .expect("created event")
                .content
                .as_str(),
            "factory pipeline created"
        );

        let reloaded = JinOrchestrator::new(FileStore::open(&state_path).expect("store reloads"));
        assert_eq!(
            reloaded
                .get_project_content_profile("jin")
                .expect("profile persists")
                .tone
                .as_deref(),
            Some("pragmatic")
        );
        assert_eq!(reloaded.list_factory_pipelines().len(), 1);
        assert!(reloaded.settings().telegram.bot_token.is_none());
        assert!(reloaded.settings().telegram.bot_token_configured);
    }

    #[test]
    fn factory_lifecycle_controls_update_status_and_timeline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");

        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        orchestrator
            .register_project("jin", project_root)
            .expect("project registers");

        let pipeline = orchestrator
            .create_factory_pipeline(crate::factory::CreateFactoryPipelineRequest {
                project: "jin".to_string(),
                title: None,
                brief: "Create a content pipeline".to_string(),
                mode: crate::factory::FactoryPipelineMode::Continuous,
                review_policy: crate::factory::FactoryReviewPolicy::FinalOnly,
                content_types: vec![crate::factory::FactoryArtifactKind::Script],
                output_path: Some("content-output".into()),
                sync_targets: Some(Vec::new()),
            })
            .expect("factory creates");

        let paused = orchestrator
            .pause_factory_pipeline(&pipeline.id)
            .expect("pipeline pauses");
        assert_eq!(paused.status, crate::factory::FactoryPipelineStatus::Paused);

        let resumed = orchestrator
            .resume_factory_pipeline(&pipeline.id)
            .expect("pipeline resumes");
        assert_eq!(
            resumed.status,
            crate::factory::FactoryPipelineStatus::Scheduled
        );

        let stopped = orchestrator
            .stop_factory_pipeline(&pipeline.id)
            .expect("pipeline stops");
        assert_eq!(
            stopped.status,
            crate::factory::FactoryPipelineStatus::Stopped
        );

        let events = orchestrator.list_factory_events(&pipeline.id);
        assert!(events
            .iter()
            .any(|event| event.content == "factory pipeline paused"));
        assert!(events
            .iter()
            .any(|event| event.content == "factory pipeline resumed"));
        assert!(events
            .iter()
            .any(|event| event.content == "factory pipeline stopped"));
    }
}
