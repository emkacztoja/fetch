import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface Message {
  role: "user" | "assistant" | "tool";
  text: string;
  id: number;
}

let msgId = 0;

function App() {
  const [chatVisible, setChatVisible] = useState(false);
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [loading, setLoading] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const chatLogRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = useCallback(() => {
    setTimeout(() => {
      if (chatLogRef.current) {
        chatLogRef.current.scrollTop = chatLogRef.current.scrollHeight;
      }
    }, 50);
  }, []);

  const addMessage = useCallback((role: Message["role"], text: string) => {
    setMessages((prev) => [...prev, { role, text, id: ++msgId }]);
    scrollToBottom();
  }, [scrollToBottom]);

  const handleDoubleClick = useCallback(() => {
    setChatVisible((v) => !v);
  }, []);

  const handleSend = useCallback(async () => {
    const prompt = input.trim();
    if (!prompt || loading) return;

    setChatVisible(true);
    setInput("");
    setLoading(true);
    addMessage("user", prompt);

    try {
      const response = await invoke<string>("chat_with_pet", { prompt });
      addMessage("assistant", response);
    } catch (err) {
      addMessage("tool", `Error: ${err}`);
    } finally {
      setLoading(false);
    }
  }, [input, loading, addMessage]);

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

  const handleClear = useCallback(async () => {
    try {
      await invoke("clear_conversation");
      setMessages([]);
    } catch {
      setMessages([]);
    }
  }, []);

  // Focus input when chat opens
  useEffect(() => {
    if (chatVisible) {
      setTimeout(() => inputRef.current?.focus(), 100);
    }
  }, [chatVisible]);

  return (
    <div className="pet-container" data-tauri-drag-region>
      {/* Chat log panel */}
      {chatVisible && messages.length > 0 && (
        <div className="chat-log-panel" ref={chatLogRef}>
          {messages.map((msg) => (
            <div key={msg.id} className={`chat-bubble ${msg.role}`}>
              <span className="bubble-label">
                {msg.role === "user" ? "You" : msg.role === "tool" ? "Tool" : "Fetch"}
              </span>
              <p>{msg.text}</p>
            </div>
          ))}
          {loading && (
            <div className="chat-bubble assistant">
              <span className="bubble-label">Fetch</span>
              <p className="typing-indicator">...</p>
            </div>
          )}
        </div>
      )}

      {/* Pet sprite */}
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
        {messages.length > 0 && (
          <button className="clear-btn" onClick={handleClear} title="Clear conversation">
            Clear
          </button>
        )}
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
