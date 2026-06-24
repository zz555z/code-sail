import { memo, type PointerEvent } from "react";
import type { HealthStatus, ProviderView } from "../lib/types";
import { ProviderActions } from "./ProviderActions";
type DragOverPlacement = "before" | "after";

type ProviderRowProps = {
  provider: ProviderView;
  active: boolean;
  activeModel: string;
  selected: boolean;
  busy: boolean;
  dragging?: boolean;
  dragOverPlacement?: DragOverPlacement | null;
  reorderDisabled?: boolean;
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
  reorderDisabled = false,
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
      data-provider-reorder-disabled={reorderDisabled ? "true" : undefined}
      onPointerDown={onPointerDown}
      role="listitem"
    >
      <div className="config-row-content">
        <div className="config-row-top">
          <div className="provider-copy" title={provider.id}>
            <strong>{provider.name || provider.id}</strong>
          </div>

          <ProviderActions
            provider={provider}
            active={active}
            busy={busy}
            healthStatus={healthStatus}
            onEdit={onEdit}
            onCopy={onCopy}
            onSetCurrent={onSetCurrent}
            onHealthCheck={onHealthCheck}
            onDelete={onDelete}
          />
        </div>

        <div className="provider-details">
          <em>{provider.baseUrl || "base_url missing"}</em>
          {activeModel ? <em className="provider-model">{active ? "当前模型" : "模型"}：{activeModel}</em> : null}
        </div>
      </div>
    </article>
  );
});
