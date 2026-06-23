import { type SVGProps, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ChevronDown,
  Eye,
  EyeOff,
  FileCog,
  Plus,
  Power,
  RefreshCw,
  Save,
  SlidersHorizontal,
  Settings2,
  Trash2,
  X
} from "lucide-react";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ModelCombobox } from "../components/ModelCombobox";
import { NotificationToast } from "../components/NotificationToast";
import { ProviderRow } from "../components/ProviderRow";
import { useActiveToolContext } from "../contexts/ActiveToolContext";
import { useMessage } from "../contexts/MessageContext";
import { useProviderEditorContext } from "../contexts/ProviderEditorContext";
import { useProviderReorder } from "../hooks/useProviderReorder";
import type { ProviderView, ToolType } from "../lib/types";

type ToolIconProps = SVGProps<SVGSVGElement> & { title?: string };

const EMPTY_PROVIDERS: ProviderView[] = [];

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

const DEFAULT_WIRE_API = "responses";

function quotedConfigValue(value: string) {
  return JSON.stringify(value);
}

function providerTableKey(providerId: string) {
  if (/^[A-Za-z0-9_-]+$/.test(providerId)) {
    return providerId;
  }
  return quotedConfigValue(providerId);
}

export function ModelsPage() {
  const { message, messageClassName, dismissMessage } = useMessage();
  const { activeTool, switching: toolSwitching, switchTool } = useActiveToolContext();
  const [toolDropdownOpen, setToolDropdownOpen] = useState(false);
  const [showImportPrompt, setShowImportPrompt] = useState(false);
  const [dismissedImportPrompt, setDismissedImportPrompt] = useState(false);
  const [providerPendingDelete, setProviderPendingDelete] = useState<ProviderView | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const toolDropdownRef = useRef<HTMLDivElement>(null);
  const {
    state,
    selected,
    selectedId,
    draft,
    models,
    modelValue,
    claudeHaikuModel,
    claudeOpusModel,
    claudeSonnetModel,
    providerCount,
    editorOpen,
    busy,
    importingProviders,
    restarting,
    loadingModels,
    modelMenuOpen,
    modelMenuTarget,
    tokenVisible,
    updateConfigFile,
    canSave,
    modelComboboxRef,
    setUpdateConfigFile,
    setModelMenuOpen,
    setModelMenuTarget,
    setModelValue,
    setClaudeHaikuModel,
    setClaudeOpusModel,
    setClaudeSonnetModel,
    openModelMenu,
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
  const providers = state?.providers ?? EMPTY_PROVIDERS;
  const activeProviderId = state?.activeProvider ?? null;
  const activeModel = state?.activeModel ?? "";

  const providerMap = useMemo(() => {
    const map = new Map<string, ProviderView>();
    for (const provider of providers) {
      map.set(provider.id, provider);
    }
    return map;
  }, [providers]);

  const handleEditProvider = useCallback((providerId: string) => {
    const provider = providerMap.get(providerId);
    if (provider) void openEditProvider(provider);
  }, [providerMap, openEditProvider]);

  const handleCopyProvider = useCallback((providerId: string) => {
    void copyProvider(providerId);
  }, [copyProvider]);

  const handleSetCurrentProvider = useCallback((providerId: string) => {
    const provider = providerMap.get(providerId);
    if (provider) void setCurrentProvider(provider);
  }, [providerMap, setCurrentProvider]);

  const handleHealthCheckProvider = useCallback((providerId: string) => {
    const provider = providerMap.get(providerId);
    if (provider) void healthCheckProvider(provider);
  }, [providerMap, healthCheckProvider]);

  const handleDeleteProvider = useCallback((providerId: string) => {
    const provider = providerMap.get(providerId);
    if (provider) setProviderPendingDelete(provider);
  }, [providerMap]);

  useEffect(() => {
    if (!editorOpen) return;
    const firstInput = document.querySelector<HTMLInputElement>(".config-editor .field-grid input");
    firstInput?.focus();
  }, [editorOpen]);

  useEffect(() => {
    setAdvancedOpen(false);
  }, [editorOpen, selectedId]);

  const previewProviderId = draft.originalId || "<auto>";
  const previewName = draft.name.trim() || previewProviderId;
  const previewBaseUrl = draft.baseUrl.trim() || "https://example.com/v1";
  const previewModel = modelValue.trim() || draft.model.trim() || "<model>";
  const previewHaikuModel = claudeHaikuModel.trim() || previewModel;
  const previewOpusModel = claudeOpusModel.trim() || previewModel;
  const previewSonnetModel = claudeSonnetModel.trim() || previewModel;
  const previewToken = draft.token.trim();
  const previewToolType = draft.toolType;
  const previewWireApi = draft.wireApi.trim() || DEFAULT_WIRE_API;
  const previewRequiresOpenaiAuth = draft.requiresOpenaiAuth;

  const configPreview = useMemo(() => {
    const providerId = previewProviderId;

    if (previewToolType === "claude") {
      const tokenEnvKey = previewRequiresOpenaiAuth ? "ANTHROPIC_API_KEY" : "ANTHROPIC_AUTH_TOKEN";
      return JSON.stringify(
        {
          env: {
            [tokenEnvKey]: previewToken ? "<saved token>" : "<token>",
            ANTHROPIC_BASE_URL: previewBaseUrl,
            ANTHROPIC_DEFAULT_HAIKU_MODEL: previewHaikuModel,
            ANTHROPIC_DEFAULT_OPUS_MODEL: previewOpusModel,
            ANTHROPIC_DEFAULT_SONNET_MODEL: previewSonnetModel
          }
        },
        null,
        2
      );
    }

    const lines = updateConfigFile
      ? [
          `model_provider = ${quotedConfigValue(providerId)}`,
          `model = ${quotedConfigValue(previewModel)}`,
          "",
          `[model_providers.${providerTableKey(providerId)}]`,
          `name = ${quotedConfigValue(previewName)}`,
          `wire_api = ${quotedConfigValue(previewWireApi)}`,
          `requires_openai_auth = ${previewRequiresOpenaiAuth ? "true" : "false"}`,
          `base_url = ${quotedConfigValue(previewBaseUrl)}`
        ]
      : [
          "# 当前关闭了“更新配置文件”，保存时只会更新 CodeSail 本地数据。",
          "",
          `[model_providers.${providerTableKey(providerId)}]`,
          `name = ${quotedConfigValue(previewName)}`,
          `wire_api = ${quotedConfigValue(previewWireApi)}`,
          `requires_openai_auth = ${previewRequiresOpenaiAuth ? "true" : "false"}`,
          `base_url = ${quotedConfigValue(previewBaseUrl)}`
        ];

    return lines.join("\n");
  }, [
    previewBaseUrl,
    previewModel,
    previewHaikuModel,
    previewOpusModel,
    previewSonnetModel,
    previewName,
    previewProviderId,
    previewRequiresOpenaiAuth,
    previewToken,
    previewToolType,
    previewWireApi,
    updateConfigFile
  ]);

  const { draggingProviderId, dragOverTarget, handleProviderPointerDown } = useProviderReorder({
    providers,
    busy,
    reorderProviders
  });

  const toast = <NotificationToast message={message} messageClassName={messageClassName} onDismiss={dismissMessage} />;

  return (
    <div className="models-page">
      <ConfirmDialog
        open={showImportPrompt}
        title="导入 codex 配置？"
        description="Claude 当前没有模型配置，可以从 codex 配置复制一份过来。"
        confirmLabel={importingProviders ? "导入中" : "导入"}
        busy={importingProviders}
        onCancel={() => {
          setDismissedImportPrompt(true);
          setShowImportPrompt(false);
        }}
        onConfirm={async () => {
          await importFromCodexToClaude();
          setDismissedImportPrompt(false);
          setShowImportPrompt(false);
        }}
      />

      <ConfirmDialog
        open={Boolean(providerPendingDelete)}
        title="删除配置？"
        description={`将删除 ${providerPendingDelete?.name || providerPendingDelete?.id || "该配置"} 及其保存的模型列表和 token。`}
        confirmLabel={busy ? "删除中" : "删除"}
        danger
        busy={busy}
        onCancel={() => setProviderPendingDelete(null)}
        onConfirm={async () => {
          const provider = providerPendingDelete;
          if (!provider) return;
          await removeProvider(provider.id);
          setProviderPendingDelete(null);
        }}
      />

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
              <div className="tool-dropdown" ref={toolDropdownRef}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    setToolDropdownOpen(false);
                    return;
                  }
                  if (!toolDropdownOpen) return;
                  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                    event.preventDefault();
                    const items = toolDropdownRef.current?.querySelectorAll<HTMLButtonElement>(".tool-dropdown-item");
                    if (!items?.length) return;
                    const current = Array.from(items).findIndex((el) => el === document.activeElement);
                    const next = event.key === "ArrowDown"
                      ? (current + 1) % items.length
                      : (current - 1 + items.length) % items.length;
                    items[next]?.focus();
                    return;
                  }
                  if (event.key === "Enter" && document.activeElement instanceof HTMLButtonElement) {
                    document.activeElement.click();
                  }
                }}
              >
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

              {activeTool === "codex" ? (
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
              ) : null}
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

          <div className="config-list" role="list" aria-label="模型配置列表">
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
                  onEdit={() => handleEditProvider(provider.id)}
                  onCopy={() => handleCopyProvider(provider.id)}
                  onSetCurrent={() => handleSetCurrentProvider(provider.id)}
                  onHealthCheck={() => handleHealthCheckProvider(provider.id)}
                  onDelete={() => handleDeleteProvider(provider.id)}
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
                  placeholder="请求模型的地址，根据第三方模型请求地址填写"
                />
              </label>
              <label className="wide">
                <span>Token</span>
                <div className="secret-input-wrap">
                  <input
                    value={draft.token}
                    type={tokenVisible ? "text" : "password"}
                    onChange={(event) => updateDraft({ token: event.target.value })}
                    placeholder="sk-...密钥"
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
              {draft.toolType === "claude" ? (
                <div className="field-group wide">
                  <div className="claude-model-header">
                    <span>Model</span>
                    <button
                      className="fetch-button"
                      type="button"
                      onClick={() => void fetchProviderModels()}
                      disabled={loadingModels || busy}
                    >
                      <RefreshCw size={15} />
                      {loadingModels ? "获取中" : "获取模型"}
                    </button>
                  </div>
                  <div className="claude-models-grid">
                    {([
                      { key: "haiku" as const, label: "Haiku", value: claudeHaikuModel, set: setClaudeHaikuModel },
                      { key: "opus" as const, label: "Opus", value: claudeOpusModel, set: setClaudeOpusModel },
                      { key: "sonnet" as const, label: "Sonnet", value: claudeSonnetModel, set: setClaudeSonnetModel }
                    ]).map(({ key, label, value, set }) => (
                      <div key={key} className="claude-model-field">
                        <span className="claude-model-label">{label}</span>
                        <ModelCombobox
                          value={value}
                          models={models}
                          menuOpen={modelMenuOpen && modelMenuTarget === key}
                          menuId={`model-options-${key}`}
                          ariaLabel={`${label} 模型列表`}
                          onChange={set}
                          onSelect={(model) => selectModel(model, key)}
                          onMenuToggle={(open) => {
                            setModelMenuOpen(open);
                            if (open) setModelMenuTarget(key);
                          }}
                        />
                      </div>
                    ))}
                  </div>
                </div>
              ) : (
                <div className="field-group wide">
                  <span>Model</span>
                  <div className="model-field-stack">
                    <ModelCombobox
                      value={modelValue}
                      models={models}
                      menuOpen={modelMenuOpen}
                      containerRef={modelComboboxRef}
                      onChange={(nextModel) => {
                        setModelValue(nextModel);
                        updateDraft({ model: nextModel });
                      }}
                      onSelect={selectModel}
                      onMenuToggle={setModelMenuOpen}
                    />
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
              )}

              <div className="advanced-settings wide">
                <button
                  className={`advanced-toggle ${advancedOpen ? "open" : ""}`}
                  type="button"
                  aria-expanded={advancedOpen}
                  onClick={() => setAdvancedOpen((open) => !open)}
                >
                  <SlidersHorizontal size={17} />
                  <span>高级设置</span>
                  <ChevronDown size={17} />
                </button>

                {advancedOpen ? (
                  <div className="advanced-panel">
                    {draft.toolType === "codex" ? (
                      <div className="advanced-grid">
                        <div className="field-group">
                          <span>认证方式</span>
                          <div
                            className={`auth-segment ${draft.requiresOpenaiAuth ? "auth-openai" : "auth-token"}`}
                            role="group"
                            aria-label="认证方式"
                          >
                            <button
                              className={!draft.requiresOpenaiAuth ? "active" : ""}
                              type="button"
                              onClick={() => updateDraft({ requiresOpenaiAuth: false })}
                            >
                              Token
                            </button>
                            <button
                              className={draft.requiresOpenaiAuth ? "active" : ""}
                              type="button"
                              onClick={() => updateDraft({ requiresOpenaiAuth: true })}
                            >
                              OpenAI 登录
                            </button>
                          </div>
                        </div>
                      </div>
                    ) : null}

                    {draft.toolType === "claude" ? (
                      <div className="advanced-grid">
                        <div className="field-group">
                          <span>认证方式</span>
                          <div
                            className={`auth-segment ${draft.requiresOpenaiAuth ? "auth-openai" : "auth-token"}`}
                            role="group"
                            aria-label="认证方式"
                          >
                            <button
                              className={!draft.requiresOpenaiAuth ? "active" : ""}
                              type="button"
                              onClick={() => updateDraft({ requiresOpenaiAuth: false })}
                            >
                              Bearer Token
                            </button>
                            <button
                              className={draft.requiresOpenaiAuth ? "active" : ""}
                              type="button"
                              onClick={() => updateDraft({ requiresOpenaiAuth: true })}
                            >
                              API Key
                            </button>
                          </div>
                        </div>
                      </div>
                    ) : null}

                    <div className="config-preview">
                      <span>配置预览</span>
                      <pre>{configPreview}</pre>
                    </div>
                  </div>
                ) : null}
              </div>
            </div>
          </div>

          <footer className="editor-foot">
            {selected ? (
              <button
                className="danger-button"
                type="button"
                onClick={() => setProviderPendingDelete(selected)}
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
