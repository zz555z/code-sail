import { useMemo, useState } from "react";
import { ChevronDown, SlidersHorizontal } from "lucide-react";
import type { ProviderDraft } from "../lib/types";

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

type AdvancedSettingsProps = {
  draft: ProviderDraft;
  modelValue: string;
  claudeHaikuModel: string;
  claudeOpusModel: string;
  claudeSonnetModel: string;
  updateConfigFile: boolean;
  onUpdateDraft: (patch: Partial<ProviderDraft>) => void;
};

export function AdvancedSettings({
  draft,
  modelValue,
  claudeHaikuModel,
  claudeOpusModel,
  claudeSonnetModel,
  updateConfigFile,
  onUpdateDraft
}: AdvancedSettingsProps) {
  const [open, setOpen] = useState(false);

  const configPreview = useMemo(() => {
    const providerId = draft.originalId || "<auto>";
    const name = draft.name.trim() || providerId;
    const baseUrl = draft.baseUrl.trim() || "https://example.com/v1";
    const model = modelValue.trim() || draft.model.trim() || "<model>";
    const haiku = claudeHaikuModel.trim() || model;
    const opus = claudeOpusModel.trim() || model;
    const sonnet = claudeSonnetModel.trim() || model;
    const token = draft.token.trim();
    const wireApi = draft.wireApi.trim() || DEFAULT_WIRE_API;

    if (draft.toolType === "claude") {
      const tokenEnvKey = draft.requiresOpenaiAuth ? "ANTHROPIC_API_KEY" : "ANTHROPIC_AUTH_TOKEN";
      return JSON.stringify(
        {
          env: {
            [tokenEnvKey]: token ? "<saved token>" : "<token>",
            ANTHROPIC_BASE_URL: baseUrl,
            ANTHROPIC_DEFAULT_HAIKU_MODEL: haiku,
            ANTHROPIC_DEFAULT_OPUS_MODEL: opus,
            ANTHROPIC_DEFAULT_SONNET_MODEL: sonnet
          }
        },
        null,
        2
      );
    }

    const lines = updateConfigFile
      ? [
          `model_provider = ${quotedConfigValue(providerId)}`,
          `model = ${quotedConfigValue(model)}`,
          "",
          `[model_providers.${providerTableKey(providerId)}]`,
          `name = ${quotedConfigValue(name)}`,
          `wire_api = ${quotedConfigValue(wireApi)}`,
          `requires_openai_auth = ${draft.requiresOpenaiAuth ? "true" : "false"}`,
          `base_url = ${quotedConfigValue(baseUrl)}`
        ]
      : [
          "# 当前关闭了'更新配置文件'，保存时只会更新 CodeSail 本地数据。",
          "",
          `[model_providers.${providerTableKey(providerId)}]`,
          `name = ${quotedConfigValue(name)}`,
          `wire_api = ${quotedConfigValue(wireApi)}`,
          `requires_openai_auth = ${draft.requiresOpenaiAuth ? "true" : "false"}`,
          `base_url = ${quotedConfigValue(baseUrl)}`
        ];

    return lines.join("\n");
  }, [draft, modelValue, claudeHaikuModel, claudeOpusModel, claudeSonnetModel, updateConfigFile]);

  return (
    <div className="advanced-settings wide">
      <button
        className={`advanced-toggle ${open ? "open" : ""}`}
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <SlidersHorizontal size={17} />
        <span>高级设置</span>
        <ChevronDown size={17} />
      </button>

      {open ? (
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
                    onClick={() => onUpdateDraft({ requiresOpenaiAuth: false })}
                  >
                    Token
                  </button>
                  <button
                    className={draft.requiresOpenaiAuth ? "active" : ""}
                    type="button"
                    onClick={() => onUpdateDraft({ requiresOpenaiAuth: true })}
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
                    onClick={() => onUpdateDraft({ requiresOpenaiAuth: false })}
                  >
                    Bearer Token
                  </button>
                  <button
                    className={draft.requiresOpenaiAuth ? "active" : ""}
                    type="button"
                    onClick={() => onUpdateDraft({ requiresOpenaiAuth: true })}
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
  );
}
