use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerRequest {
    pub task_id: String,
    pub working_directory: PathBuf,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerEvent {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerResult {
    pub runner_name: String,
    pub exit_code: i32,
    pub events: Vec<RunnerEvent>,
}

pub trait RunnerAdapter {
    fn run(&mut self, request: RunnerRequest) -> Result<RunnerResult, RunnerError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeRunner {
    name: String,
    exit_code: i32,
    output: Vec<RunnerEvent>,
    last_request: Option<RunnerRequest>,
}

impl FakeRunner {
    pub fn new(name: impl Into<String>, exit_code: i32) -> Self {
        Self {
            name: name.into(),
            exit_code,
            output: Vec::new(),
            last_request: None,
        }
    }

    pub fn push_output(&mut self, message: impl Into<String>) {
        self.output.push(RunnerEvent {
            message: message.into(),
        });
    }

    pub fn last_request(&self) -> Option<&RunnerRequest> {
        self.last_request.as_ref()
    }
}

impl RunnerAdapter for FakeRunner {
    fn run(&mut self, request: RunnerRequest) -> Result<RunnerResult, RunnerError> {
        self.last_request = Some(request);

        Ok(RunnerResult {
            runner_name: self.name.clone(),
            exit_code: self.exit_code,
            events: self.output.clone(),
        })
    }
}

#[derive(Debug, Default)]
pub struct ShellRunner;

impl ShellRunner {
    pub fn new() -> Self {
        Self
    }
}

impl RunnerAdapter for ShellRunner {
    fn run(&mut self, request: RunnerRequest) -> Result<RunnerResult, RunnerError> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&request.prompt)
            .current_dir(&request.working_directory)
            .output()
            .map_err(|error| RunnerError::Failed(error.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(RunnerResult {
            runner_name: "shell".to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            events: vec![RunnerEvent {
                message: format!("{stdout}{stderr}"),
            }],
        })
    }
}

#[derive(Debug, Default)]
pub struct CodexRunner;

impl CodexRunner {
    pub fn new() -> Self {
        Self
    }
}

impl RunnerAdapter for CodexRunner {
    fn run(&mut self, request: RunnerRequest) -> Result<RunnerResult, RunnerError> {
        let output = Command::new("codex")
            .arg("exec")
            .arg("--cd")
            .arg(&request.working_directory)
            .arg("--ask-for-approval")
            .arg("never")
            .arg("--sandbox")
            .arg("workspace-write")
            .arg(&request.prompt)
            .output()
            .map_err(|error| RunnerError::Failed(error.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        Ok(RunnerResult {
            runner_name: "codex".to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            events: vec![RunnerEvent {
                message: format!("{stdout}{stderr}"),
            }],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerError {
    Failed(String),
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for RunnerError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn fake_runner_records_output_and_returns_structured_result() {
        let mut runner = FakeRunner::new("codex", 0);
        runner.push_output("inspecting project");
        runner.push_output("done");

        let result = runner
            .run(RunnerRequest {
                task_id: "task-1".to_string(),
                working_directory: PathBuf::from("/tmp/jin"),
                prompt: "inspect".to_string(),
            })
            .expect("runner should succeed");

        assert_eq!(result.runner_name, "codex");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.events.len(), 2);
        assert_eq!(
            runner
                .last_request()
                .expect("request should be recorded")
                .task_id,
            "task-1"
        );
    }

    #[test]
    fn shell_runner_executes_command_in_working_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut runner = ShellRunner::new();

        let result = runner
            .run(RunnerRequest {
                task_id: "task-shell".to_string(),
                working_directory: temp.path().to_path_buf(),
                prompt: "printf shell-runner".to_string(),
            })
            .expect("shell runner succeeds");

        assert_eq!(result.runner_name, "shell");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.events[0].message, "shell-runner");
    }
}
