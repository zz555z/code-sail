import { Copy, HeartPulse, Loader2, MousePointerClick, Pencil, Trash2 } from "lucide-react";
import type { HealthStatus, ProviderView } from "../lib/types";

type ProviderActionsProps = {
  provider: ProviderView;
  active: boolean;
  busy: boolean;
  healthStatus?: HealthStatus;
  onEdit: () => void;
  onCopy: () => void;
  onSetCurrent: () => void;
  onHealthCheck: () => void;
  onDelete: () => void;
};

export function ProviderActions({
  provider,
  active,
  busy,
  healthStatus,
  onEdit,
  onCopy,
  onSetCurrent,
  onHealthCheck,
  onDelete
}: ProviderActionsProps) {
  const label = provider.name || provider.id;

  return (
    <div className="config-row-tools" aria-label={`${provider.id} 操作`}>
      {active ? <span className="flag current">当前</span> : null}
      <div className="row-actions">
        {!active ? (
          <button
            className="row-button set-current"
            type="button"
            data-tooltip="设为当前模型"
            data-tooltip-placement="left"
            aria-label={`将 ${label} 设为当前模型`}
            onClick={onSetCurrent}
            disabled={busy}
          >
            <MousePointerClick size={15} />
          </button>
        ) : null}
        <button
          className="row-button"
          type="button"
          data-tooltip="修改"
          data-tooltip-placement="left"
          aria-label="修改"
          onClick={onEdit}
          disabled={busy}
        >
          <Pencil size={15} />
        </button>
        <button
          className="row-button"
          type="button"
          data-tooltip="复制"
          data-tooltip-placement="left"
          aria-label="复制"
          onClick={onCopy}
          disabled={busy}
        >
          <Copy size={15} />
        </button>
        <button
          className={`row-button health-check ${healthStatus === "loading" ? "health-check-loading" : ""} ${healthStatus && healthStatus !== "loading" ? (healthStatus.available ? "health-ok" : "health-err") : ""}`}
          type="button"
          data-tooltip={
            healthStatus === "loading"
              ? "检查中..."
              : healthStatus
                ? (healthStatus.available ? `可用 · ${healthStatus.latencyMs}ms` : `不可用${healthStatus.error ? ": " + healthStatus.error : ""}`)
                : "健康检查"
          }
          data-tooltip-placement="left"
          aria-label={healthStatus === "loading" ? "检查中" : "健康检查"}
          onClick={onHealthCheck}
          disabled={busy || healthStatus === "loading"}
        >
          {healthStatus === "loading" ? <Loader2 size={15} /> : <HeartPulse size={15} />}
        </button>
        <button
          className="row-button danger"
          type="button"
          data-tooltip="删除"
          data-tooltip-placement="left"
          aria-label="删除"
          onClick={onDelete}
          disabled={busy}
        >
          <Trash2 size={15} />
        </button>
      </div>
    </div>
  );
}
