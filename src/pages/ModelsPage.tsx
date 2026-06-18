import { type PointerEvent, type SVGProps, useEffect, useRef, useState } from "react";
import {
  ChevronDown,
  Eye,
  EyeOff,
  FileCog,
  Plus,
  Power,
  RefreshCw,
  Save,
  Settings2,
  Trash2,
  X
} from "lucide-react";
import { NotificationToast } from "../components/NotificationToast";
import { ProviderRow } from "../components/ProviderRow";
import { useActiveToolContext } from "../contexts/ActiveToolContext";
import { useMessage } from "../contexts/MessageContext";
import { useProviderEditorContext } from "../contexts/ProviderEditorContext";
import type { ProviderView, ToolType } from "../lib/types";

type ToolIconProps = SVGProps<SVGSVGElement> & { title?: string };
type DragOverPlacement = "before" | "after";
type DragOverTarget = { providerId: string; placement: DragOverPlacement } | null;
type DragRowRect = { providerId: string; top: number; midpoint: number };

function CodexLogoIcon({ title, ...props }: ToolIconProps) {
  return (
    <svg viewBox="0 0 32 32" role={title ? "img" : "presentation"} aria-hidden={title ? undefined : true} {...props}>
      {title ? <title>{title}</title> : null}
      <defs>
        <linearGradient id="codex-cloud-gradient" x1="8" y1="7" x2="24" y2="26" gradientUnits="userSpaceOnUse">
          <stop offset="0" stopColor="#c59cff" />
          <stop offset="0.48" stopColor="#5e7cff" />
          <stop offset="1" stopColor="#172eff" />
        </linearGradient>
      </defs>
      <rect x="1.5" y="1.5" width="29" height="29" rx="7.5" fill="#fffefa" />
      <path
        d="M10.15 24.25c-3.25-.25-5.8-2.93-5.8-6.25 0-3.02 2.15-5.55 5.02-6.13C10.52 8.28 13.88 5.7 17.8 5.7c4.18 0 7.7 2.93 8.58 6.83 2.42.8 4.12 3.07 4.12 5.77 0 3.37-2.72 6.1-6.08 6.1H10.15z"
        fill="url(#codex-cloud-gradient)"
      />
      <path
        d="M11.35 13.05l2.9 4.02-2.9 4.03M18.1 17.08h4.85"
        fill="none"
        stroke="#fffefa"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2.35"
      />
    </svg>
  );
}

function ClaudeLogoIcon({ title, ...props }: ToolIconProps) {
  return (
    <svg viewBox="0 0 32 32" role={title ? "img" : "presentation"} aria-hidden={title ? undefined : true} {...props}>
      {title ? <title>{title}</title> : null}
      <rect width="32" height="32" rx="8" fill="#d97735" />
      <path
        d="M16 5.5l1.86 7.22 5.64-4.94-3.03 6.82 7.43-.73-6.36 3.9 6.36 3.9-7.43-.72 3.03 6.81-5.64-4.94L16 30l-1.86-7.18-5.64 4.94 3.03-6.81-7.43.72 6.36-3.9-6.36-3.9 7.43.73L8.5 7.78l5.64 4.94L16 5.5z"
        fill="#fff7ed"
      />
      <circle cx="16" cy="17.77" r="2.72" fill="#d97735" />
    </svg>
  );
}

const toolOptions: Array<{ value: ToolType; label: string; icon: (props: ToolIconProps) => JSX.Element }> = [
  { value: "codex", label: "codex", icon: CodexLogoIcon },
  { value: "claude", label: "Claude Code", icon: ClaudeLogoIcon }
];

export function ModelsPage() {
  const { message, messageClassName } = useMessage();
  const { activeTool, switching: toolSwitching, switchTool } = useActiveToolContext();
  const [toolDropdownOpen, setToolDropdownOpen] = useState(false);
  const [showImportPrompt, setShowImportPrompt] = useState(false);
  const [dismissedImportPrompt, setDismissedImportPrompt] = useState(false);
  const [draggingProviderId, setDraggingProviderId] = useState<string | null>(null);
  const [dragOverTarget, setDragOverTarget] = useState<DragOverTarget>(null);
  const toolDropdownRef = useRef<HTMLDivElement>(null);
  const providersRef = useRef<ProviderView[]>([]);
  const draggingProviderIdRef = useRef<string | null>(null);
  const dragOverTargetRef = useRef<DragOverTarget>(null);
  const dragRowRectsRef = useRef<DragRowRect[]>([]);
  const canDragProvidersRef = useRef(false);
  const {
    state,
    selected,
    selectedId,
    draft,
    models,
    modelValue,
    providerCount,
    editorOpen,
    busy,
    importingProviders,
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
    importFromCodexToClaude,
    reorderProviders,
    setCurrentProvider,
    removeProvider,
    closeEditor,
    updateDraft,
    selectModel,
    fetchProviderModels,
    saveCurrentProvider,
    healthCheckResults,
    healthCheckProvider
  } = useProviderEditorContext();

  useEffect(() => {
    if (!toolDropdownOpen) return;
    const handleClickOutside = (event: MouseEvent) => {
      if (toolDropdownRef.current && !toolDropdownRef.current.contains(event.target as Node)) {
        setToolDropdownOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [toolDropdownOpen]);

  useEffect(() => {
    const shouldPrompt =
      activeTool === "claude" &&
      state?.activeTool === "claude" &&
      providerCount === 0 &&
      !editorOpen &&
      !dismissedImportPrompt;

    if (shouldPrompt) {
      setShowImportPrompt(true);
    } else {
      setShowImportPrompt(false);
    }
  }, [activeTool, dismissedImportPrompt, editorOpen, providerCount, state?.activeTool]);

  useEffect(() => {
    if (activeTool !== "claude" || providerCount > 0) {
      setDismissedImportPrompt(false);
    }
  }, [activeTool, providerCount]);

  const currentTool = toolOptions.find((t) => t.value === activeTool) ?? toolOptions[0];
  const CurrentToolIcon = currentTool.icon;
  const providers = state?.providers ?? [];
  const activeProviderId = state?.activeProvider ?? null;
  const activeModel = state?.activeModel ?? "";
  const canDragProviders = providers.length > 1 && !busy;

  const toast = <NotificationToast message={message} messageClassName={messageClassName} />;

  useEffect(() => {
    providersRef.current = providers;
  }, [providers]);

  useEffect(() => {
    canDragProvidersRef.current = canDragProviders;
  }, [canDragProviders]);

  function providerIds() {
    return providersRef.current.map((provider) => provider.id);
  }

  function moveProviderToTarget(sourceId: string, targetId: string, placement: DragOverPlacement) {
    if (sourceId === targetId) return;

    const nextIds = providerIds();
    const sourceIndex = nextIds.indexOf(sourceId);
    if (sourceIndex < 0) return;

    const remainingIds = nextIds.filter((providerId) => providerId !== sourceId);
    const targetIndex = remainingIds.indexOf(targetId);
    if (targetIndex < 0) return;

    remainingIds.splice(placement === "after" ? targetIndex + 1 : targetIndex, 0, sourceId);
    void reorderProviders(remainingIds);
  }

  function dragTargetFromClientY(clientY: number): DragOverTarget {
    const sourceId = draggingProviderIdRef.current;
    const rows = dragRowRectsRef.current;
    if (!rows.length) return null;

    for (const row of rows) {
      if (clientY <= row.midpoint) {
        return row.providerId === sourceId ? null : { providerId: row.providerId, placement: "before" };
      }
    }

    const lastProviderId = rows[rows.length - 1]?.providerId;
    return lastProviderId !== sourceId
      ? { providerId: lastProviderId, placement: "after" }
      : null;
  }

  function cacheDragRowRects() {
    dragRowRectsRef.current = Array.from(document.querySelectorAll<HTMLElement>("[data-provider-row-id]"))
      .map((row) => {
        const providerId = row.dataset.providerRowId;
        if (!providerId) return null;
        const rect = row.getBoundingClientRect();
        return { providerId, top: rect.top, midpoint: rect.top + rect.height / 2 };
      })
      .filter((row): row is DragRowRect => Boolean(row))
      .sort((left, right) => left.top - right.top);
  }

  function updateDragTarget(clientY: number) {
    const nextTarget = dragTargetFromClientY(clientY);
    dragOverTargetRef.current = nextTarget;
    setDragOverTarget((current) =>
      current?.providerId === nextTarget?.providerId && current?.placement === nextTarget?.placement
        ? current
        : nextTarget
    );
  }

  function finishProviderPointerDrag() {
    const sourceId = draggingProviderIdRef.current;
    const target = dragOverTargetRef.current;

    draggingProviderIdRef.current = null;
    dragOverTargetRef.current = null;
    dragRowRectsRef.current = [];
    setDraggingProviderId(null);
    setDragOverTarget(null);

    if (!sourceId || !target || !canDragProvidersRef.current) return;
    moveProviderToTarget(sourceId, target.providerId, target.placement);
  }

  function handleProviderPointerDown(event: PointerEvent<HTMLElement>, providerId: string) {
    if (!canDragProviders || event.button !== 0) return;

    const target = event.target;
    if (
      target instanceof HTMLElement &&
      target.closest(".row-actions, .config-row-tools")
    ) {
      return;
    }

    event.preventDefault();
    event.currentTarget.setPointerCapture?.(event.pointerId);
    cacheDragRowRects();
    draggingProviderIdRef.current = providerId;
    setDraggingProviderId(providerId);
    setDragOverTarget(null);
    updateDragTarget(event.clientY);
  }

  useEffect(() => {
    if (!draggingProviderId) return;

    function handlePointerMove(event: globalThis.PointerEvent) {
      updateDragTarget(event.clientY);
    }

    function handlePointerUp() {
      finishProviderPointerDrag();
    }

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
    window.addEventListener("pointercancel", handlePointerUp, { once: true });
    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draggingProviderId]);

  return (
    <div className="models-page">
      {showImportPrompt ? (
        <div className="confirm-overlay" role="presentation">
          <section className="confirm-dialog" role="dialog" aria-modal="true" aria-labelledby="import-codex-title">
            <div className="confirm-dialog-copy">
              <strong id="import-codex-title">导入 codex 配置？</strong>
              <span>Claude 当前没有模型配置，可以从 codex 配置复制一份过来。</span>
            </div>
            <div className="confirm-dialog-actions">
              <button
                className="soft-button"
                type="button"
                onClick={() => {
                  setDismissedImportPrompt(true);
                  setShowImportPrompt(false);
                }}
                disabled={importingProviders}
              >
                取消
              </button>
              <button
                className="primary-button"
                type="button"
                onClick={async () => {
                  await importFromCodexToClaude();
                  setDismissedImportPrompt(false);
                  setShowImportPrompt(false);
                }}
                disabled={importingProviders}
              >
                {importingProviders ? "导入中" : "导入"}
              </button>
            </div>
          </section>
        </div>
      ) : null}

      {!editorOpen ? (
        <section className="configs-board">
          <header className="board-head">
            <div className="panel-title">
              <Settings2 size={18} />
              <div>
                <h3>模型配置</h3>
                <p>当前有 {providerCount} 条模型配置</p>
              </div>
            </div>

            <div className="board-actions">
              <div className="tool-dropdown" ref={toolDropdownRef}>
                <button
                  className={`tool-dropdown-trigger ${toolDropdownOpen ? "open" : ""}`}
                  type="button"
                  data-tooltip={currentTool.label}
                  data-tooltip-placement="left"
                  aria-label={currentTool.label}
                  aria-haspopup="listbox"
                  aria-expanded={toolDropdownOpen}
                  onClick={() => setToolDropdownOpen((open) => !open)}
                  disabled={toolSwitching}
                >
                  <CurrentToolIcon className="tool-brand-icon" />
                </button>
                {toolDropdownOpen ? (
                  <div className="tool-dropdown-menu" role="listbox" aria-label="工具选择">
                    {toolOptions.map((option) => {
                      const OptionIcon = option.icon;
                      const isActive = option.value === activeTool;
                      return (
                        <button
                          key={option.value}
                          className={`tool-dropdown-item ${isActive ? "active" : ""}`}
                          type="button"
                          data-tooltip={option.label}
                          data-tooltip-placement="left"
                          aria-label={option.label}
                          role="option"
                          aria-selected={isActive}
                          onClick={() => {
                            setToolDropdownOpen(false);
                            if (!isActive) {
                              void switchTool(option.value);
                            }
                          }}
                        >
                          <OptionIcon className="tool-brand-icon" />
                        </button>
                      );
                    })}
                  </div>
                ) : null}
              </div>

              <label
                className={`config-sync-option icon-sync-option ${updateConfigFile ? "enabled" : ""}`}
                data-tooltip={updateConfigFile ? "更新配置文件：开" : "更新配置文件：关"}
                data-tooltip-placement="left"
              >
                <FileCog size={17} aria-hidden="true" />
                <input
                  aria-label="更新配置文件"
                  checked={updateConfigFile}
                  role="switch"
                  type="checkbox"
                  onChange={(event) => setUpdateConfigFile(event.target.checked)}
                />
                <span className="config-sync-switch" aria-hidden="true">
                  <span />
                </span>
              </label>

              <button
                className="soft-button toolbar-icon-button restart-codex-button"
                type="button"
                data-tooltip={restarting ? "正在重启 Codex" : "重启 Codex"}
                data-tooltip-placement="left"
                aria-label="重启 Codex"
                onClick={() => void restartCodex()}
                disabled={busy || restarting}
              >
                <Power size={17} />
              </button>
              <button
                className="soft-button toolbar-icon-button"
                type="button"
                data-tooltip="刷新"
                data-tooltip-placement="left"
                aria-label="刷新"
                onClick={() => void refresh()}
                disabled={busy}
              >
                <RefreshCw size={17} />
              </button>
              <button
                className="primary-button toolbar-icon-button"
                type="button"
                data-tooltip="新增配置"
                data-tooltip-placement="left"
                aria-label="新增配置"
                onClick={openCreateProvider}
                disabled={busy}
              >
                <Plus size={17} />
              </button>
            </div>
          </header>

          {toast}

          <div className="config-list" role="list">
            {providers.length ? (
              providers.map((provider) => (
                <ProviderRow
                  key={provider.id}
                  provider={provider}
                  active={provider.id === activeProviderId}
                  activeModel={
                    provider.id === activeProviderId
                      ? activeModel || provider.model || ""
                      : provider.model || ""
                  }
                  selected={editorOpen && provider.id === selectedId}
                  busy={busy}
                  dragging={draggingProviderId === provider.id}
                  dragOverPlacement={
                    dragOverTarget?.providerId === provider.id ? dragOverTarget.placement : null
                  }
                  healthStatus={healthCheckResults[provider.id]}
                  onPointerDown={(event) => handleProviderPointerDown(event, provider.id)}
                  onEdit={() => void openEditProvider(provider)}
                  onCopy={() => void copyProvider(provider.id)}
                  onSetCurrent={() => void setCurrentProvider(provider)}
                  onHealthCheck={() => void healthCheckProvider(provider)}
                  onDelete={() => void removeProvider(provider.id)}
                />
              ))
            ) : (
              <div className="empty-configs">
                <Settings2 size={24} />
                <strong>还没有模型配置</strong>
                <span>点击新增配置开始填写。</span>
              </div>
            )}
          </div>
        </section>
      ) : (
        <section className="config-editor">
          <header className="editor-head">
            <div className="panel-title">
              <Settings2 size={18} />
              <div>
                <h3>{selected ? "修改配置" : "新增配置"}</h3>
                <p>{canSave ? "有未保存修改" : "配置已同步"}</p>
              </div>
            </div>

            <div className="editor-actions">
              <button className="soft-button" type="button" onClick={closeEditor} disabled={busy}>
                <X size={17} />
                返回列表
              </button>
            </div>
          </header>

          {toast}

          {loadingModels ? (
            <div className="loading-mask" data-tauri-drag-region="false" role="status" aria-live="polite">
              <div className="loading-card">
                <RefreshCw size={24} />
                <strong>正在获取模型列表</strong>
                <span>请求返回前请稍候</span>
              </div>
            </div>
          ) : null}

          <div className="editor-body">
            <div className="field-grid">
              <label className="wide">
                <span>Name</span>
                <input
                  value={draft.name}
                  onChange={(event) => updateDraft({ name: event.target.value })}
                  placeholder="显示名称"
                />
              </label>
              <label className="wide">
                <span>Base URL</span>
                <input
                  value={draft.baseUrl}
                  onChange={(event) => updateDraft({ baseUrl: event.target.value })}
                  placeholder="https://example.com/v1"
                />
              </label>
              <label className="wide">
                <span>Token</span>
                <div className="secret-input-wrap">
                  <input
                    value={draft.token}
                    type={tokenVisible ? "text" : "password"}
                    onChange={(event) => updateDraft({ token: event.target.value })}
                    placeholder="sk-..."
                  />
                  <button
                    className="secret-toggle"
                    type="button"
                    data-tooltip={tokenVisible ? "隐藏 Token" : "显示 Token"}
                    data-tooltip-placement="left"
                    aria-label={tokenVisible ? "隐藏 Token" : "显示 Token"}
                    onClick={toggleTokenVisible}
                  >
                    {tokenVisible ? <EyeOff size={17} /> : <Eye size={17} />}
                  </button>
                </div>
              </label>
              <div className="field-group wide">
                <span>Model</span>
                <div className="model-field-stack">
                  <div className="model-combobox">
                    <div
                      className="model-input-wrap"
                      ref={modelComboboxRef}
                      onBlur={(event) => {
                        if (!event.currentTarget.contains(event.relatedTarget)) {
                          setModelMenuOpen(false);
                        }
                      }}
                    >
                      <input
                        value={modelValue}
                        role="combobox"
                        aria-expanded={modelMenuOpen}
                        aria-controls="model-options"
                        onFocus={() => setModelMenuOpen(models.length > 0)}
                        onClick={() => setModelMenuOpen(models.length > 0)}
                        onChange={(event) => {
                          const nextModel = event.target.value;
                          setModelValue(nextModel);
                          updateDraft({ model: nextModel });
                          setModelMenuOpen(models.length > 0);
                        }}
                        onKeyDown={(event) => {
                          if (event.key === "Escape") {
                            setModelMenuOpen(false);
                          }
                        }}
                        placeholder="选择模型或手动填写"
                      />
                      <button
                        className={`model-menu-toggle ${modelMenuOpen ? "open" : ""}`}
                        type="button"
                        data-tooltip="展开模型列表"
                        data-tooltip-placement="left"
                        aria-label="展开模型列表"
                        disabled={!models.length}
                        onMouseDown={(event) => event.preventDefault()}
                        onClick={() => setModelMenuOpen((open) => (models.length ? !open : false))}
                      >
                        <ChevronDown size={17} />
                      </button>
                      {modelMenuOpen && models.length ? (
                        <div className="model-menu" id="model-options" role="listbox" aria-label="模型列表">
                          {models.map((model) => (
                            <button
                              key={model}
                              className={`model-option ${model === modelValue ? "selected" : ""}`}
                              type="button"
                              role="option"
                              aria-selected={model === modelValue}
                              onMouseDown={(event) => {
                                event.preventDefault();
                                event.stopPropagation();
                                selectModel(model);
                              }}
                              onClick={(event) => {
                                event.preventDefault();
                                event.stopPropagation();
                                selectModel(model);
                              }}
                            >
                              {model}
                            </button>
                          ))}
                        </div>
                      ) : null}
                    </div>
                    <button
                      className="fetch-button"
                      type="button"
                      onClick={() => void fetchProviderModels()}
                      disabled={loadingModels || busy}
                    >
                      <RefreshCw size={17} />
                      {loadingModels ? "获取中" : "获取模型"}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <footer className="editor-foot">
            {selected ? (
              <button
                className="danger-button"
                type="button"
                onClick={() => void removeProvider(selected.id)}
                disabled={busy}
              >
                <Trash2 size={17} />
                删除
              </button>
            ) : (
              <span />
            )}

            <button className="primary-button" type="button" onClick={() => void saveCurrentProvider()} disabled={busy || !canSave}>
              <Save size={17} />
              保存
            </button>
          </footer>
        </section>
      )}
    </div>
  );
}
