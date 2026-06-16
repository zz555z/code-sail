type NotificationToastProps = {
  message: string;
  messageClassName: string;
};

export function NotificationToast({ message, messageClassName }: NotificationToastProps) {
  if (!message) return null;

  return (
    <div className="notification-toast-layer" role="status" aria-live="polite">
      <div className={`${messageClassName} notification-toast`}>{message}</div>
    </div>
  );
}
