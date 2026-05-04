#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PolicyConfig {
    pub allowed_shell_commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyEngine {
    config: PolicyConfig,
}

impl PolicyEngine {
    pub fn new(config: PolicyConfig) -> Self {
        Self { config }
    }

    pub fn evaluate(&self, action: &GuardedAction) -> PolicyDecision {
        match action {
            GuardedAction::ShellCommand(command) => {
                if self
                    .config
                    .allowed_shell_commands
                    .iter()
                    .any(|allowed| allowed == command)
                {
                    PolicyDecision::Allow
                } else {
                    PolicyDecision::RequireApproval {
                        reason: "shell command is not in the allowlist".to_string(),
                    }
                }
            }
            GuardedAction::PromoteJinVersion => PolicyDecision::RequireApproval {
                reason: "changes to jin require explicit approval".to_string(),
            },
            GuardedAction::GitPush => PolicyDecision::RequireApproval {
                reason: "git push requires explicit approval".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardedAction {
    ShellCommand(String),
    GitPush,
    PromoteJinVersion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    RequireApproval { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_allows_small_shell_allowlist_and_guards_other_shell_commands() {
        let policy = PolicyEngine::new(PolicyConfig {
            allowed_shell_commands: vec!["git status".to_string()],
        });

        assert_eq!(
            policy.evaluate(&GuardedAction::ShellCommand("git status".to_string())),
            PolicyDecision::Allow
        );
        assert_eq!(
            policy.evaluate(&GuardedAction::ShellCommand(
                "cargo test --workspace".to_string()
            )),
            PolicyDecision::RequireApproval {
                reason: "shell command is not in the allowlist".to_string()
            }
        );
    }

    #[test]
    fn policy_guards_jin_promotion() {
        let policy = PolicyEngine::new(PolicyConfig::default());

        assert_eq!(
            policy.evaluate(&GuardedAction::PromoteJinVersion),
            PolicyDecision::RequireApproval {
                reason: "changes to jin require explicit approval".to_string()
            }
        );
    }
}
