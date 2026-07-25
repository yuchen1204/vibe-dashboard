import { useState } from "react";
import { useParams } from "react-router-dom";
import { useTargets } from "@/hooks/useTargets";
import { useTodos } from "@/hooks/useTodos";
import { TargetList } from "@/components/target/TargetList";
import { Board } from "@/components/board/Board";
import { ChatSidebar } from "@/components/chat/ChatSidebar";
import { Button } from "@/components/ui/button";

export function WorkspaceViewPage() {
  const { wid } = useParams<{ wid: string }>();
  const [selectedTargetId, setSelectedTargetId] = useState<string | null>(null);
  const [chatOpen, setChatOpen] = useState(false);
  const { data: targets } = useTargets(wid ?? "");
  const { data: todos } = useTodos(wid ?? "");

  if (!wid) return null;

  return (
    <div className="flex flex-1">
      <TargetList
        workspaceId={wid}
        selectedTargetId={selectedTargetId}
        onSelect={setSelectedTargetId}
      />
      <Board
        workspaceId={wid}
        targets={targets ?? []}
        selectedTargetId={selectedTargetId}
        todos={todos ?? []}
      />
      {/* Chat toggle button */}
      {!chatOpen && (
        <div className="fixed bottom-4 right-4 z-50">
          <Button
            onClick={() => setChatOpen(true)}
            className="rounded-full shadow-lg h-12 w-12"
            size="icon"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              className="h-5 w-5"
            >
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
          </Button>
        </div>
      )}
      {/* Chat sidebar */}
      <ChatSidebar
        workspaceId={wid}
        open={chatOpen}
        onClose={() => setChatOpen(false)}
      />
    </div>
  );
}