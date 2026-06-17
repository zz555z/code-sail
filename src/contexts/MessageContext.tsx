import { createContext, useContext, type ReactNode } from "react";

export type MessageContextValue = {
  message: string;
  messageClassName: string;
  setMessage: (message: string) => void;
  setMessagePaused: (paused: boolean) => void;
};

const MessageContext = createContext<MessageContextValue | null>(null);

export function useMessage(): MessageContextValue {
  const ctx = useContext(MessageContext);
  if (!ctx) throw new Error("useMessage must be used within a MessageProvider");
  return ctx;
}

export function MessageProvider({
  value,
  children
}: {
  value: MessageContextValue;
  children: ReactNode;
}) {
  return <MessageContext.Provider value={value}>{children}</MessageContext.Provider>;
}
