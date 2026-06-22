import { type MouseEvent, type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
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

function startWindowDrag(event: MouseEvent<HTMLElement>) {
  if (event.button !== 0) return;
  void getCurrentWindow().startDragging();
}

export function App() {
  const [activePage, setActivePage] = useState<PageId>("overview");
  const appVersion = packageJson.version;
  const { message, setMessage, setPaused: setMessagePaused, messageClassName } = useTransientMessage();
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

  useEffect(() => {
    void loadActiveTool();
    void providerEditor.refresh();
    void refreshToolStatuses();
    void refreshAppUpdate();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
    () => ({ message, messageClassName, setMessage, setMessagePaused }),
    [message, messageClassName, setMessage, setMessagePaused]
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
            <ProviderEditorProvider value={providerEditor}>
              <AppServicesProvider value={appServicesValue}>
                <HistoryProvider value={historySessions}>
                  <section className="workbench">
                    {activePage === "overview" ? (
                      <ErrorBoundary>
                        <OverviewPage themePreference={themePreference} onCycleTheme={cycleTheme} />
                      </ErrorBoundary>
                    ) : null}

                    {activePage === "history" ? (
                      <ErrorBoundary>
                        <HistoryPage />
                      </ErrorBoundary>
                    ) : null}

                    {activePage === "models" ? (
                      <ErrorBoundary>
                        <ModelsPage />
                      </ErrorBoundary>
                    ) : null}
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
