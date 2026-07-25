import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useUpdateTodo, useDeleteTodo } from "@/hooks/useTodos";
import type { Target, Todo, TodoStatus } from "@/types/api";
import { TodoDialog } from "./TodoDialog";
import { ExecuteButton } from "@/components/execution/ExecuteButton";
import { useExecutionStore } from "@/stores/execution";

const TODO_STATUSES: TodoStatus[] = ["todo", "doing", "done", "blocked"];
const STATUS_LABEL: Record<TodoStatus, string> = {
  todo: "待办",
  doing: "进行中",
  done: "已完成",
  blocked: "阻塞",
};

interface Props {
  workspaceId: string;
  todo: Todo;
  targets: Target[];
}

export function TodoCard({ workspaceId, todo, targets }: Props) {
  const update = useUpdateTodo(workspaceId);
  const del = useDeleteTodo(workspaceId);
  const target = targets.find((t) => t.id === todo.target_id);
  const jobRecord = useExecutionStore((s) => s.jobByTodo[todo.id]);
  const runningJobId = jobRecord?.jobId ?? null;

  return (
    <div className="rounded-md border bg-background p-3 space-y-2">
      <div className="flex items-start justify-between gap-2">
        <span className="text-sm font-medium">{todo.title}</span>
        {target && (
          <Badge variant="secondary" className="text-xs shrink-0">
            {target.title}
          </Badge>
        )}
      </div>
      {todo.description && (
        <p className="text-xs text-muted-foreground line-clamp-2">
          {todo.description}
        </p>
      )}
      <div className="flex items-center gap-2">
        <Select
          value={todo.status}
          onValueChange={(v) =>
            update.mutate({
              id: todo.id,
              input: { status: v as TodoStatus },
            })
          }
        >
          <SelectTrigger className="h-7 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {TODO_STATUSES.map((s) => (
              <SelectItem key={s} value={s}>
                {STATUS_LABEL[s]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <ExecuteButton
          workspaceId={workspaceId}
          todo={todo}
          runningJobId={runningJobId}
        />
        <TodoDialog
          workspaceId={workspaceId}
          targets={targets}
          editing={todo}
          trigger={
            <Button size="sm" variant="ghost" className="h-7 px-2 text-xs">
              编辑
            </Button>
          }
        />
        <Button
          size="sm"
          variant="ghost"
          className="h-7 px-2 text-xs text-destructive"
          onClick={() => del.mutate(todo.id)}
        >
          删除
        </Button>
      </div>
    </div>
  );
}