import { useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  FolderOpen,
  History,
  MessageSquareText,
  Play,
  RefreshCw,
  Trash2
} from "lucide-react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { NotificationToast } from "../components/NotificationToast";
import { useActiveToolContext } from "../contexts/ActiveToolContext";
import { useHistoryContext } from "../contexts/HistoryContext";
import { useMessage } from "../contexts/MessageContext";
import { formatHistoryTime, roleClass, roleLabel } from "../lib/format";
import type { HistoryProviderGroup, HistorySessionSummary } from "../lib/types";

export function HistoryPage() {
  const { message, messageClassName, dismissMessage } = useMessage();
  const { activeTool } = useActiveToolContext();
  const {
    historySessionCount,
    historyGroups,
    historyConversation,
    selectedHistoryPath,
    selectedHistorySession,
    expandedHistoryProviders,
    historyLoading,
    historyBusy,
    refreshHistory,
    toggleHistoryProvider,
    openHistorySession,
    resumeHistory,
    removeHistorySession,
    removeHistoryProvider
  } = useHistoryContext();
  const [sessionPendingDelete, setSessionPendingDelete] = useState<HistorySessionSummary | null>(null);
  const [providerPendingDelete, setProviderPendingDelete] = useState<HistoryProviderGroup | null>(null);
  const activeToolName = activeTool === "claude" ? "Claude" : "Codex";
  const historyRootLabel = activeTool === "claude" ? "~/.claude/projects" : "~/.codex/sessions";

  return (
    <section className="history-board">
      <ConfirmDialog
        open={Boolean(sessionPendingDelete)}
        title="删除会话？"
        description={`将删除会话 ${sessionPendingDelete?.title || sessionPendingDelete?.sessionId || ""}。`}
        confirmLabel={historyBusy ? "删除中" : "删除"}
        danger
        busy={historyBusy}
        onCancel={() => setSessionPendingDelete(null)}
        onConfirm={async () => {
          const session = sessionPendingDelete;
          if (!session) return;
          await removeHistorySession(session);
          setSessionPendingDelete(null);
        }}
      />

      <ConfirmDialog
        open={Boolean(providerPendingDelete)}
        title="删除分组历史？"
        description={`将删除 ${providerPendingDelete?.provider || "该分组"} 下的全部历史会话。`}
        confirmLabel={historyBusy ? "删除中" : "删除"}
        danger
        busy={historyBusy}
        onCancel={() => setProviderPendingDelete(null)}
        onConfirm={async () => {
          const group = providerPendingDelete;
          if (!group) return;
          await removeHistoryProvider(group);
          setProviderPendingDelete(null);
        }}
      />

      <header className="board-head">
        <div className="panel-title">
          <History size={18} />
          <div>
            <h3>历史记录</h3>
            <p>{activeToolName} · 共 {historySessionCount} 条会话</p>
          </div>
        </div>

        <div className="board-actions">
          <button
            className="soft-button toolbar-icon-button"
            type="button"
            data-tooltip="刷新历史"
            data-tooltip-placement="left"
            aria-label="刷新历史"
            onClick={() => void refreshHistory()}
            disabled={historyLoading || historyBusy}
          >
            <RefreshCw size={17} />
          </button>
        </div>
      </header>

      <NotificationToast message={message} messageClassName={messageClassName} onDismiss={dismissMessage} />

      <div className="history-layout">
        <aside className="history-provider-list" aria-label="Provider 会话分组">
          {historyGroups.length ? (
            historyGroups.map((group) => {
              const expanded = expandedHistoryProviders[group.provider] ?? true;
              return (
                <section className="history-provider-group" key={group.provider}>
                  <div className="history-provider-head">
                    <button
                      className="history-provider-toggle"
                      type="button"
                      onClick={() => toggleHistoryProvider(group.provider)}
                      aria-expanded={expanded}
                    >
                      {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                      <FolderOpen size={16} />
                      <span>{group.provider}</span>
                      <em>{group.sessions.length}</em>
                    </button>
                    <button
                      className="row-button danger"
                      type="button"
                      data-tooltip="删除该分组历史"
                      data-tooltip-placement="left"
                      aria-label={`删除 ${group.provider} 分组历史`}
                      onClick={() => setProviderPendingDelete(group)}
                      disabled={historyLoading || historyBusy}
                    >
                      <Trash2 size={15} />
                    </button>
                  </div>

                  {expanded ? (
                    <div className="history-session-list">
                      {group.sessions.map((session) => (
                        <button
                          className={`history-session-item ${selectedHistoryPath === session.path ? "selected" : ""}`}
                          key={session.path}
                          type="button"
                          onClick={() => void openHistorySession(session)}
                          disabled={historyLoading || historyBusy}
                        >
                          <span className="history-session-icon">
                            <MessageSquareText size={15} />
                          </span>
                          <span className="history-session-copy">
                            <strong>{session.title}</strong>
                            <span>
                              {formatHistoryTime(session.timestamp)} · {session.messageCount} 条消息
                            </span>
                          </span>
                        </button>
                      ))}
                    </div>
                  ) : null}
                </section>
              );
            })
          ) : (
            <div className="history-empty-state">
              <History size={24} />
              <strong>{historyLoading ? "正在扫描历史记录" : "还没有历史记录"}</strong>
              <span>{historyRootLabel}</span>
            </div>
          )}
        </aside>

        <section className="history-detail-panel" aria-label="会话对话">
          {historyConversation && selectedHistorySession ? (
            <>
              <header className="history-detail-head">
                <div>
                  <h3>{historyConversation.title}</h3>
                  <p>
                    {historyConversation.provider} · {historyConversation.messages.length} 条消息
                  </p>
                </div>
                <div className="row-actions">
                  <button
                    className="row-button set-current"
                    type="button"
                    data-tooltip="恢复会话"
                    data-tooltip-placement="left"
                    aria-label="恢复会话"
                    onClick={() => void resumeHistory(selectedHistorySession)}
                    disabled={historyLoading || historyBusy}
                  >
                    <Play size={15} />
                  </button>
                  <button
                    className="row-button danger"
                    type="button"
                    data-tooltip="删除会话"
                    data-tooltip-placement="left"
                    aria-label="删除会话"
                    onClick={() => setSessionPendingDelete(selectedHistorySession)}
                    disabled={historyLoading || historyBusy}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              </header>

              <div className="conversation-list" role="log" aria-label="会话对话内容">
                {historyConversation.messages.length ? (
                  historyConversation.messages.map((item, index) => (
                    <article className={`conversation-message ${roleClass(item.role)}`} key={`${historyConversation.sessionId}-${index}`}>
                      <span>{roleLabel(item.role)}</span>
                      <p>{item.content}</p>
                    </article>
                  ))
                ) : (
                  <div className="history-empty-state compact">
                    <MessageSquareText size={22} />
                    <strong>暂无可展示消息</strong>
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="history-empty-detail">
              <MessageSquareText size={26} />
              <strong>{historyLoading ? "正在读取会话" : "选择一条会话"}</strong>
            </div>
          )}
        </section>
      </div>
    </section>
  );
}
