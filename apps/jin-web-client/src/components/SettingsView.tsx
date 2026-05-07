import { useState } from "react";
import type { JinSettings } from "../api/types";

interface SettingsViewApi {
  updateSettings(payload: JinSettings): Promise<JinSettings>;
}

interface SettingsViewProps {
  settings: JinSettings;
  api: SettingsViewApi;
  onChanged?: (settings: JinSettings) => void;
}

export function SettingsView({ settings, api, onChanged }: SettingsViewProps) {
  const [publicHost, setPublicHost] = useState(settings.public_host ?? "");
  const [telegramToken, setTelegramToken] = useState("");
  const [telegramGroupChatId, setTelegramGroupChatId] = useState(
    settings.telegram.default_group_chat_id ?? "",
  );
  const firstSyncTarget = settings.default_sync_targets[0];
  const [syncLabel, setSyncLabel] = useState(firstSyncTarget?.label ?? "");
  const [syncChatId, setSyncChatId] = useState(firstSyncTarget?.chat_id ?? "");
  const [syncThreadId, setSyncThreadId] = useState(
    firstSyncTarget?.message_thread_id ? String(firstSyncTarget.message_thread_id) : "",
  );
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    try {
      const next = await api.updateSettings({
        public_host: publicHost.trim() || null,
        telegram: {
          bot_token: telegramToken.trim() || null,
          bot_token_configured: settings.telegram.bot_token_configured,
          default_group_chat_id: telegramGroupChatId.trim() || null,
        },
        default_sync_targets:
          syncLabel.trim() && syncChatId.trim()
            ? [
                {
                  id: "telegram-default",
                  label: syncLabel.trim(),
                  kind: syncThreadId.trim() ? "TelegramForumTopic" : "TelegramChat",
                  chat_id: syncChatId.trim(),
                  message_thread_id: syncThreadId.trim() ? Number(syncThreadId.trim()) : null,
                },
              ]
            : [],
      });
      setStatus("Settings saved");
      setError(null);
      onChanged?.(next);
    } catch (err) {
      setStatus(null);
      setError(err instanceof Error ? err.message : "Failed to update settings");
    }
  };

  return (
    <section className="settings-view">
      <h1>Jin settings</h1>
      <form className="settings-form" onSubmit={save}>
        <label>
          <span>Public host</span>
          <input
            value={publicHost}
            placeholder={window.location.hostname}
            onChange={(event) => setPublicHost(event.target.value)}
          />
        </label>
        <section className="settings-panel">
          <h2>Telegram</h2>
          <p className="muted">
            Bot token is write-only. Current status:{" "}
            {settings.telegram.bot_token_configured ? "configured" : "not configured"}.
          </p>
          <label>
            <span>Bot token</span>
            <input
              type="password"
              value={telegramToken}
              placeholder={settings.telegram.bot_token_configured ? "Configured" : "Paste token"}
              onChange={(event) => setTelegramToken(event.target.value)}
            />
          </label>
          <label>
            <span>Default group chat id</span>
            <input
              value={telegramGroupChatId}
              placeholder="-100..."
              onChange={(event) => setTelegramGroupChatId(event.target.value)}
            />
          </label>
        </section>
        <section className="settings-panel">
          <h2>Default sync target</h2>
          <label>
            <span>Label</span>
            <input value={syncLabel} onChange={(event) => setSyncLabel(event.target.value)} />
          </label>
          <label>
            <span>Telegram chat id</span>
            <input value={syncChatId} onChange={(event) => setSyncChatId(event.target.value)} />
          </label>
          <label>
            <span>Telegram topic id</span>
            <input value={syncThreadId} onChange={(event) => setSyncThreadId(event.target.value)} />
          </label>
        </section>
        <button type="submit">Save settings</button>
      </form>
      {status ? <p className="form-status">{status}</p> : null}
      {error ? <p className="form-error">{error}</p> : null}
    </section>
  );
}
