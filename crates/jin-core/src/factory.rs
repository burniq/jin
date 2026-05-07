use crate::sync::SyncTarget;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectContentProfile {
    pub project: String,
    #[serde(default)]
    pub audience: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub content_pillars: Vec<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub publish_channels: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectContentProfileUpdate {
    pub project: String,
    #[serde(default)]
    pub audience: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub persona: Option<String>,
    #[serde(default)]
    pub content_pillars: Vec<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub publish_channels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateFactoryPipelineRequest {
    pub project: String,
    #[serde(default)]
    pub title: Option<String>,
    pub brief: String,
    pub mode: FactoryPipelineMode,
    pub review_policy: FactoryReviewPolicy,
    #[serde(default)]
    pub content_types: Vec<FactoryArtifactKind>,
    #[serde(default)]
    pub output_path: Option<PathBuf>,
    #[serde(default)]
    pub sync_targets: Option<Vec<SyncTarget>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryPipeline {
    pub id: String,
    pub project: String,
    pub title: String,
    pub brief: String,
    pub mode: FactoryPipelineMode,
    pub review_policy: FactoryReviewPolicy,
    pub status: FactoryPipelineStatus,
    #[serde(default)]
    pub content_types: Vec<FactoryArtifactKind>,
    #[serde(default)]
    pub output_path: Option<PathBuf>,
    pub schedule: FactorySchedule,
    #[serde(default)]
    pub sync_targets: Vec<SyncTarget>,
    #[serde(default)]
    pub stages: Vec<FactoryStage>,
    #[serde(default)]
    pub artifacts: Vec<FactoryArtifact>,
    #[serde(default)]
    pub review_bundles: Vec<ReviewBundle>,
    #[serde(default)]
    pub events: Vec<FactoryEvent>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactoryPipelineMode {
    Finite,
    Continuous,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactoryReviewPolicy {
    FinalOnly,
    PerStage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactoryPipelineStatus {
    Draft,
    Scheduled,
    Running,
    WaitingApproval,
    WaitingCapacity,
    Paused,
    Completed,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactoryArtifactKind {
    Text,
    Script,
    Image,
    ThreeD,
    Music,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactorySchedule {
    #[serde(default)]
    pub run_window: Option<String>,
    #[serde(default)]
    pub pause_between_iterations_minutes: Option<u32>,
    #[serde(default)]
    pub max_iterations_per_window: Option<u32>,
    #[serde(default)]
    pub max_artifacts_per_run: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryStage {
    pub stage_type: FactoryStageType,
    pub status: FactoryStageStatus,
    pub revision: u32,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactoryStageType {
    Brief,
    Research,
    Plan,
    Generate,
    Refine,
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactoryStageStatus {
    Pending,
    Running,
    WaitingApproval,
    Approved,
    NeedsChanges,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryArtifact {
    pub id: String,
    pub kind: FactoryArtifactKind,
    pub stage_type: FactoryStageType,
    pub status: FactoryArtifactStatus,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub preview: Option<String>,
    #[serde(default)]
    pub files: Vec<PathBuf>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub revision: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactoryArtifactStatus {
    Draft,
    WaitingApproval,
    Approved,
    Rejected,
    NeedsChanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewBundle {
    pub id: String,
    pub stage_type: FactoryStageType,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
    pub status: ReviewBundleStatus,
    #[serde(default)]
    pub request_changes: Option<String>,
    pub revision: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewBundleStatus {
    WaitingApproval,
    Approved,
    Rejected,
    NeedsChanges,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactoryEvent {
    pub id: String,
    pub pipeline_id: String,
    pub kind: FactoryEventKind,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FactoryEventKind {
    System,
    Worker,
    Approval,
    Error,
}

pub fn default_factory_stages() -> Vec<FactoryStage> {
    [
        FactoryStageType::Brief,
        FactoryStageType::Research,
        FactoryStageType::Plan,
        FactoryStageType::Generate,
        FactoryStageType::Refine,
        FactoryStageType::Review,
    ]
    .into_iter()
    .map(|stage_type| FactoryStage {
        stage_type,
        status: FactoryStageStatus::Pending,
        revision: 0,
        notes: None,
    })
    .collect()
}
