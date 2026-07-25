import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useCreateTodo, useUpdateTodo } from "@/hooks/useTodos";
import type { Target, Todo, TodoStatus } from "@/types/api";

const TODO_STATUSES: TodoStatus[] = ["todo", "doing", "done", "blocked"];
const STATUS_LABEL: Record<TodoStatus, string> = {
  todo: "待办",
  doing: "进行中",
  done: "已完成",
  blocked: "阻塞",
};

interface Props {
  workspaceId: string;
  targets: Target[];
  editing?: Todo | null;
  defaultTargetId?: string;
  trigger: React.ReactNode;
}

export function TodoDialog({
  workspaceId,
  targets,
  editing,
  defaultTargetId,
  trigger,
}: Props) {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [status, setStatus] = useState<TodoStatus>("todo");
  const [targetId, setTargetId] = useState(
    defaultTargetId ?? targets[0]?.id ?? "",
  );

  const create = useCreateTodo(workspaceId);
  const update = useUpdateTodo(workspaceId);

  useEffect(() => {
    if (open) {
      setTitle(editing?.title ?? "");
      setDescription(editing?.description ?? "");
      setStatus(editing?.status ?? "todo");
      setTargetId(
        editing?.target_id ?? defaultTargetId ?? targets[0]?.id ?? "",
      );
    }
  }, [open, editing, defaultTargetId, targets]);

  const handleSubmit = async () => {
    if (!title.trim() || !targetId) return;
    if (editing) {
      await update.mutateAsync({
        id: editing.id,
        input: {
          title: title.trim(),
          description: description.trim(),
          status,
        },
      });
    } else {
      await create.mutateAsync({
        targetId,
        input: { title: title.trim(), description: description.trim() },
      });
    }
    setOpen(false);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        {trigger}
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{editing ? "编辑 Todo" : "新建 Todo"}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 py-2">
          <div className="space-y-2">
            <Label htmlFor="todo-title">标题</Label>
            <Input
              id="todo-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="todo-desc">描述</Label>
            <Textarea
              id="todo-desc"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          <div className="space-y-2">
            <Label>Target</Label>
            <Select
              value={targetId}
              onValueChange={setTargetId}
              disabled={!!editing}
            >
              <SelectTrigger>
                <SelectValue placeholder="选择 target" />
              </SelectTrigger>
              <SelectContent>
                {targets.map((t) => (
                  <SelectItem key={t.id} value={t.id}>
                    {t.title}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          {editing && (
            <div className="space-y-2">
              <Label>状态</Label>
              <Select
                value={status}
                onValueChange={(v) => setStatus(v as TodoStatus)}
              >
                <SelectTrigger>
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
            </div>
          )}
        </div>
        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">取消</Button>
          </DialogClose>
          <Button
            onClick={handleSubmit}
            disabled={
              create.isPending || update.isPending || !title.trim() || !targetId
            }
          >
            {editing ? "保存" : "创建"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}