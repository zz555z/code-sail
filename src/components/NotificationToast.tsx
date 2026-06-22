import { type KeyboardEvent } from "react";

type NotificationToastProps = {
  message: string;
  messageClassName: string;
  onDismiss?: () => void;
};

export function NotificationToast({ message, messageClassName, onDismiss }: NotificationToastProps) {
  if (!message) return null;

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (onDismiss && (event.key === "Enter" || event.key === " ")) {
      event.preventDefault();
      onDismiss();
    }
  };

  return (
    <div className="notification-toast-layer" role="status" aria-live="polite">
      <div
        className={`${messageClassName} notification-toast`}
        onClick={onDismiss}
        onKeyDown={handleKeyDown}
        role={onDismiss ? "button" : undefined}
        tabIndex={onDismiss ? 0 : undefined}
        style={{ cursor: onDismiss ? "pointer" : undefined }}
      >
        {message}
      </div>
    </div>
  );
}
