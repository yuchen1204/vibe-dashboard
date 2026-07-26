import { useState, useCallback, useEffect, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { wsClient } from "@/lib/ws";
import type { ServerMsg } from "@/types/api";

interface ChatMessage {
  role: "user" | "assistant" | "thinking" | "tool_call" | "tool_result";
  content: string;
  tool_name?: string;
  iteration?: number;
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
        case "chat_thinking":
          setMessages((prev) => [
            ...prev,
            {
              role: "thinking",
              content: msg.payload.text,
              iteration: msg.payload.iteration,
            },
          ]);
          break;
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
        case "session_history":
          setMessages(
            msg.payload.messages
              .filter((m) => m.role === "user" || m.role === "assistant")
              .map((m) => ({
                role: m.role as "user" | "assistant",
                content: m.content,
              }))
          );
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

  const handleNewSession = useCallback(() => {
    setMessages([]);
    wsClient.send({
      type: "new_session",
      payload: { workspace_id: workspaceId },
    });
  }, [workspaceId]);

  const handleGetHistory = useCallback(() => {
    setMessages([]);
    setLoading(true);
    wsClient.send({
      type: "get_history",
      payload: { workspace_id: workspaceId },
    });
  }, [workspaceId]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  if (!open) return null;

  return (
    <div className="w-96 border-l border-border bg-background flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border shrink-0">
        <div className="flex items-center gap-2">
          <h3 className="text-sm font-semibold">AI 编排助手</h3>
          {loading && (
            <span className="flex items-center gap-1 text-xs text-muted-foreground">
              <span className="h-1.5 w-1.5 rounded-full bg-blue-500 animate-pulse" />
              思考中
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            onClick={handleGetHistory}
            className="h-7 w-7"
            title="查看历史会话"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="h-4 w-4"
            >
              <circle cx="12" cy="12" r="10" />
              <polyline points="12 6 12 12 16 14" />
            </svg>
            <span className="sr-only">历史会话</span>
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={handleNewSession}
            className="h-7 w-7"
            title="开启新会话"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="h-4 w-4"
            >
              <path d="M12 5v14M5 12h14" />
            </svg>
            <span className="sr-only">新会话</span>
          </Button>
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
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto">
        <div className="p-4 space-y-4">
          {messages.length === 0 && (
            <p className="text-sm text-muted-foreground text-center pt-8">
              输入你的需求，AI 编排助手会帮你分解任务、创建和执行 todo。
            </p>
          )}
          {messages.map((msg, i) => (
            <MessageBubble key={i} msg={msg} />
          ))}
          {loading && messages[messages.length - 1]?.role !== "thinking" && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span className="h-2 w-2 rounded-full bg-blue-500 animate-pulse" />
              <span>思考中...</span>
            </div>
          )}
          <div ref={bottomRef} />
        </div>
      </div>

      {/* Input */}
      <div className="border-t border-border p-3 shrink-0">
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

function MessageBubble({ msg }: { msg: ChatMessage }) {
  if (msg.role === "user") {
    return (
      <div className="text-right">
        <div className="inline-block bg-primary text-primary-foreground rounded-2xl rounded-tr-md px-4 py-2.5 max-w-[85%] text-sm">
          {msg.content}
        </div>
      </div>
    );
  }

  if (msg.role === "thinking") {
    return (
      <div className="flex gap-2">
        <div className="shrink-0 mt-1">
          <div className="h-5 w-5 rounded-full bg-blue-100 dark:bg-blue-900 flex items-center justify-center text-[10px] font-bold text-blue-600 dark:text-blue-300">
            {msg.iteration}
          </div>
        </div>
        <div className="flex-1">
          <div className="text-xs text-muted-foreground mb-1 flex items-center gap-1">
            <span className="h-1.5 w-1.5 rounded-full bg-blue-500 animate-pulse" />
            LLM 思考
          </div>
          <div className="text-sm text-muted-foreground whitespace-pre-wrap border-l-2 border-blue-200 dark:border-blue-800 pl-3 italic">
            {msg.content ? (
              msg.content
            ) : (
              <span className="text-blue-400">调用工具中...</span>
            )}
          </div>
        </div>
      </div>
    );
  }

  if (msg.role === "tool_call") {
    let args: Record<string, unknown> = {};
    try {
      args = JSON.parse(msg.content);
    } catch {
      args = {};
    }

    return (
      <div className="flex gap-2">
        <div className="shrink-0 mt-1">
          <div className="h-5 w-5 rounded-full bg-amber-100 dark:bg-amber-900 flex items-center justify-center text-[10px]">
            🔧
          </div>
        </div>
        <div className="flex-1">
          <div className="text-xs text-amber-600 dark:text-amber-400 font-medium">
            {msg.tool_name}
          </div>
          <div className="mt-1 text-xs text-muted-foreground space-y-0.5">
            {Object.entries(args).map(([key, value]) => (
              <div key={key} className="flex gap-1">
                <span className="text-amber-500 shrink-0">{key}:</span>
                <span className="truncate">
                  {typeof value === "string"
                    ? value.length > 80
                      ? value.slice(0, 80) + "..."
                      : value
                    : JSON.stringify(value).slice(0, 80)}
                </span>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (msg.role === "tool_result") {
    return (
      <div className="flex gap-2">
        <div className="shrink-0 mt-1">
          <div className="h-5 w-5 rounded-full bg-green-100 dark:bg-green-900 flex items-center justify-center text-[10px]">
            ✓
          </div>
        </div>
        <div className="flex-1 min-w-0">
          <div className="text-xs text-green-600 dark:text-green-400 font-medium">
            {msg.tool_name}
          </div>
          <div className="mt-1 text-xs text-muted-foreground bg-muted/50 rounded-md p-2 overflow-x-auto max-h-32 overflow-y-auto whitespace-pre-wrap font-mono">
            {msg.content.length > 300
              ? msg.content.slice(0, 300) + "..."
              : msg.content}
          </div>
        </div>
      </div>
    );
  }

  // assistant role
  return (
    <div className="flex gap-2">
      <div className="shrink-0 mt-1">
        <div className="h-5 w-5 rounded-full bg-primary/10 flex items-center justify-center text-[10px] font-bold text-primary">
          AI
        </div>
      </div>
      <div className="flex-1">
        <div className="text-sm whitespace-pre-wrap rounded-2xl rounded-tl-md bg-muted px-4 py-2.5 max-w-[85%]">
          {msg.content}
        </div>
      </div>
    </div>
  );
}