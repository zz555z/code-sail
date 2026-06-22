import {
  AlertCircle,
  CheckCircle2,
  Clock3,
  Download,
  LayoutDashboard,
  RefreshCw,
  Terminal
} from "lucide-react";
import { NotificationToast } from "../components/NotificationToast";
import { useActiveToolContext } from "../contexts/ActiveToolContext";
import { useAppServicesContext } from "../contexts/AppServicesContext";
import { useHistoryContext } from "../contexts/HistoryContext";
import { useMessage } from "../contexts/MessageContext";
import { useProviderEditorContext } from "../contexts/ProviderEditorContext";
import { formatHistoryTime } from "../lib/format";
import { themeIcon, themeLabel, type ThemePreference } from "../lib/theme";
import type { ToolStatus } from "../lib/types";

const fallbackToolStatuses: ToolStatus[] = [
  {
    name: "Codex",
    command: "codex",
    available: false,
    version: null,
    detail: null,
    installLabel: "安装",
    installHint: "打开 Codex 安装说明",
    installUrl: "https://developers.openai.com/codex/"
  },
  {
    name: "Claude Code",
    command: "claude",
    available: false,
    version: null,
    detail: null,
    installLabel: "安装",
    installHint: "打开 Claude Code 安装说明",
    installUrl: "https://docs.anthropic.com/en/docs/claude-code/overview"
  },
  {
    name: "Node.js",
    command: "node",
    available: false,
    version: null,
    detail: null,
    installLabel: "下载",
    installHint: "打开 Node.js 下载页面",
    installUrl: "https://nodejs.org/en/download"
  },
  {
    name: "npm",
    command: "npm",
    available: false,
    version: null,
    detail: null,
    installLabel: "安装",
    installHint: "npm 通常随 Node.js 一起安装",
    installUrl: "https://nodejs.org/en/download"
  }
];

type OverviewPageProps = {
  themePreference: ThemePreference;
  onCycleTheme: () => void;
};

export function OverviewPage({ themePreference, onCycleTheme }: OverviewPageProps) {
  const { message, messageClassName, dismissMessage } = useMessage();
  const { activeTool } = useActiveToolContext();
  const { state, activeProvider, busy, refresh } = useProviderEditorContext();
  const {
    appVersion,
    appUpdate,
    checkingAppUpdate,
    openingAppUpdate,
    openUpdatePage,
    refreshAppUpdate,
    toolStatuses,
    toolStatusesLoading,
    openingTerminal,
    installingTool,
    refreshToolStatuses,
    openInTerminal,
    openToolInstaller
  } = useAppServicesContext();
  const {
    historyLoading,
    historyProviderStats,
    topHistoryProviderStats,
    historySessionCount,
    historyMessageCount,
    latestHistorySession,
    refreshHistory
  } = useHistoryContext();

  const latestVersionLabel = appUpdate?.latestVersion ? `v${appUpdate.latestVersion}` : null;
  const activeToolName = activeTool === "claude" ? "Claude" : "Codex";

  return (
    <section className="overview-board">
      <header className="board-head">
        <div className="panel-title">
          <LayoutDashboard size={18} />
          <div>
            <h3>概览</h3>
            <p>{activeToolName} 环境、当前模型和历史记录摘要</p>
          </div>
        </div>

        <div className="board-actions">
          <button
            className="soft-button toolbar-icon-button"
            type="button"
            data-tooltip="切换主题"
            data-tooltip-placement="left"
            aria-label={`切换主题，当前为${themeLabel(themePreference)}`}
            onClick={onCycleTheme}
          >
            {themeIcon(themePreference)}
          </button>
          <button
            className="soft-button toolbar-icon-button"
            type="button"
            data-tooltip="刷新状态"
            data-tooltip-placement="left"
            aria-label="刷新状态"
            onClick={() => {
              void refresh();
              void refreshToolStatuses();
              void refreshHistory();
              void refreshAppUpdate();
            }}
            disabled={busy || historyLoading || toolStatusesLoading}
          >
            <RefreshCw size={17} />
          </button>
        </div>
      </header>

      <NotificationToast message={message} messageClassName={messageClassName} onDismiss={dismissMessage} />

      <div className="overview-layout">
        <section className="overview-hero">
          <div className="overview-current-model">
            <span>当前设置模型</span>
            <strong>{state?.activeModel || activeProvider?.model || "未设置"}</strong>
            <p>
              {activeProvider
                ? `${activeProvider.name || activeProvider.id} · ${activeProvider.baseUrl || "Base URL 未设置"}`
                : "还没有选择当前 provider"}
            </p>
          </div>

          <div className="overview-status-grid">
            <article className={`overview-status-card ${appUpdate?.updateAvailable ? "has-update" : ""}`}>
              <span>当前版本</span>
              <strong>v{appVersion}</strong>
              <em>
                {checkingAppUpdate
                  ? "检查更新中"
                  : appUpdate?.updateAvailable && latestVersionLabel
                    ? `发现 ${latestVersionLabel}`
                    : appUpdate?.detail
                      ? appUpdate.detail
                      : latestVersionLabel
                        ? "已是最新"
                        : "CodeSail"}
              </em>
              {appUpdate?.updateAvailable ? (
                <button
                  className="version-update-button"
                  type="button"
                  onClick={() => void openUpdatePage()}
                  disabled={openingAppUpdate}
                >
                  <Download size={14} />
                  {openingAppUpdate ? "打开中" : "更新"}
                </button>
              ) : null}
            </article>
          </div>
        </section>

        <section className="overview-section">
          <div className="overview-section-head">
            <div>
              <h4>运行环境</h4>
              <p>{activeToolName}、Node.js、npm 当前状态和版本</p>
            </div>
            <button
              className="row-button overview-section-action"
              type="button"
              data-tooltip={openingTerminal ? `正在打开 ${activeToolName}` : `在终端打开 ${activeToolName}`}
              data-tooltip-placement="left"
              aria-label={`在终端打开 ${activeToolName}`}
              onClick={() => void openInTerminal(activeTool)}
              disabled={openingTerminal}
            >
              <Terminal size={15} />
            </button>
          </div>

          <div className="tool-status-list">
            {(toolStatuses.length ? toolStatuses : fallbackToolStatuses).map((tool) => (
              <article className="tool-status-row" key={tool.command}>
                <span className={`tool-status-icon ${tool.available ? "available" : ""}`}>
                  {tool.available ? <CheckCircle2 size={16} /> : <AlertCircle size={16} />}
                </span>
                <div>
                  <strong>{tool.name}</strong>
                  <em>{toolStatusesLoading ? "检测中" : tool.available ? "可用" : tool.detail || "未检测到"}</em>
                </div>
                {tool.available ? (
                  <code>{tool.version || "--"}</code>
                ) : (
                  <button
                    className="tool-install-button"
                    type="button"
                    data-tooltip={tool.installHint}
                    data-tooltip-placement="left"
                    aria-label={`${tool.installLabel} ${tool.name}`}
                    onClick={() => void openToolInstaller(tool)}
                    disabled={installingTool === tool.command}
                  >
                    <Download size={14} />
                    {installingTool === tool.command ? "打开中" : tool.installLabel}
                  </button>
                )}
              </article>
            ))}
          </div>
        </section>

        <section className="overview-section">
          <div className="overview-section-head">
            <div>
              <h4>历史记录摘要</h4>
              <p>{historyLoading ? "正在扫描会话" : `${activeToolName} · ${historyProviderStats.length} 个分组有历史记录`}</p>
            </div>
            <Clock3 size={18} />
          </div>

          <div className="history-summary-grid">
            <article>
              <span>总会话数</span>
              <strong>{historySessionCount}</strong>
            </article>
            <article>
              <span>消息总数</span>
              <strong>{historyMessageCount}</strong>
            </article>
            <article>
              <span>最近一次会话时间</span>
              <strong>{latestHistorySession ? formatHistoryTime(latestHistorySession.timestamp) : "暂无"}</strong>
            </article>
          </div>

          <div className="provider-summary-list">
            {topHistoryProviderStats.length ? (
              topHistoryProviderStats.map((item) => (
                <div className="provider-summary-row" key={item.provider}>
                  <span>{item.provider}</span>
                  <strong>{item.sessionCount}</strong>
                </div>
              ))
            ) : (
              <div className="overview-empty-line">暂无 provider 会话</div>
            )}
          </div>
        </section>
      </div>
    </section>
  );
}
