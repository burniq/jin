use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramUpdate {
    pub message: Option<TelegramMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramMessage {
    pub chat: TelegramChat,
    pub from: Option<TelegramUser>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramCommand {
    CreateTask {
        chat_id: i64,
        user_id: Option<i64>,
        project: String,
        runner: String,
        command: String,
    },
    Approve {
        chat_id: i64,
        user_id: Option<i64>,
        approval_id: String,
    },
    Reject {
        chat_id: i64,
        user_id: Option<i64>,
        approval_id: String,
    },
}

pub fn parse_update(update: TelegramUpdate) -> Result<TelegramCommand, TelegramParseError> {
    let message = update.message.ok_or(TelegramParseError::MissingMessage)?;
    let text = message.text.ok_or(TelegramParseError::MissingText)?;
    let user_id = message.from.map(|from| from.id);
    parse_text(message.chat.id, user_id, &text)
}

fn parse_text(
    chat_id: i64,
    user_id: Option<i64>,
    text: &str,
) -> Result<TelegramCommand, TelegramParseError> {
    let mut parts = text.trim().splitn(3, char::is_whitespace);
    let command = parts.next().ok_or(TelegramParseError::MissingText)?;

    match command {
        "/shell" => {
            let project = parts.next().ok_or(TelegramParseError::MissingProject)?;
            let command = parts
                .next()
                .ok_or(TelegramParseError::MissingCommand)?
                .trim();
            if command.is_empty() {
                return Err(TelegramParseError::MissingCommand);
            }
            Ok(TelegramCommand::CreateTask {
                chat_id,
                user_id,
                project: project.to_string(),
                runner: "shell".to_string(),
                command: command.to_string(),
            })
        }
        "/codex" => {
            let project = parts.next().ok_or(TelegramParseError::MissingProject)?;
            let prompt = parts
                .next()
                .ok_or(TelegramParseError::MissingCommand)?
                .trim();
            if prompt.is_empty() {
                return Err(TelegramParseError::MissingCommand);
            }
            Ok(TelegramCommand::CreateTask {
                chat_id,
                user_id,
                project: project.to_string(),
                runner: "codex".to_string(),
                command: prompt.to_string(),
            })
        }
        "/approve" => Ok(TelegramCommand::Approve {
            chat_id,
            user_id,
            approval_id: parts
                .next()
                .ok_or(TelegramParseError::MissingApprovalId)?
                .to_string(),
        }),
        "/reject" => Ok(TelegramCommand::Reject {
            chat_id,
            user_id,
            approval_id: parts
                .next()
                .ok_or(TelegramParseError::MissingApprovalId)?
                .to_string(),
        }),
        _ => Err(TelegramParseError::UnknownCommand),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramParseError {
    MissingMessage,
    MissingText,
    MissingProject,
    MissingCommand,
    MissingApprovalId,
    UnknownCommand,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shell_command_from_telegram_update() {
        let command = parse_update(TelegramUpdate {
            message: Some(TelegramMessage {
                chat: TelegramChat { id: 10 },
                from: Some(TelegramUser { id: 20 }),
                text: Some("/shell jin printf hello".to_string()),
            }),
        })
        .expect("update parses");

        assert_eq!(
            command,
            TelegramCommand::CreateTask {
                chat_id: 10,
                user_id: Some(20),
                project: "jin".to_string(),
                runner: "shell".to_string(),
                command: "printf hello".to_string(),
            }
        );
    }

    #[test]
    fn parses_approval_command_from_telegram_update() {
        let command = parse_update(TelegramUpdate {
            message: Some(TelegramMessage {
                chat: TelegramChat { id: 10 },
                from: Some(TelegramUser { id: 20 }),
                text: Some("/approve approval-1".to_string()),
            }),
        })
        .expect("update parses");

        assert_eq!(
            command,
            TelegramCommand::Approve {
                chat_id: 10,
                user_id: Some(20),
                approval_id: "approval-1".to_string(),
            }
        );
    }
}
