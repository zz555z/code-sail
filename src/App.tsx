import { type MouseEvent, type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { History, LayoutDashboard, Settings2 } from "lucide-react";
import packageJson from "../package.json";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { ActiveToolProvider } from "./contexts/ActiveToolContext";
import { AppServicesProvider } from "./contexts/AppServicesContext";
import { HistoryProvider } from "./contexts/HistoryContext";
import { MessageProvider } from "./contexts/MessageContext";
import { ProviderEditorProvider } from "./contexts/ProviderEditorContext";
import { useActiveTool } from "./hooks/useActiveTool";
import { useAppUpdate } from "./hooks/useAppUpdate";
import { useHistorySessions } from "./hooks/useHistorySessions";
import { useProviderEditor } from "./hooks/useProviderEditor";
import { useThemePreference } from "./hooks/useThemePreference";
import { useToolStatuses } from "./hooks/useToolStatuses";
import { useTransientMessage } from "./hooks/useTransientMessage";
import { HistoryPage } from "./pages/HistoryPage";
import { ModelsPage } from "./pages/ModelsPage";
import { OverviewPage } from "./pages/OverviewPage";
import type { ToolType } from "./lib/types";

type PageId = "overview" | "models" | "history";
type TraySwitchPayload = {
  providerId: string;
  providerName: string;
  model: string | null;
};

function startWindowDrag(event: MouseEvent<HTMLElement>) {
  if (event.button !== 0) return;
  void getCurrentWindow().startDragging();
}

export function App() {
  const [activePage, setActivePage] = useState<PageId>("overview");
  const [initialLoading, setInitialLoading] = useState(true);
  const appVersion = packageJson.version;
  const { message, setMessage, setPaused: setMessagePaused, dismissMessage, messageClassName } = useTransientMessage();
  const { themePreference, cycleTheme } = useThemePreference();
  const { activeTool, switching: toolSwitching, loadActiveTool, switchTool } = useActiveTool();
  const providerEditor = useProviderEditor({ setMessage, setMessagePaused });
  const {
    toolStatuses, toolStatusesLoading, openingTerminal, installingTool,
    refreshToolStatuses, openInTerminal, openToolInstaller
  } = useToolStatuses({ setMessage });
  const historySessions = useHistorySessions({ activeTool, setMessage });
  const {
    appUpdate, checkingAppUpdate, openingAppUpdate,
    refreshAppUpdate, openUpdatePage
  } = useAppUpdate({ appVersion, setMessage });

  // Prevent text selection on drag
  useEffect(() => {
    const preventSelection = (e: Event) => {
      const target = e.target as HTMLElement;
      // Allow selection in input fields and content areas
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target.closest("input, textarea, code, pre, .conversation-message-content, .provider-details")
      ) {
        return;
      }
      e.preventDefault();
    };

    document.addEventListener("selectstart", preventSelection);
    return () => document.removeEventListener("selectstart", preventSelection);
  }, []);

  useEffect(() => {
    const initialize = async () => {
      try {
        // 只等待关键数据加载完成
        await Promise.allSettled([
          loadActiveTool(),
          providerEditor.refresh()
        ]);
      } finally {
        setInitialLoading(false);
      }
    };
    void initialize();

    // 非关键数据在后台加载
    void refreshToolStatuses();
    void refreshAppUpdate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const unlisten = listen<TraySwitchPayload>("tray-switch-provider", (event) => {
      void providerEditor.refresh({ preferredId: event.payload.providerId });
      setMessage(
        event.payload.model
          ? `已通过托盘切换到 ${event.payload.providerName} / ${event.payload.model}。`
          : `已通过托盘切换到 ${event.payload.providerName}。`
      );
    });

    return () => {
      void unlisten.then((stopListening) => stopListening());
    };
  }, [providerEditor.refresh, setMessage]);

  const handleToolSwitch = useCallback(async (tool: ToolType) => {
    setMessage("");
    await switchTool(tool);
    await providerEditor.refresh();
  }, [setMessage, switchTool, providerEditor.refresh]);

  useEffect(() => {
    if (activePage === "history" || activePage === "overview") {
      void historySessions.refreshHistory();
    }
  }, [activePage, activeTool, historySessions.refreshHistory]);

  const messageValue = useMemo(
    () => ({ message, messageClassName, setMessage, setMessagePaused, dismissMessage }),
    [message, messageClassName, setMessage, setMessagePaused, dismissMessage]
  );

  const appServicesValue = useMemo(
    () => ({
      appVersion,
      appUpdate,
      checkingAppUpdate,
      openingAppUpdate,
      refreshAppUpdate,
      openUpdatePage,
      toolStatuses,
      toolStatusesLoading,
      openingTerminal,
      installingTool,
      refreshToolStatuses,
      openInTerminal,
      openToolInstaller
    }),
    [
      appVersion, appUpdate, checkingAppUpdate, openingAppUpdate, refreshAppUpdate, openUpdatePage,
      toolStatuses, toolStatusesLoading, openingTerminal, installingTool,
      refreshToolStatuses, openInTerminal, openToolInstaller
    ]
  );

  const activeToolValue = useMemo(
    () => ({
      activeTool,
      switching: toolSwitching,
      switchTool: handleToolSwitch
    }),
    [activeTool, toolSwitching, handleToolSwitch]
  );

  const providerEditorValue = useMemo(
    () => providerEditor,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [providerEditor.state, providerEditor.selected, providerEditor.selectedId, providerEditor.draft, providerEditor.models, providerEditor.modelValue, providerEditor.providerCount, providerEditor.activeProvider, providerEditor.editorOpen, providerEditor.busy, providerEditor.importingProviders, providerEditor.restarting, providerEditor.loadingModels, providerEditor.modelMenuOpen, providerEditor.tokenVisible, providerEditor.updateConfigFile, providerEditor.canSave, providerEditor.healthCheckResults]
  );

  const historyValue = useMemo(
    () => historySessions,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [historySessions.historyGroups, historySessions.historyConversation, historySessions.selectedHistoryPath, historySessions.selectedHistorySession, historySessions.expandedHistoryProviders, historySessions.historyLoading, historySessions.historyBusy, historySessions.historyProviderStats, historySessions.topHistoryProviderStats, historySessions.historySessionCount, historySessions.historyMessageCount, historySessions.latestHistorySession]
  );

  const navItems: Array<{ id: PageId; label: string; icon: ReactNode }> = useMemo(
    () => [
      { id: "overview", label: "概览", icon: <LayoutDashboard size={20} /> },
      { id: "models", label: "模型配置", icon: <Settings2 size={20} /> },
      { id: "history", label: "历史记录", icon: <History size={20} /> }
    ],
    []
  );

  return (
    <main className="app-shell">
      <div className="window-drag-region" data-tauri-drag-region onMouseDown={startWindowDrag} />
      <aside className="app-nav">
        <nav className="nav-list" aria-label="主导航">
          {navItems.map((item) => (
            <button
              key={item.id}
              className={`nav-item ${activePage === item.id ? "active" : ""}`}
              type="button"
              aria-current={activePage === item.id ? "page" : undefined}
              onClick={() => {
                setMessage("");
                setActivePage(item.id);
              }}
            >
              {item.icon}
              <span>{item.label}</span>
            </button>
          ))}
        </nav>
      </aside>

      <ErrorBoundary>
        <ActiveToolProvider value={activeToolValue}>
          <MessageProvider value={messageValue}>
            <ProviderEditorProvider value={providerEditorValue}>
              <AppServicesProvider value={appServicesValue}>
                <HistoryProvider value={historyValue}>
                  <section className="workbench">
                    {initialLoading ? (
                      <div className="initial-loading">
                        <div className="loading-spinner" />
                        <span>正在加载配置...</span>
                      </div>
                    ) : (
                      <>
                        {activePage === "overview" ? (
                          <div className="page-transition" key="overview">
                            <ErrorBoundary>
                              <OverviewPage themePreference={themePreference} onCycleTheme={cycleTheme} />
                            </ErrorBoundary>
                          </div>
                        ) : null}

                        {activePage === "history" ? (
                          <div className="page-transition" key="history">
                            <ErrorBoundary>
                              <HistoryPage />
                            </ErrorBoundary>
                          </div>
                        ) : null}

                        {activePage === "models" ? (
                          <div className="page-transition" key="models">
                            <ErrorBoundary>
                              <ModelsPage />
                            </ErrorBoundary>
                          </div>
                        ) : null}
                      </>
                    )}
                  </section>
                </HistoryProvider>
              </AppServicesProvider>
            </ProviderEditorProvider>
          </MessageProvider>
        </ActiveToolProvider>
      </ErrorBoundary>
    </main>
  );
}
