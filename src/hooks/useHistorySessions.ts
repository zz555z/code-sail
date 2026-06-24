import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  deleteHistoryProvider,
  deleteHistorySession,
  listHistorySessions,
  readHistorySession,
  resumeHistorySession
} from "../lib/api";
import { formatDeleteHistoryFailure } from "../lib/format";
import type { HistoryConversation, HistoryProviderGroup, HistorySessionSummary, ToolType } from "../lib/types";
import { errorMessage } from "../lib/utils";

type UseHistorySessionsOptions = {
  activeTool: ToolType;
  setMessage: (message: string) => void;
};

export function useHistorySessions({ activeTool, setMessage }: UseHistorySessionsOptions) {
  const [historyGroups, setHistoryGroups] = useState<HistoryProviderGroup[]>([]);
  const [historyConversation, setHistoryConversation] = useState<HistoryConversation | null>(null);
  const [selectedHistoryPath, setSelectedHistoryPath] = useState<string | null>(null);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyBusy, setHistoryBusy] = useState(false);
  const [expandedHistoryProviders, setExpandedHistoryProviders] = useState<Record<string, boolean>>({});
  const selectedHistoryPathRef = useRef(selectedHistoryPath);
  const historyRequestRef = useRef(0);
  selectedHistoryPathRef.current = selectedHistoryPath;

  const allHistorySessions = useMemo(() => historyGroups.flatMap((group) => group.sessions), [historyGroups]);
  const historySessionCount = allHistorySessions.length;
  const historyMessageCount = useMemo(
    () => allHistorySessions.reduce((total, session) => total + session.messageCount, 0),
    [allHistorySessions]
  );
  const historyProviderStats = useMemo(
    () =>
      historyGroups
        .map((group) => ({
          provider: group.provider,
          sessionCount: group.sessions.length
        }))
        .sort(
          (left, right) =>
            right.sessionCount - left.sessionCount || left.provider.localeCompare(right.provider)
        ),
    [historyGroups]
  );
  const topHistoryProviderStats = useMemo(() => historyProviderStats.slice(0, 3), [historyProviderStats]);
  const latestHistorySession = useMemo(
    () =>
      allHistorySessions.reduce<HistorySessionSummary | null>((latest, session) => {
        if (!latest) return session;
        return (session.timestamp ?? 0) > (latest.timestamp ?? 0) ? session : latest;
      }, null),
    [allHistorySessions]
  );
  const selectedHistorySession = useMemo(
    () => allHistorySessions.find((session) => session.path === selectedHistoryPath) || null,
    [allHistorySessions, selectedHistoryPath]
  );

  useEffect(() => {
    historyRequestRef.current += 1;
    setHistoryGroups([]);
    setHistoryConversation(null);
    setSelectedHistoryPath(null);
    setExpandedHistoryProviders({});
  }, [activeTool]);

  const refreshHistory = useCallback(async (options?: { preferredPath?: string | null }) => {
    const requestId = historyRequestRef.current + 1;
    historyRequestRef.current = requestId;
    setHistoryLoading(true);
    setMessage("");
    try {
      const groups = await listHistorySessions(activeTool);
      if (requestId !== historyRequestRef.current) return;
      const sessions = groups.flatMap((group) => group.sessions);
      const hasPreferredPath = Object.prototype.hasOwnProperty.call(options || {}, "preferredPath");
      const desiredPath = hasPreferredPath ? options?.preferredPath ?? null : selectedHistoryPathRef.current;
      const nextSelectedPath =
        desiredPath && sessions.some((session) => session.path === desiredPath)
          ? desiredPath
          : sessions[0]?.path ?? null;
      const conversation = nextSelectedPath ? await readHistorySession(nextSelectedPath, activeTool) : null;
      if (requestId !== historyRequestRef.current) return;

      setHistoryGroups(groups);
      setExpandedHistoryProviders((current) => {
        const next = { ...current };
        for (const group of groups) {
          if (next[group.provider] === undefined) next[group.provider] = true;
        }
        return next;
      });
      setSelectedHistoryPath(nextSelectedPath);
      setHistoryConversation(conversation);
    } catch (error) {
      if (requestId !== historyRequestRef.current) return;
      setHistoryConversation(null);
      setMessage(errorMessage(error));
    } finally {
      if (requestId === historyRequestRef.current) {
        setHistoryLoading(false);
      }
    }
  }, [activeTool, setMessage]);

  const openHistorySession = useCallback(async (session: HistorySessionSummary) => {
    const requestId = historyRequestRef.current + 1;
    historyRequestRef.current = requestId;
    setSelectedHistoryPath(session.path);
    setHistoryLoading(true);
    setMessage("");
    try {
      const conversation = await readHistorySession(session.path, activeTool);
      if (requestId !== historyRequestRef.current) return;
      setHistoryConversation(conversation);
    } catch (error) {
      if (requestId !== historyRequestRef.current) return;
      setHistoryConversation(null);
      setMessage(errorMessage(error));
    } finally {
      if (requestId === historyRequestRef.current) {
        setHistoryLoading(false);
      }
    }
  }, [activeTool, setMessage]);

  const toggleHistoryProvider = useCallback((provider: string) => {
    setExpandedHistoryProviders((current) => ({
      ...current,
      [provider]: !(current[provider] ?? true)
    }));
  }, []);

  const resumeHistory = useCallback(async (session: HistorySessionSummary | null) => {
    if (!session) return;
    setHistoryBusy(true);
    setMessage("");
    try {
      await resumeHistorySession(session.sessionId, activeTool);
      setMessage(`已打开终端恢复会话 ${session.sessionId}。`);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setHistoryBusy(false);
    }
  }, [activeTool, setMessage]);

  const removeHistorySession = useCallback(async (session: HistorySessionSummary | null) => {
    if (!session) return;
    setHistoryBusy(true);
    setMessage("");
    try {
      const result = await deleteHistorySession(session.path, activeTool);
      await refreshHistory({ preferredPath: null });
      setMessage(result.failureCount ? formatDeleteHistoryFailure(result) : `已删除会话 ${session.sessionId}。`);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setHistoryBusy(false);
    }
  }, [activeTool, setMessage, refreshHistory]);

  const removeHistoryProvider = useCallback(async (group: HistoryProviderGroup) => {
    setHistoryBusy(true);
    setMessage("");
    try {
      const result = await deleteHistoryProvider(group.provider, activeTool);
      await refreshHistory({ preferredPath: null });
      setMessage(
        result.failureCount
          ? `已删除 ${result.successCount} 条，${formatDeleteHistoryFailure(result)}`
          : `已删除 ${result.successCount} 条历史会话。`
      );
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setHistoryBusy(false);
    }
  }, [activeTool, setMessage, refreshHistory]);

  return useMemo(() => ({
    historyGroups,
    historyConversation,
    selectedHistoryPath,
    selectedHistorySession,
    expandedHistoryProviders,
    historyLoading,
    historyBusy,
    historyProviderStats,
    topHistoryProviderStats,
    historySessionCount,
    historyMessageCount,
    latestHistorySession,
    refreshHistory,
    openHistorySession,
    toggleHistoryProvider,
    resumeHistory,
    removeHistorySession,
    removeHistoryProvider
  }), [
    historyGroups,
    historyConversation,
    selectedHistoryPath,
    selectedHistorySession,
    expandedHistoryProviders,
    historyLoading,
    historyBusy,
    historyProviderStats,
    topHistoryProviderStats,
    historySessionCount,
    historyMessageCount,
    latestHistorySession,
    refreshHistory,
    openHistorySession,
    toggleHistoryProvider,
    resumeHistory,
    removeHistorySession,
    removeHistoryProvider
  ]);
}
