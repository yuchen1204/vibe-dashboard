import { useNavigate } from "react-router-dom";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import type { Workspace } from "@/types/api";
import { useDeleteWorkspace } from "@/hooks/useWorkspaces";

export function WorkspaceCard({ workspace }: { workspace: Workspace }) {
  const navigate = useNavigate();
  const del = useDeleteWorkspace();

  const handleDelete = () => {
    if (
      confirm(`删除工作区「${workspace.name}」及其所有 target 和 todo？`)
    ) {
      del.mutate(workspace.id);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{workspace.name}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 text-sm">
        <div className="text-muted-foreground font-mono text-xs truncate">
          {workspace.path}
        </div>
        <div className="flex gap-2 pt-2">
          <Button size="sm" onClick={() => navigate(`/workspaces/${workspace.id}`)}>
            进入
          </Button>
          <Button size="sm" variant="outline" onClick={handleDelete} disabled={del.isPending}>
            删除
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}