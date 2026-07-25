import { useState, useCallback, useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { wsClient } from "@/lib/ws";
import type { ServerMsg } from "@/types/api";

interface ChatMessage {
  role: "user" | "assistant" | "tool_call" | "tool_result";
  content: string;
  tool_name?: string;
}

interface ChatSidebarProps {
  workspaceId: string;
  open: boolean;
  onClose: () => void;
}

export function ChatSidebar({ workspaceId, open, onClose }: ChatSidebarProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  // Scroll to bottom on new messages
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  // Subscribe to WS messages
  useEffect(() => {
    if (!open) return;

    const unsub = wsClient.subscribe((msg: ServerMsg) => {
      switch (msg.type) {
        case "chat_response":
          setMessages((prev) => [
            ...prev,
            { role: "assistant", content: msg.payload.text },
          ]);
          setLoading(false);
          break;
        case "chat_tool_call":
          setMessages((prev) => [
            ...prev,
            {
              role: "tool_call",
              content: JSON.stringify(msg.payload.args, null, 2),
              tool_name: msg.payload.tool_name,
            },
          ]);
          break;
        case "chat_tool_result":
          setMessages((prev) => [
            ...prev,
            {
              role: "tool_result",
              content: msg.payload.result,
              tool_name: msg.payload.tool_name,
            },
          ]);
          break;
        case "chat_error":
          setMessages((prev) => [
            ...prev,
            { role: "assistant", content: `错误: ${msg.payload.message}` },
          ]);
          setLoading(false);
          break;
      }
    });

    return () => unsub();
  }, [open]);

  const handleSend = useCallback(() => {
    const text = input.trim();
    if (!text || loading) return;

    setMessages((prev) => [...prev, { role: "user", content: text }]);
    setInput("");
    setLoading(true);

    wsClient.send({
      type: "chat_message",
      payload: { text, workspace_id: workspaceId },
    });
  }, [input, loading, workspaceId]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  if (!open) return null;

  return (
    <div className="w-80 border-l border-border bg-background flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <h3 className="text-sm font-semibold">AI 编排助手</h3>
        <Button variant="ghost" size="icon" onClick={onClose} className="h-7 w-7">
          <span className="sr-only">关闭</span>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            className="h-4 w-4"
          >
            <path d="M18 6L6 18M6 6l12 12" />
          </svg>
        </Button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {messages.length === 0 && (
          <p className="text-sm text-muted-foreground text-center pt-8">
            输入你的需求，AI 编排助手会帮你分解任务、创建和执行 todo。
          </p>
        )}
        {messages.map((msg, i) => (
          <div
            key={i}
            className={`text-sm ${
              msg.role === "user" ? "text-right" : ""
            }`}
          >
            {msg.role === "user" && (
              <div className="inline-block bg-primary text-primary-foreground rounded-lg px-3 py-2 max-w-[90%]">
                {msg.content}
              </div>
            )}
            {msg.role === "assistant" && (
              <div className="inline-block bg-muted rounded-lg px-3 py-2 max-w-[90%] whitespace-pre-wrap">
                {msg.content}
              </div>
            )}
            {msg.role === "tool_call" && (
              <div className="inline-block bg-amber-50 dark:bg-amber-950 border border-amber-200 dark:border-amber-800 rounded-lg px-3 py-2 max-w-[90%] text-xs">
                <span className="font-medium text-amber-700 dark:text-amber-400">
                  🔧 {msg.tool_name}
                </span>
                <pre className="mt-1 text-amber-600 dark:text-amber-500 overflow-x-auto">
                  {msg.content}
                </pre>
              </div>
            )}
            {msg.role === "tool_result" && (
              <div className="inline-block bg-green-50 dark:bg-green-950 border border-green-200 dark:border-green-800 rounded-lg px-3 py-2 max-w-[90%] text-xs">
                <span className="font-medium text-green-700 dark:text-green-400">
                  ✅ {msg.tool_name}
                </span>
                <pre className="mt-1 text-green-600 dark:text-green-500 overflow-x-auto whitespace-pre-wrap">
                  {msg.content}
                </pre>
              </div>
            )}
          </div>
        ))}
        {loading && (
          <div className="text-sm text-muted-foreground">
            <div className="inline-block bg-muted rounded-lg px-3 py-2">
              <span className="animate-pulse">思考中...</span>
            </div>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <div className="border-t border-border p-3">
        <div className="flex gap-2">
          <Input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="输入需求..."
            disabled={loading}
            className="flex-1"
          />
          <Button onClick={handleSend} disabled={loading || !input.trim()} size="sm">
            发送
          </Button>
        </div>
      </div>
    </div>
  );
}