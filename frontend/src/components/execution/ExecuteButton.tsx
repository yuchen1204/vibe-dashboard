import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { useExecuteTodo, useJob } from "@/hooks/useExecution";
import type { Todo } from "@/types/api";

interface Props {
  workspaceId: string;
  todo: Todo;
  runningJobId: string | null;
}

const STATUS_LABEL: Record<string, string> = {
  pending: "等待中",
  running: "执行中",
  success: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

export function ExecuteButton({ workspaceId, todo, runningJobId }: Props) {
  const [showLog, setShowLog] = useState(false);
  const execute = useExecuteTodo(workspaceId);
  const { data: job } = useJob(runningJobId);

  // Check if this todo is currently being executed
  const isExecuting = runningJobId != null && job?.status === "running";
  const hasCompleted = runningJobId != null && job && (job.status === "success" || job.status === "failed" || job.status === "cancelled");

  const handleExecute = () => {
    execute.mutate({ todoId: todo.id });
  };

  return (
    <>
      <div className="flex items-center gap-1">
        <Button
          size="sm"
          variant="outline"
          className="h-7 px-2 text-xs"
          onClick={handleExecute}
          disabled={execute.isPending || isExecuting}
          title="执行此 Todo"
        >
          {isExecuting ? "⏳" : hasCompleted ? "↻" : "▶"}
        </Button>
        {runningJobId && (
          <Button
            size="sm"
            variant="ghost"
            className="h-7 px-1 text-xs"
            onClick={() => setShowLog(true)}
          >
            📋
          </Button>
        )}
      </div>

      <Dialog open={showLog} onOpenChange={setShowLog}>
        <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle>
              执行日志 — {todo.title}
              {job && (
                <span className="ml-2 text-sm font-normal text-muted-foreground">
                  [{STATUS_LABEL[job.status] ?? job.status}]
                </span>
              )}
            </DialogTitle>
          </DialogHeader>
          <div className="flex-1 overflow-y-auto">
            <pre className="bg-muted p-4 rounded-md text-xs font-mono whitespace-pre-wrap break-all max-h-[50vh]">
              {job?.output || "无输出"}
            </pre>
          </div>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">关闭</Button>
            </DialogClose>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}