import { type PointerEvent, useEffect, useRef, useState } from "react";
import type { ProviderView } from "../lib/types";

export type DragOverPlacement = "before" | "after";
export type DragOverTarget = { providerId: string; placement: DragOverPlacement } | null;

type DragRowRect = { providerId: string; top: number; midpoint: number };

type UseProviderReorderOptions = {
  providers: ProviderView[];
  busy: boolean;
  reorderProviders: (providerIds: string[]) => Promise<void>;
};

export function useProviderReorder({ providers, busy, reorderProviders }: UseProviderReorderOptions) {
  const [draggingProviderId, setDraggingProviderId] = useState<string | null>(null);
  const [dragOverTarget, setDragOverTarget] = useState<DragOverTarget>(null);
  const providersRef = useRef<ProviderView[]>([]);
  const draggingProviderIdRef = useRef<string | null>(null);
  const dragOverTargetRef = useRef<DragOverTarget>(null);
  const dragRowRectsRef = useRef<DragRowRect[]>([]);
  const canDragProvidersRef = useRef(false);

  const canDragProviders = providers.length > 1 && !busy;

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
    if (target instanceof HTMLElement && target.closest(".row-actions, .config-row-tools")) {
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

  return {
    draggingProviderId,
    dragOverTarget,
    handleProviderPointerDown
  };
}
