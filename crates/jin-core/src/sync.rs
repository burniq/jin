use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_token: Option<String>,
    #[serde(default)]
    pub bot_token_configured: bool,
    #[serde(default)]
    pub default_group_chat_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncTarget {
    pub id: String,
    pub label: String,
    pub kind: SyncTargetKind,
    #[serde(default)]
    pub chat_id: Option<String>,
    #[serde(default)]
    pub message_thread_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncTargetKind {
    TelegramChat,
    TelegramForumTopic,
}

pub fn redacted_telegram_settings(settings: &TelegramSettings) -> TelegramSettings {
    TelegramSettings {
        bot_token: None,
        bot_token_configured: settings.bot_token.is_some() || settings.bot_token_configured,
        default_group_chat_id: settings.default_group_chat_id.clone(),
    }
}

pub fn normalize_sync_targets(targets: Vec<SyncTarget>) -> Vec<SyncTarget> {
    targets
        .into_iter()
        .filter_map(|target| {
            let id = target.id.trim();
            let label = target.label.trim();
            if id.is_empty() || label.is_empty() {
                return None;
            }
            Some(SyncTarget {
                id: id.to_string(),
                label: label.to_string(),
                kind: target.kind,
                chat_id: target
                    .chat_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                message_thread_id: target.message_thread_id,
            })
        })
        .collect()
}
