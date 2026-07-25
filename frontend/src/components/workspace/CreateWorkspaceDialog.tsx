import { useState } from "react";
import { useNavigate } from "react-router-dom";
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
import { PathInput } from "@/components/workspace/PathInput";
import { useCreateWorkspace } from "@/hooks/useWorkspaces";

export function CreateWorkspaceDialog() {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const navigate = useNavigate();
  const create = useCreateWorkspace();

  const handleSubmit = async () => {
    if (!name.trim() || !path.trim()) return;
    const ws = await create.mutateAsync({
      name: name.trim(),
      path: path.trim(),
    });
    setName("");
    setPath("");
    setOpen(false);
    navigate(`/workspaces/${ws.id}`);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>新建工作区</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新建工作区</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 py-2">
          <div className="space-y-2">
            <Label htmlFor="ws-name">名称</Label>
            <Input
              id="ws-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="我的项目"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="ws-path">路径</Label>
            <PathInput
              id="ws-path"
              value={path}
              onChange={setPath}
              placeholder="E:/projects/my-project"
            />
          </div>
        </div>
        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">取消</Button>
          </DialogClose>
          <Button
            onClick={handleSubmit}
            disabled={create.isPending || !name.trim() || !path.trim()}
          >
            创建
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}