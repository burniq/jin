#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Result<Self, TaskError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(TaskError::BlankTaskId);
        }

        Ok(Self(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Queued,
    Running,
    WaitingApproval,
    Cancelled,
    Failed,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardedOperation {
    ShellCommand(String),
    PromoteJinVersion,
    RollbackJin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Approved,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    id: String,
    operation: GuardedOperation,
    reason: String,
    decision: Option<ApprovalDecision>,
    decided_by: Option<String>,
}

impl ApprovalRequest {
    pub fn new(
        id: impl Into<String>,
        operation: GuardedOperation,
        reason: impl Into<String>,
    ) -> Result<Self, TaskError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(TaskError::BlankApprovalId);
        }

        Ok(Self {
            id,
            operation,
            reason: reason.into(),
            decision: None,
            decided_by: None,
        })
    }

    pub fn decision(&self) -> Option<ApprovalDecision> {
        self.decision
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    id: TaskId,
    description: String,
    state: TaskState,
    approvals: Vec<ApprovalRequest>,
}

impl Task {
    pub fn new(id: TaskId, description: String) -> Self {
        Self {
            id,
            description,
            state: TaskState::Queued,
            approvals: Vec::new(),
        }
    }

    pub fn state(&self) -> TaskState {
        self.state
    }

    pub fn approvals(&self) -> &[ApprovalRequest] {
        &self.approvals
    }

    pub fn request_approval(&mut self, approval: ApprovalRequest) {
        self.approvals.push(approval);
        self.state = TaskState::WaitingApproval;
    }

    pub fn approve(&mut self, actor: impl Into<String>) -> Result<(), TaskError> {
        let approval = self
            .approvals
            .last_mut()
            .ok_or(TaskError::NoPendingApproval)?;

        if approval.decision.is_some() {
            return Err(TaskError::ApprovalAlreadyDecided);
        }

        approval.decision = Some(ApprovalDecision::Approved);
        approval.decided_by = Some(actor.into());
        self.state = TaskState::Running;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    BlankTaskId,
    BlankApprovalId,
    NoPendingApproval,
    ApprovalAlreadyDecided,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_waits_for_approval_and_resumes_after_approval() {
        let mut task = Task::new(
            TaskId::new("task-1").expect("valid task id"),
            "build jin".to_string(),
        );

        let approval = ApprovalRequest::new(
            "approval-1",
            GuardedOperation::ShellCommand("cargo test --workspace".to_string()),
            "shell command requires confirmation".to_string(),
        )
        .expect("valid approval");

        task.request_approval(approval);
        assert_eq!(task.state(), TaskState::WaitingApproval);

        task.approve("nikita").expect("approval should resume task");

        assert_eq!(task.state(), TaskState::Running);
        assert_eq!(
            task.approvals()[0].decision(),
            Some(ApprovalDecision::Approved)
        );
    }
}
use serde::{Deserialize, Serialize};
