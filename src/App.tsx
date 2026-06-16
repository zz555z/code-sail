import { type MouseEvent, type ReactNode, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { History, LayoutDashboard, Settings2 } from "lucide-react";
import packageJson from "../package.json";
import { useHistorySessions } from "./hooks/useHistorySessions";
import { useProviderEditor } from "./hooks/useProviderEditor";
import { useThemePreference } from "./hooks/useThemePreference";
import { useToolStatuses } from "./hooks/useToolStatuses";
import { useTransientMessage } from "./hooks/useTransientMessage";
import { HistoryPage } from "./pages/HistoryPage";
import { ModelsPage } from "./pages/ModelsPage";
import { OverviewPage } from "./pages/OverviewPage";

type PageId = "overview" | "models" | "history";

function startWindowDrag(event: MouseEvent<HTMLElement>) {
  if (event.button !== 0) return;
  void getCurrentWindow().startDragging();
}

export function App() {
  const [activePage, setActivePage] = useState<PageId>("overview");
  const { message, setMessage, setPaused: setMessagePaused, messageClassName } = useTransientMessage();
  const { themePreference, cycleTheme } = useThemePreference();
  const providerEditor = useProviderEditor({ setMessage, setMessagePaused });
  const {
    state,
    selected,
    selectedId,
    draft,
    models,
    modelValue,
    providerCount,
    activeProvider,
    editorOpen,
    busy,
    restarting,
    loadingModels,
    modelMenuOpen,
    tokenVisible,
    updateConfigFile,
    canSave,
    modelComboboxRef,
    setUpdateConfigFile,
    setModelMenuOpen,
    setModelValue,
    toggleTokenVisible,
    refresh,
    restartCodex,
    openCreateProvider,
    openEditProvider,
    copyProvider,
    setCurrentProvider,
    removeProvider,
    closeEditor,
    updateDraft,
    selectModel,
    fetchProviderModels,
    saveCurrentProvider
  } = providerEditor;
  const {
    toolStatuses,
    toolStatusesLoading,
    openingCodexTerminal,
    installingTool,
    refreshToolStatuses,
    openCodexInTerminal,
    openToolInstaller
  } = useToolStatuses({ setMessage });
  const {
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
  } = useHistorySessions({ setMessage });

  useEffect(() => {
    void refresh();
    void refreshToolStatuses();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (activePage === "history" || activePage === "overview") {
      void refreshHistory();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activePage]);

  const navItems: Array<{ id: PageId; label: string; icon: ReactNode }> = [
    { id: "overview", label: "概览", icon: <LayoutDashboard size={20} /> },
    { id: "models", label: "模型配置", icon: <Settings2 size={20} /> },
    { id: "history", label: "历史记录", icon: <History size={20} /> }
  ];

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

      <section className="workbench">
        {activePage === "overview" ? (
          <OverviewPage
            appVersion={packageJson.version}
            state={state}
            activeProvider={activeProvider}
            message={message}
            messageClassName={messageClassName}
            themePreference={themePreference}
            busy={busy}
            historyLoading={historyLoading}
            toolStatusesLoading={toolStatusesLoading}
            toolStatuses={toolStatuses}
            openingCodexTerminal={openingCodexTerminal}
            installingTool={installingTool}
            historyProviderStats={historyProviderStats}
            topHistoryProviderStats={topHistoryProviderStats}
            historySessionCount={historySessionCount}
            historyMessageCount={historyMessageCount}
            latestHistorySession={latestHistorySession}
            onCycleTheme={cycleTheme}
            onRefresh={() => {
              void refresh();
              void refreshToolStatuses();
              void refreshHistory();
            }}
            onOpenCodexTerminal={() => void openCodexInTerminal()}
            onOpenToolInstall={(tool) => void openToolInstaller(tool)}
          />
        ) : null}

        {activePage === "history" ? (
          <HistoryPage
            message={message}
            messageClassName={messageClassName}
            historySessionCount={historySessionCount}
            historyGroups={historyGroups}
            historyConversation={historyConversation}
            selectedHistoryPath={selectedHistoryPath}
            selectedHistorySession={selectedHistorySession}
            expandedHistoryProviders={expandedHistoryProviders}
            historyLoading={historyLoading}
            historyBusy={historyBusy}
            onRefreshHistory={() => void refreshHistory()}
            onToggleHistoryProvider={toggleHistoryProvider}
            onOpenHistorySession={(session) => void openHistorySession(session)}
            onResumeHistory={(session) => void resumeHistory(session)}
            onDeleteHistorySession={(session) => void removeHistorySession(session)}
            onDeleteHistoryProvider={(group) => void removeHistoryProvider(group)}
          />
        ) : null}

        {activePage === "models" ? (
          <ModelsPage
            state={state}
            selected={selected}
            selectedId={selectedId}
            draft={draft}
            models={models}
            modelValue={modelValue}
            providerCount={providerCount}
            message={message}
            messageClassName={messageClassName}
            editorOpen={editorOpen}
            busy={busy}
            restarting={restarting}
            loadingModels={loadingModels}
            modelMenuOpen={modelMenuOpen}
            tokenVisible={tokenVisible}
            updateConfigFile={updateConfigFile}
            canSave={canSave}
            modelComboboxRef={modelComboboxRef}
            onSetUpdateConfigFile={setUpdateConfigFile}
            onRestartCodex={() => void restartCodex()}
            onRefresh={() => void refresh()}
            onCreateProvider={openCreateProvider}
            onEditProvider={openEditProvider}
            onCopyProvider={(providerId) => void copyProvider(providerId)}
            onSetCurrentProvider={(provider) => void setCurrentProvider(provider)}
            onDeleteProvider={(providerId) => void removeProvider(providerId)}
            onCloseEditor={closeEditor}
            onUpdateDraft={updateDraft}
            onToggleTokenVisible={toggleTokenVisible}
            onSetModelMenuOpen={setModelMenuOpen}
            onSetModelValue={setModelValue}
            onSelectModel={selectModel}
            onFetchModels={() => void fetchProviderModels()}
            onSave={() => void saveCurrentProvider()}
          />
        ) : null}
      </section>
    </main>
  );
}
