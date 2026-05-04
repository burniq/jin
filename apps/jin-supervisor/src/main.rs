use clap::{Parser, Subcommand};
use jin_core::version::{HealthStatus, VersionRecord, VersionRegistry};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: SupervisorCommand,
}

#[derive(Debug, Subcommand)]
enum SupervisorCommand {
    Status {
        #[arg(long, default_value = ".jin/state/versions.json")]
        state: PathBuf,
    },
    InitStable {
        #[arg(long, default_value = ".jin/state/versions.json")]
        state: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        source_ref: String,
        #[arg(long)]
        artifact_path: String,
    },
    SetCandidate {
        #[arg(long, default_value = ".jin/state/versions.json")]
        state: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        source_ref: String,
        #[arg(long)]
        artifact_path: String,
        #[arg(long)]
        unhealthy: bool,
    },
    Promote {
        #[arg(long, default_value = ".jin/state/versions.json")]
        state: PathBuf,
    },
    Rollback {
        #[arg(long, default_value = ".jin/state/versions.json")]
        state: PathBuf,
    },
}

fn main() {
    let args = Args::parse();
    match run_command(args.command) {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("jin-supervisor failed: {error}");
            std::process::exit(1);
        }
    }
}

fn run_command(command: SupervisorCommand) -> Result<String, SupervisorError> {
    match command {
        SupervisorCommand::Status { state } => {
            let registry = load_registry(&state)?;
            Ok(serde_json::to_string_pretty(&registry)?)
        }
        SupervisorCommand::InitStable {
            state,
            name,
            source_ref,
            artifact_path,
        } => {
            let registry = VersionRegistry::new(VersionRecord {
                name,
                source_ref,
                artifact_path,
                health: HealthStatus::Healthy,
            });
            save_registry(&state, &registry)?;
            Ok("initialized stable version".to_string())
        }
        SupervisorCommand::SetCandidate {
            state,
            name,
            source_ref,
            artifact_path,
            unhealthy,
        } => {
            let mut registry = load_registry(&state)?;
            registry.set_candidate(VersionRecord {
                name,
                source_ref,
                artifact_path,
                health: if unhealthy {
                    HealthStatus::Unhealthy
                } else {
                    HealthStatus::Healthy
                },
            });
            save_registry(&state, &registry)?;
            Ok("candidate set".to_string())
        }
        SupervisorCommand::Promote { state } => {
            let mut registry = load_registry(&state)?;
            registry.promote_candidate()?;
            let stable_name = registry.stable().name.clone();
            save_registry(&state, &registry)?;
            Ok(format!("promoted {stable_name}"))
        }
        SupervisorCommand::Rollback { state } => {
            let mut registry = load_registry(&state)?;
            let artifact = registry.rollback()?;
            save_registry(&state, &registry)?;
            Ok(format!("rollback artifact: {artifact}"))
        }
    }
}

fn load_registry(path: &Path) -> Result<VersionRegistry, SupervisorError> {
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn save_registry(path: &Path, registry: &VersionRegistry) -> Result<(), SupervisorError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(registry)?)?;
    Ok(())
}

#[derive(Debug)]
enum SupervisorError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Version(jin_core::version::VersionError),
}

impl From<std::io::Error> for SupervisorError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for SupervisorError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<jin_core::version::VersionError> for SupervisorError {
    fn from(error: jin_core::version::VersionError) -> Self {
        Self::Version(error)
    }
}

impl std::fmt::Display for SupervisorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Json(error) => write!(f, "json error: {error}"),
            Self::Version(error) => write!(f, "version error: {error:?}"),
        }
    }
}

impl std::error::Error for SupervisorError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_promotes_candidate_and_rolls_back_to_previous_artifact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let state = temp.path().join("versions.json");

        run_command(SupervisorCommand::InitStable {
            state: state.clone(),
            name: "stable-a".to_string(),
            source_ref: "commit-a".to_string(),
            artifact_path: "/opt/jin/stable-a".to_string(),
        })
        .expect("init stable");

        run_command(SupervisorCommand::SetCandidate {
            state: state.clone(),
            name: "candidate-b".to_string(),
            source_ref: "commit-b".to_string(),
            artifact_path: "/opt/jin/candidate-b".to_string(),
            unhealthy: false,
        })
        .expect("set candidate");

        run_command(SupervisorCommand::Promote {
            state: state.clone(),
        })
        .expect("promote");
        let rollback_output = run_command(SupervisorCommand::Rollback { state }).expect("rollback");

        assert!(rollback_output.contains("/opt/jin/stable-a"));
    }
}
