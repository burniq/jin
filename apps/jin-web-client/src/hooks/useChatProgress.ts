import { useEffect, useState } from "react";
import type { ChatMessagesQuery, ChatProgressResponse } from "../api/types";

export interface ProgressApi {
  getChatProgress(chatId: string, query?: ChatMessagesQuery): Promise<ChatProgressResponse>;
}

export interface UseChatProgressOptions {
  chatId: string | null;
  enabled: boolean;
  api: ProgressApi;
  query?: ChatMessagesQuery;
  pollMs?: number;
  onProgress?: (progress: ChatProgressResponse) => void;
}

export function useChatProgress({
  chatId,
  enabled,
  api,
  query,
  pollMs = 1200,
  onProgress,
}: UseChatProgressOptions) {
  const [progress, setProgress] = useState<ChatProgressResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!enabled || !chatId) {
      return;
    }

    let cancelled = false;
    let interval: number | undefined;

    const poll = async () => {
      try {
        const next = await api.getChatProgress(chatId, query);
        if (cancelled) {
          return;
        }
        setProgress(next);
        setError(null);
        onProgress?.(next);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to load chat progress");
        }
      }
    };

    void poll();
    interval = window.setInterval(() => void poll(), pollMs);

    return () => {
      cancelled = true;
      if (interval !== undefined) {
        window.clearInterval(interval);
      }
    };
  }, [api, chatId, enabled, onProgress, pollMs, query]);

  return { progress, error };
}
