import { useCallback, useEffect, useState } from "react";
import type { JinApiClient } from "../api/client";
import type { ChatSession, JinSettings, ProjectRecord, ToolDescriptor } from "../api/types";

export function useJinData(api: JinApiClient) {
  const [settings, setSettings] = useState<JinSettings>({ public_host: null });
  const [tools, setTools] = useState<ToolDescriptor[]>([]);
  const [projects, setProjects] = useState<ProjectRecord[]>([]);
  const [chats, setChats] = useState<ChatSession[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refreshChats = useCallback(async () => {
    const next = await api.listChats();
    setChats(next);
    return next;
  }, [api]);

  const refreshSettings = useCallback(async () => {
    const next = await api.getSettings();
    setSettings(next);
    return next;
  }, [api]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [nextSettings, nextTools, nextProjects, nextChats] = await Promise.all([
        api.getSettings(),
        api.listTools(),
        api.listProjects(),
        api.listChats(),
      ]);
      setSettings(nextSettings);
      setTools(nextTools);
      setProjects(nextProjects);
      setChats(nextChats);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load Jin data");
    } finally {
      setLoading(false);
    }
  }, [api]);

  useEffect(() => {
    void load();
  }, [load]);

  return {
    settings,
    tools,
    projects,
    chats,
    loading,
    error,
    refresh: load,
    refreshChats,
    refreshSettings,
  };
}
