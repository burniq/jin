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
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    try {
      const next = await api.updateSettings({
        public_host: publicHost.trim() || null,
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
        <button type="submit">Save settings</button>
      </form>
      {status ? <p className="form-status">{status}</p> : null}
      {error ? <p className="form-error">{error}</p> : null}
    </section>
  );
}
