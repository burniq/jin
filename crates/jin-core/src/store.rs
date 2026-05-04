use crate::chat::{ChatMessage, ChatSession};
use crate::orchestrator::{ApprovalRecord, ProjectRecord, TaskRecord};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JinSettings {
    #[serde(default)]
    pub public_host: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JinState {
    #[serde(default)]
    pub settings: JinSettings,
    #[serde(default)]
    pub projects: Vec<ProjectRecord>,
    #[serde(default)]
    pub tasks: Vec<TaskRecord>,
    #[serde(default)]
    pub approvals: Vec<ApprovalRecord>,
    #[serde(default)]
    pub chats: Vec<ChatSession>,
    #[serde(default)]
    pub chat_messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone)]
pub struct FileStore {
    path: PathBuf,
    state: JinState,
}

impl FileStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        let state = if path.exists() {
            let raw = fs::read_to_string(&path).map_err(StoreError::Io)?;
            serde_json::from_str(&raw).map_err(StoreError::Json)?
        } else {
            JinState::default()
        };

        Ok(Self { path, state })
    }

    pub fn state(&self) -> &JinState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut JinState {
        &mut self.state
    }

    pub fn save(&self) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(StoreError::Io)?;
        }

        let raw = serde_json::to_string_pretty(&self.state).map_err(StoreError::Json)?;
        fs::write(&self.path, raw).map_err(StoreError::Io)
    }
}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "store io error: {error}"),
            Self::Json(error) => write!(f, "store json error: {error}"),
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::{CreateTaskRequest, JinOrchestrator};
    use crate::task::TaskState;

    #[test]
    fn file_store_persists_projects_tasks_and_approvals() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state_path = temp.path().join("state.json");
        let project_root = temp.path().join("project");
        std::fs::create_dir_all(&project_root).expect("project root");

        let mut orchestrator =
            JinOrchestrator::new(FileStore::open(&state_path).expect("store opens"));
        orchestrator
            .register_project("jin", project_root.clone())
            .expect("project registers");

        let task = orchestrator
            .create_task(CreateTaskRequest {
                project: "jin".to_string(),
                runner: "shell".to_string(),
                command: "cargo test --workspace".to_string(),
            })
            .expect("task is created");

        assert_eq!(task.state, TaskState::WaitingApproval);
        assert!(task.pending_approval_id.is_some());

        let reloaded = JinOrchestrator::new(FileStore::open(&state_path).expect("store reloads"));
        assert_eq!(reloaded.list_projects().len(), 1);
        assert_eq!(
            reloaded.get_task(&task.id).expect("task exists").state,
            TaskState::WaitingApproval
        );
    }
}
