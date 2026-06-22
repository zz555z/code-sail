import { useCallback, useEffect, useState } from "react";

const SUCCESS_DISMISS_MS = 1500;
const SUCCESS_CLEAR_MS = 1860;
const ERROR_DISMISS_MS = 4000;
const ERROR_CLEAR_MS = 4360;

export type MessageInput = string | { text: string; isError: boolean };

function isErrorMessage(message: MessageInput): boolean {
  if (typeof message === "string") {
    return message.includes("失败") || message.includes("错误") || message.includes("无法") || message.includes("Error") || message.includes("error");
  }
  return message.isError;
}

function extractText(message: MessageInput): string {
  return typeof message === "string" ? message : message.text;
}

export function useTransientMessage(initialPaused = false) {
  const [message, setMessage] = useState("");
  const [isErrorState, setIsErrorState] = useState(false);
  const [messageDismissing, setMessageDismissing] = useState(false);
  const [paused, setPaused] = useState(initialPaused);

  const setMessageWithMeta = useCallback((input: MessageInput) => {
    const text = extractText(input);
    const isError = isErrorMessage(input);
    setMessage(text);
    setIsErrorState(isError);
  }, []);

  useEffect(() => {
    if (!message) {
      setMessageDismissing(false);
      return;
    }

    setMessageDismissing(false);
    if (paused) return;

    const dismissMs = isErrorState ? ERROR_DISMISS_MS : SUCCESS_DISMISS_MS;
    const clearMs = isErrorState ? ERROR_CLEAR_MS : SUCCESS_CLEAR_MS;

    const dismissId = window.setTimeout(() => setMessageDismissing(true), dismissMs);
    const clearId = window.setTimeout(() => setMessage(""), clearMs);
    return () => {
      window.clearTimeout(dismissId);
      window.clearTimeout(clearId);
    };
  }, [message, paused, isErrorState]);

  const dismissMessage = useCallback(() => {
    setMessageDismissing(true);
    window.setTimeout(() => setMessage(""), 360);
  }, []);

  return {
    message,
    setMessage: setMessageWithMeta,
    setPaused,
    dismissMessage,
    messageClassName: `message-strip ${messageDismissing ? "leaving" : ""}`
  };
}
