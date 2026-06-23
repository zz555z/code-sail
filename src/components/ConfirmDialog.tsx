import { useEffect, useRef } from "react";

type ConfirmDialogProps = {
  open: boolean;
  title: string;
  description: string;
  confirmLabel: string;
  cancelLabel?: string;
  danger?: boolean;
  busy?: boolean;
  onCancel: () => void;
  onConfirm: () => void | Promise<void>;
};

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel,
  cancelLabel = "取消",
  danger = false,
  busy = false,
  onCancel,
  onConfirm
}: ConfirmDialogProps) {
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;

    if (open && !dialog.open) {
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open]);

  const handleClose = (event: React.SyntheticEvent<HTMLDialogElement>) => {
    // 仅处理点击 backdrop 关闭的情况
    if (event.target === dialogRef.current) {
      onCancel();
    }
  };

  const handleCancel = (event: React.SyntheticEvent<HTMLDialogElement>) => {
    event.preventDefault();
    onCancel();
  };

  return (
    <dialog
      className="confirm-dialog"
      ref={dialogRef}
      onClose={handleClose}
      onCancel={handleCancel}
      aria-labelledby="confirm-dialog-title"
    >
      <div className="confirm-dialog-copy">
        <strong id="confirm-dialog-title">{title}</strong>
        <span>{description}</span>
      </div>
      <div className="confirm-dialog-actions">
        <button className="soft-button" type="button" onClick={onCancel} disabled={busy}>
          {cancelLabel}
        </button>
        <button
          className={danger ? "danger-button" : "primary-button"}
          type="button"
          onClick={() => void onConfirm()}
          disabled={busy}
        >
          {confirmLabel}
        </button>
      </div>
    </dialog>
  );
}
