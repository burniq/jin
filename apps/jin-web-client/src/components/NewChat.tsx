import { useEffect, useMemo, useState } from "react";
import type { ChatSession, ProjectRecord, SyncTarget, ToolDescriptor } from "../api/types";

export interface NewChatApi {
  createChat(payload: {
    project: string;
    tool: string;
    title: string | null;
    settings: Record<string, string>;
    sync_targets?: SyncTarget[] | null;
  }): Promise<ChatSession>;
  sendMessage(chatId: string, payload: { content: string }): Promise<unknown>;
}

interface NewChatProps {
  projects: ProjectRecord[];
  tools: ToolDescriptor[];
  api: NewChatApi;
  initialProject?: string | null;
  defaultSyncTargets?: SyncTarget[];
  onCreated: (chatId: string) => void;
}

export function NewChat({
  projects,
  tools,
  api,
  initialProject,
  defaultSyncTargets = [],
  onCreated,
}: NewChatProps) {
  const [project, setProject] = useState(() => resolveInitialProject(projects, initialProject));
  const [toolId, setToolId] = useState(tools[0]?.id ?? "codex");
  const [prompt, setPrompt] = useState("");
  const [title, setTitle] = useState("");
  const [settings, setSettings] = useState<Record<string, string>>({});
  const [selectedSyncTargetIds, setSelectedSyncTargetIds] = useState<Set<string>>(
    () => new Set(defaultSyncTargets.map((target) => target.id)),
  );
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const selectedTool = useMemo(
    () => tools.find((tool) => tool.id === toolId) ?? tools[0],
    [toolId, tools],
  );

  const visibleSettings = selectedTool?.settings ?? [];
  const selectedSyncTargets = useMemo(
    () => defaultSyncTargets.filter((target) => selectedSyncTargetIds.has(target.id)),
    [defaultSyncTargets, selectedSyncTargetIds],
  );

  const settingValue = (id: string, fallback: string | null) => settings[id] ?? fallback ?? "";

  useEffect(() => {
    const resolvedProject = resolveInitialProject(projects, initialProject);
    setProject((current) => {
      if (initialProject && resolvedProject && current !== resolvedProject) {
        return resolvedProject;
      }
      if (!current || !projects.some((item) => item.name === current)) {
        return resolvedProject;
      }
      return current;
    });
  }, [initialProject, projects]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const selectedSettings = Object.fromEntries(
        visibleSettings
          .map((setting) => [setting.id, settingValue(setting.id, setting.default)])
          .filter(([, value]) => value.trim().length > 0),
      );
      const chat = await api.createChat({
        project,
        tool: toolId,
        title: title.trim() ? title.trim() : null,
        settings: selectedSettings,
        sync_targets: selectedSyncTargets,
      });
      if (prompt.trim()) {
        await api.sendMessage(chat.id, { content: prompt.trim() });
      }
      onCreated(chat.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create chat");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form className="new-chat" onSubmit={submit}>
      <label className="prompt-label">
        <span>First prompt</span>
        <textarea
          value={prompt}
          onChange={(event) => setPrompt(event.target.value)}
          placeholder="Ask jin to work on a project..."
        />
      </label>
      <div className="composer-toolbar">
        <label>
          <span>Project</span>
          <select value={project} onChange={(event) => setProject(event.target.value)} required>
            {projects.map((item) => (
              <option key={item.name} value={item.name}>
                {item.name}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>Tool</span>
          <select value={toolId} onChange={(event) => setToolId(event.target.value)} required>
            {tools.map((tool) => (
              <option key={tool.id} value={tool.id}>
                {tool.name}
              </option>
            ))}
          </select>
        </label>
        {visibleSettings.map((setting) => (
          <label key={setting.id}>
            <span>{setting.label}</span>
            {setting.kind === "Select" ? (
              <select
                value={settingValue(setting.id, setting.default)}
                onChange={(event) =>
                  setSettings((current) => ({ ...current, [setting.id]: event.target.value }))
                }
              >
                {setting.options.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            ) : (
              <input
                value={settingValue(setting.id, setting.default)}
                onChange={(event) =>
                  setSettings((current) => ({ ...current, [setting.id]: event.target.value }))
                }
              />
            )}
          </label>
        ))}
        <details className="secondary-settings">
          <summary>More</summary>
          {defaultSyncTargets.length > 0 ? (
            <fieldset className="inline-options">
              <legend>Sync</legend>
              {defaultSyncTargets.map((target) => (
                <label key={target.id}>
                  <input
                    type="checkbox"
                    checked={selectedSyncTargetIds.has(target.id)}
                    onChange={() =>
                      setSelectedSyncTargetIds((current) => {
                        const next = new Set(current);
                        if (next.has(target.id)) {
                          next.delete(target.id);
                        } else {
                          next.add(target.id);
                        }
                        return next;
                      })
                    }
                  />
                  <span>{target.label}</span>
                </label>
              ))}
            </fieldset>
          ) : null}
          <label>
            <span>Title</span>
            <input value={title} onChange={(event) => setTitle(event.target.value)} />
          </label>
        </details>
        <button type="submit" disabled={submitting || !project || !toolId}>
          {submitting ? "Starting..." : "Start"}
        </button>
      </div>
      {error ? <p className="form-error">{error}</p> : null}
    </form>
  );
}

function resolveInitialProject(projects: ProjectRecord[], initialProject?: string | null) {
  if (initialProject && projects.some((project) => project.name === initialProject)) {
    return initialProject;
  }
  return projects[0]?.name ?? "";
}
