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
import { NotificationToast } from "../components/NotificationToast";
import { useHistoryContext } from "../contexts/HistoryContext";
import { useMessage } from "../contexts/MessageContext";
import { formatHistoryTime, roleClass, roleLabel } from "../lib/format";

export function HistoryPage() {
  const { message, messageClassName } = useMessage();
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

  return (
    <section className="history-board">
      <header className="board-head">
        <div className="panel-title">
          <History size={18} />
          <div>
            <h3>历史记录</h3>
            <p>共 {historySessionCount} 条会话</p>
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

      <NotificationToast message={message} messageClassName={messageClassName} />

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
                      onClick={() => void removeHistoryProvider(group)}
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
              <span>~/.codex/sessions</span>
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
                    onClick={() => void removeHistorySession(selectedHistorySession)}
                    disabled={historyLoading || historyBusy}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              </header>

              <div className="conversation-list">
                {historyConversation.messages.length ? (
                  historyConversation.messages.map((item) => (
                    <article className={`conversation-message ${roleClass(item.role)}`} key={`${item.role}-${item.content.length}-${item.content.slice(0, 40)}`}>
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
