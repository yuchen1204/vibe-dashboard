import { useState } from "react";
import { useParams } from "react-router-dom";
import { useTargets } from "@/hooks/useTargets";
import { useTodos } from "@/hooks/useTodos";
import { TargetList } from "@/components/target/TargetList";
import { Board } from "@/components/board/Board";

export function WorkspaceViewPage() {
  const { wid } = useParams<{ wid: string }>();
  const [selectedTargetId, setSelectedTargetId] = useState<string | null>(null);
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
    </div>
  );
}