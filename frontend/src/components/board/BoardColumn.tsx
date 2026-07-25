import type { Todo, Target } from "@/types/api";
import { TodoCard } from "./TodoCard";

interface Props {
  title: string;
  todos: Todo[];
  workspaceId: string;
  targets: Target[];
}

export function BoardColumn({
  title,
  todos,
  workspaceId,
  targets,
}: Props) {
  return (
    <div className="flex-1 min-w-[220px] rounded-md bg-muted/40 p-3">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-semibold">{title}</h3>
        <span className="text-xs text-muted-foreground">{todos.length}</span>
      </div>
      <div className="space-y-2">
        {todos.map((todo) => (
          <TodoCard
            key={todo.id}
            workspaceId={workspaceId}
            todo={todo}
            targets={targets}
          />
        ))}
        {todos.length === 0 && (
          <p className="text-xs text-muted-foreground py-4 text-center">无</p>
        )}
      </div>
    </div>
  );
}