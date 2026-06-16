import { useEffect, useState } from "react";

export function useTransientMessage(initialPaused = false) {
  const [message, setMessage] = useState("");
  const [messageDismissing, setMessageDismissing] = useState(false);
  const [paused, setPaused] = useState(initialPaused);

  useEffect(() => {
    if (!message) {
      setMessageDismissing(false);
      return;
    }

    setMessageDismissing(false);
    if (paused) return;

    const dismissId = window.setTimeout(() => setMessageDismissing(true), 1500);
    const clearId = window.setTimeout(() => setMessage(""), 1860);
    return () => {
      window.clearTimeout(dismissId);
      window.clearTimeout(clearId);
    };
  }, [message, paused]);

  return {
    message,
    setMessage,
    setPaused,
    messageClassName: `message-strip ${messageDismissing ? "leaving" : ""}`
  };
}
