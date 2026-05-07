import { useCallback, useEffect, useState } from "react";
import type { JinApiClient } from "../api/client";
import type {
  ChatSession,
  FactoryPipeline,
  JinSettings,
  ProjectRecord,
  ToolDescriptor,
} from "../api/types";

const defaultSettings: JinSettings = {
  public_host: null,
  telegram: {
    bot_token: null,
    bot_token_configured: false,
    default_group_chat_id: null,
  },
  default_sync_targets: [],
};

export function useJinData(api: JinApiClient) {
  const [settings, setSettings] = useState<JinSettings>(defaultSettings);
  const [tools, setTools] = useState<ToolDescriptor[]>([]);
  const [projects, setProjects] = useState<ProjectRecord[]>([]);
  const [chats, setChats] = useState<ChatSession[]>([]);
  const [factories, setFactories] = useState<FactoryPipeline[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refreshChats = useCallback(async () => {
    const next = await api.listChats();
    setChats(next);
    return next;
  }, [api]);

  const refreshFactories = useCallback(async () => {
    const next = await api.listFactories();
    setFactories(next);
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
      const [nextSettings, nextTools, nextProjects, nextChats, nextFactories] = await Promise.all([
        api.getSettings(),
        api.listTools(),
        api.listProjects(),
        api.listChats(),
        api.listFactories(),
      ]);
      setSettings(nextSettings);
      setTools(nextTools);
      setProjects(nextProjects);
      setChats(nextChats);
      setFactories(nextFactories);
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
    factories,
    loading,
    error,
    refresh: load,
    refreshChats,
    refreshFactories,
    refreshSettings,
  };
}
