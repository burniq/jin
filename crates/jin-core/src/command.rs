#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSource {
    Telegram { chat_id: i64, user_id: i64 },
    Http { actor: String, request_id: String },
}

impl InputSource {
    pub fn adapter_name(&self) -> &'static str {
        match self {
            Self::Telegram { .. } => "telegram",
            Self::Http { .. } => "http",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEnvelope {
    pub source: InputSource,
    pub command_text: String,
    pub target_project: Option<String>,
}

impl CommandEnvelope {
    pub fn new(
        source: InputSource,
        command_text: impl AsRef<str>,
        target_project: Option<String>,
    ) -> Result<Self, CommandError> {
        let command_text = command_text.as_ref().trim();
        if command_text.is_empty() {
            return Err(CommandError::BlankCommand);
        }

        Ok(Self {
            source,
            command_text: command_text.to_string(),
            target_project,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    BlankCommand,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_envelope_rejects_blank_text() {
        let result = CommandEnvelope::new(
            InputSource::Http {
                actor: "nikita".to_string(),
                request_id: "req-1".to_string(),
            },
            "   ",
            None,
        );

        assert_eq!(result, Err(CommandError::BlankCommand));
    }

    #[test]
    fn command_envelope_keeps_source_and_trimmed_text() {
        let envelope = CommandEnvelope::new(
            InputSource::Telegram {
                chat_id: 10,
                user_id: 20,
            },
            "  test jin  ",
            Some("jin".to_string()),
        )
        .expect("command should be valid");

        assert_eq!(envelope.command_text, "test jin");
        assert_eq!(envelope.target_project.as_deref(), Some("jin"));
        assert_eq!(envelope.source.adapter_name(), "telegram");
    }
}
