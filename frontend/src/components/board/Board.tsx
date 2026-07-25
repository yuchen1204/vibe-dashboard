import { useMemo } from "react";
import { Button } from "@/components/ui/button";
import type { Target, Todo, TodoStatus } from "@/types/api";
import { BoardColumn } from "./BoardColumn";
import { TodoDialog } from "./TodoDialog";

const COLUMNS: { status: TodoStatus; title: string }[] = [
  { status: "todo", title: "待办" },
  { status: "doing", title: "进行中" },
  { status: "done", title: "已完成" },
  { status: "blocked", title: "阻塞" },
];

interface Props {
  workspaceId: string;
  targets: Target[];
  selectedTargetId: string | null;
  todos: Todo[];
}

export function Board({ workspaceId, targets, selectedTargetId, todos }: Props) {
  const grouped = useMemo(() => {
    const filtered = selectedTargetId
      ? todos?.filter((t) => t.target_id === selectedTargetId)
      : todos;
    const map: Record<TodoStatus, typeof filtered> = {
      todo: [],
      doing: [],
      done: [],
      blocked: [],
    };
    filtered?.forEach((t) => map[t.status].push(t));
    return map;
  }, [todos, selectedTargetId]);

  return (
    <div className="flex-1 p-6 overflow-hidden flex flex-col">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-2xl font-bold">看板</h1>
        {targets.length > 0 && (
          <TodoDialog
            workspaceId={workspaceId}
            targets={targets}
            defaultTargetId={selectedTargetId ?? undefined}
            trigger={<Button size="sm">新建 Todo</Button>}
          />
        )}
      </div>
      <div className="flex gap-4 overflow-x-auto flex-1">
        {COLUMNS.map((col) => (
          <BoardColumn
            key={col.status}
            title={col.title}
            todos={grouped[col.status] ?? []}
            workspaceId={workspaceId}
            targets={targets}
          />
        ))}
      </div>
    </div>
  );
}