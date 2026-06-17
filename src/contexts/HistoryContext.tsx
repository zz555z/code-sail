import { createContext, useContext, type ReactNode } from "react";
import type { HistoryConversation, HistoryProviderGroup, HistorySessionSummary } from "../lib/types";

export type HistoryProviderStat = {
  provider: string;
  sessionCount: number;
};

export type HistoryContextValue = {
  historyGroups: HistoryProviderGroup[];
  historyConversation: HistoryConversation | null;
  selectedHistoryPath: string | null;
  selectedHistorySession: HistorySessionSummary | null;
  expandedHistoryProviders: Record<string, boolean>;
  historyLoading: boolean;
  historyBusy: boolean;
  historyProviderStats: HistoryProviderStat[];
  topHistoryProviderStats: HistoryProviderStat[];
  historySessionCount: number;
  historyMessageCount: number;
  latestHistorySession: HistorySessionSummary | null;
  refreshHistory: (options?: { preferredPath?: string | null }) => Promise<void>;
  openHistorySession: (session: HistorySessionSummary) => Promise<void>;
  toggleHistoryProvider: (provider: string) => void;
  resumeHistory: (session: HistorySessionSummary | null) => Promise<void>;
  removeHistorySession: (session: HistorySessionSummary | null) => Promise<void>;
  removeHistoryProvider: (group: HistoryProviderGroup) => Promise<void>;
};

const HistoryContext = createContext<HistoryContextValue | null>(null);

export function useHistoryContext(): HistoryContextValue {
  const ctx = useContext(HistoryContext);
  if (!ctx) throw new Error("useHistoryContext must be used within a HistoryProvider");
  return ctx;
}

export function HistoryProvider({
  value,
  children
}: {
  value: HistoryContextValue;
  children: ReactNode;
}) {
  return <HistoryContext.Provider value={value}>{children}</HistoryContext.Provider>;
}
