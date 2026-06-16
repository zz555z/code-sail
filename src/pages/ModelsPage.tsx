import type { RefObject } from "react";
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
import type { AppState, ProviderDraft, ProviderView } from "../lib/types";

type ModelsPageProps = {
  state: AppState | null;
  selected: ProviderView | null;
  selectedId: string | null;
  draft: ProviderDraft;
  models: string[];
  modelValue: string;
  providerCount: number;
  message: string;
  messageClassName: string;
  editorOpen: boolean;
  busy: boolean;
  restarting: boolean;
  loadingModels: boolean;
  modelMenuOpen: boolean;
  tokenVisible: boolean;
  updateConfigFile: boolean;
  canSave: boolean;
  modelComboboxRef: RefObject<HTMLDivElement>;
  onSetUpdateConfigFile: (enabled: boolean) => void;
  onRestartCodex: () => void;
  onRefresh: () => void;
  onCreateProvider: () => void;
  onEditProvider: (provider: ProviderView) => void;
  onCopyProvider: (providerId: string) => void;
  onSetCurrentProvider: (provider: ProviderView) => void;
  onDeleteProvider: (providerId: string) => void;
  onCloseEditor: () => void;
  onUpdateDraft: (patch: Partial<ProviderDraft>) => void;
  onToggleTokenVisible: () => void;
  onSetModelMenuOpen: (open: boolean | ((current: boolean) => boolean)) => void;
  onSetModelValue: (model: string) => void;
  onSelectModel: (model: string) => void;
  onFetchModels: () => void;
  onSave: () => void;
};

export function ModelsPage({
  state,
  selected,
  selectedId,
  draft,
  models,
  modelValue,
  providerCount,
  message,
  messageClassName,
  editorOpen,
  busy,
  restarting,
  loadingModels,
  modelMenuOpen,
  tokenVisible,
  updateConfigFile,
  canSave,
  modelComboboxRef,
  onSetUpdateConfigFile,
  onRestartCodex,
  onRefresh,
  onCreateProvider,
  onEditProvider,
  onCopyProvider,
  onSetCurrentProvider,
  onDeleteProvider,
  onCloseEditor,
  onUpdateDraft,
  onToggleTokenVisible,
  onSetModelMenuOpen,
  onSetModelValue,
  onSelectModel,
  onFetchModels,
  onSave
}: ModelsPageProps) {
  const toast = <NotificationToast message={message} messageClassName={messageClassName} />;

  return (
    <div className="models-page">
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
                  onChange={(event) => onSetUpdateConfigFile(event.target.checked)}
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
                onClick={onRestartCodex}
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
                onClick={onRefresh}
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
                onClick={onCreateProvider}
                disabled={busy}
              >
                <Plus size={17} />
              </button>
            </div>
          </header>

          {toast}

          <div className="config-list">
            {state?.providers.length ? (
              state.providers.map((provider) => (
                <ProviderRow
                  key={provider.id}
                  provider={provider}
                  active={provider.id === state.activeProvider}
                  activeModel={
                    provider.id === state.activeProvider
                      ? state.activeModel || provider.model || ""
                      : provider.model || ""
                  }
                  selected={editorOpen && provider.id === selectedId}
                  busy={busy}
                  onEdit={() => onEditProvider(provider)}
                  onCopy={() => onCopyProvider(provider.id)}
                  onSetCurrent={() => onSetCurrentProvider(provider)}
                  onDelete={() => onDeleteProvider(provider.id)}
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
              <button className="soft-button" type="button" onClick={onCloseEditor} disabled={busy}>
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
                  onChange={(event) => onUpdateDraft({ name: event.target.value })}
                  placeholder="显示名称"
                />
              </label>
              <label className="wide">
                <span>Base URL</span>
                <input
                  value={draft.baseUrl}
                  onChange={(event) => onUpdateDraft({ baseUrl: event.target.value })}
                  placeholder="https://example.com/v1"
                />
              </label>
              <label className="wide">
                <span>Token</span>
                <div className="secret-input-wrap">
                  <input
                    value={draft.token}
                    type={tokenVisible ? "text" : "password"}
                    onChange={(event) => onUpdateDraft({ token: event.target.value })}
                    placeholder="sk-..."
                  />
                  <button
                    className="secret-toggle"
                    type="button"
                    data-tooltip={tokenVisible ? "隐藏 Token" : "显示 Token"}
                    data-tooltip-placement="left"
                    aria-label={tokenVisible ? "隐藏 Token" : "显示 Token"}
                    onClick={onToggleTokenVisible}
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
                          onSetModelMenuOpen(false);
                        }
                      }}
                    >
                      <input
                        value={modelValue}
                        role="combobox"
                        aria-expanded={modelMenuOpen}
                        aria-controls="model-options"
                        onFocus={() => onSetModelMenuOpen(models.length > 0)}
                        onClick={() => onSetModelMenuOpen(models.length > 0)}
                        onChange={(event) => {
                          const nextModel = event.target.value;
                          onSetModelValue(nextModel);
                          onUpdateDraft({ model: nextModel });
                          onSetModelMenuOpen(models.length > 0);
                        }}
                        onKeyDown={(event) => {
                          if (event.key === "Escape") {
                            onSetModelMenuOpen(false);
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
                        onClick={() => onSetModelMenuOpen((open) => (models.length ? !open : false))}
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
                                onSelectModel(model);
                              }}
                              onClick={(event) => {
                                event.preventDefault();
                                event.stopPropagation();
                                onSelectModel(model);
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
                      onClick={onFetchModels}
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
                onClick={() => onDeleteProvider(selected.id)}
                disabled={busy}
              >
                <Trash2 size={17} />
                删除
              </button>
            ) : (
              <span />
            )}

            <button className="primary-button" type="button" onClick={onSave} disabled={busy || !canSave}>
              <Save size={17} />
              保存
            </button>
          </footer>
        </section>
      )}
    </div>
  );
}
