import { memo, type PointerEvent } from "react";
import { Copy, HeartPulse, Loader2, MousePointerClick, Pencil, Trash2 } from "lucide-react";
import type { HealthCheckResponse, ProviderView } from "../lib/types";

type HealthStatus = HealthCheckResponse | "loading";
type DragOverPlacement = "before" | "after";

type ProviderRowProps = {
  provider: ProviderView;
  active: boolean;
  activeModel: string;
  selected: boolean;
  busy: boolean;
  dragging?: boolean;
  dragOverPlacement?: DragOverPlacement | null;
  healthStatus?: HealthStatus;
  onPointerDown?: (event: PointerEvent<HTMLElement>) => void;
  onEdit: () => void;
  onCopy: () => void;
  onSetCurrent: () => void;
  onHealthCheck: () => void;
  onDelete: () => void;
};

export const ProviderRow = memo(function ProviderRow({
  provider,
  active,
  activeModel,
  selected,
  busy,
  dragging = false,
  dragOverPlacement = null,
  healthStatus,
  onPointerDown,
  onEdit,
  onCopy,
  onSetCurrent,
  onHealthCheck,
  onDelete
}: ProviderRowProps) {
  const rowClassName = [
    "config-row",
    selected ? "selected" : "",
    active ? "active" : "",
    dragging ? "is-dragging" : "",
    dragOverPlacement ? `drag-over-${dragOverPlacement}` : ""
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <article
      className={rowClassName}
      data-provider-row-id={provider.id}
      onPointerDown={onPointerDown}
      role="listitem"
    >
      <div className="config-row-content">
        <div className="config-row-top">
          <div className="provider-copy" title={provider.id}>
            <strong>{provider.name || provider.id}</strong>
          </div>

          <div className="config-row-tools" aria-label={`${provider.id} 操作`}>
            {active ? <span className="flag current">当前</span> : null}
            <div className="row-actions">
              {!active ? (
                <button
                  className="row-button set-current"
                  type="button"
                  data-tooltip="设为当前模型"
                  data-tooltip-placement="left"
                  aria-label={`将 ${provider.name || provider.id} 设为当前模型`}
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
        </div>

        <div className="provider-details">
          <em>{provider.baseUrl || "base_url missing"}</em>
          {activeModel ? <em className="provider-model">{active ? "当前模型" : "模型"}：{activeModel}</em> : null}
        </div>
      </div>
    </article>
  );
});
