import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface StatusMessage {
  text: string;
  id: number;
}

function App() {
  const [chatVisible, setChatVisible] = useState(false);
  const [input, setInput] = useState("");
  const [status, setStatus] = useState<StatusMessage | null>(null);
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const statusTimeout = useRef<ReturnType<typeof setTimeout>>(undefined);
  const nextStatusId = useRef(0);

  const showStatus = useCallback((text: string, duration = 3000) => {
    if (statusTimeout.current) clearTimeout(statusTimeout.current);
    const id = nextStatusId.current++;
    setStatus({ text, id });
    statusTimeout.current = setTimeout(() => {
      setStatus((prev) => (prev?.id === id ? null : prev));
    }, duration);
  }, []);

  const handleDoubleClick = useCallback(() => {
    setChatVisible((v) => {
      const next = !v;
      if (next) {
        setTimeout(() => inputRef.current?.focus(), 100);
      }
      return next;
    });
  }, []);

  const handleSend = useCallback(async () => {
    const prompt = input.trim();
    if (!prompt || loading) return;

    setInput("");
    setLoading(true);

    try {
      const response = await invoke<string>("chat_with_pet", { prompt });
      showStatus(response);
    } catch (err) {
      showStatus(`Error: ${err}`, 5000);
    } finally {
      setLoading(false);
    }
  }, [input, loading, showStatus]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") handleSend();
      if (e.key === "Escape") {
        setChatVisible(false);
        setInput("");
      }
    },
    [handleSend],
  );

  // Close chat on click outside
  useEffect(() => {
    if (!chatVisible) return;
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest(".chat-bar-wrapper") || target.closest(".pet-sprite")) return;
      setChatVisible(false);
    };
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, [chatVisible]);

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (statusTimeout.current) clearTimeout(statusTimeout.current);
    };
  }, []);

  return (
    <div className="pet-container" data-tauri-drag-region>
      {/* Status message above pet */}
      {status && <div className="status-message" key={status.id}>{status.text}</div>}

      {/* Pet sprite - drag region for window movement */}
      <div
        className="pet-sprite"
        onDoubleClick={handleDoubleClick}
      >
        <div className="pet-body">
          <div className="pet-eyes">
            <div className="pet-eye" />
            <div className="pet-eye" />
          </div>
          <div className="pet-mouth" />
        </div>
      </div>

      {/* Slide-out chat bar */}
      <div className={`chat-bar-wrapper${chatVisible ? " visible" : ""}`}>
        <div className="chat-bar">
          <input
            ref={inputRef}
            type="text"
            placeholder={loading ? "Thinking..." : "Ask me anything..."}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={loading}
            spellCheck={false}
          />
          <button onClick={handleSend} disabled={loading}>
            {loading ? "..." : "Send"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default App;
