use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub id: String,
    pub name: String,
    pub supports_persistent_session: bool,
    pub supports_context_meter: bool,
    pub settings: Vec<ToolSettingDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSettingDescriptor {
    pub id: String,
    pub label: String,
    pub kind: ToolSettingKind,
    pub options: Vec<String>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolSettingKind {
    Select,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatStatus {
    Idle,
    Running,
    WaitingApproval,
    WaitingUser,
    Stopped,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatRole {
    User,
    Assistant,
    Tool,
    System,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSummary {
    pub supported: bool,
    pub used: Option<u32>,
    pub limit: Option<u32>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub project: String,
    pub tool: String,
    pub status: ChatStatus,
    pub settings: BTreeMap<String, String>,
    pub context: ContextSummary,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub chat_id: String,
    pub role: ChatRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

pub fn built_in_tools() -> Vec<ToolDescriptor> {
    vec![codex_descriptor(), shell_descriptor()]
}

pub fn tool_descriptor(tool_id: &str) -> Option<ToolDescriptor> {
    built_in_tools().into_iter().find(|tool| tool.id == tool_id)
}

fn codex_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        id: "codex".to_string(),
        name: "Codex".to_string(),
        supports_persistent_session: true,
        supports_context_meter: true,
        settings: vec![
            ToolSettingDescriptor {
                id: "model".to_string(),
                label: "Model".to_string(),
                kind: ToolSettingKind::Select,
                options: vec![
                    "gpt-5.4".to_string(),
                    "gpt-5.3-codex".to_string(),
                    "gpt-5.2-codex".to_string(),
                    "gpt-5.1-codex-max".to_string(),
                    "gpt-5.2".to_string(),
                    "gpt-5.1-codex-mini".to_string(),
                ],
                default: Some("gpt-5.4".to_string()),
            },
            ToolSettingDescriptor {
                id: "reasoning".to_string(),
                label: "Reasoning".to_string(),
                kind: ToolSettingKind::Select,
                options: vec![
                    "low".to_string(),
                    "medium".to_string(),
                    "high".to_string(),
                    "xhigh".to_string(),
                ],
                default: Some("medium".to_string()),
            },
            ToolSettingDescriptor {
                id: "approval_mode".to_string(),
                label: "Approval Mode".to_string(),
                kind: ToolSettingKind::Select,
                options: vec![
                    "suggest".to_string(),
                    "auto-edit".to_string(),
                    "read-only".to_string(),
                ],
                default: Some("suggest".to_string()),
            },
        ],
    }
}

fn shell_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        id: "shell".to_string(),
        name: "Shell".to_string(),
        supports_persistent_session: false,
        supports_context_meter: false,
        settings: vec![ToolSettingDescriptor {
            id: "approval_mode".to_string(),
            label: "Approval Mode".to_string(),
            kind: ToolSettingKind::Select,
            options: vec!["guarded".to_string()],
            default: Some("guarded".to_string()),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_descriptor_exposes_reasoning_levels() {
        let codex = built_in_tools()
            .into_iter()
            .find(|tool| tool.id == "codex")
            .expect("codex descriptor exists");

        assert!(codex.supports_persistent_session);
        assert!(codex.supports_context_meter);
        let reasoning = codex
            .settings
            .iter()
            .find(|setting| setting.id == "reasoning")
            .expect("reasoning setting exists");
        assert_eq!(reasoning.options, vec!["low", "medium", "high", "xhigh"]);
    }

    #[test]
    fn codex_descriptor_exposes_model_options() {
        let codex = built_in_tools()
            .into_iter()
            .find(|tool| tool.id == "codex")
            .expect("codex descriptor exists");

        let model = codex
            .settings
            .iter()
            .find(|setting| setting.id == "model")
            .expect("model setting exists");

        assert_eq!(model.kind, ToolSettingKind::Select);
        assert_eq!(model.default.as_deref(), Some("gpt-5.4"));
        assert!(model.options.contains(&"gpt-5.4".to_string()));
        assert!(model.options.contains(&"gpt-5.3-codex".to_string()));
    }

    #[test]
    fn shell_descriptor_does_not_expose_intelligence_controls() {
        let shell = built_in_tools()
            .into_iter()
            .find(|tool| tool.id == "shell")
            .expect("shell descriptor exists");

        assert!(!shell.supports_context_meter);
        assert!(shell
            .settings
            .iter()
            .all(|setting| setting.id != "reasoning"));
        assert!(shell.settings.iter().all(|setting| setting.id != "model"));
    }
}
