import { useWorkspaces } from "@/hooks/useWorkspaces";
import { WorkspaceCard } from "@/components/workspace/WorkspaceCard";
import { CreateWorkspaceDialog } from "@/components/workspace/CreateWorkspaceDialog";

export function WorkspacesPage() {
  const { data: workspaces, isLoading, isError } = useWorkspaces();

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold">工作区</h1>
        <CreateWorkspaceDialog />
      </div>

      {isLoading && <p className="text-muted-foreground">加载中…</p>}
      {isError && <p className="text-destructive">加载失败</p>}
      {workspaces && workspaces.length === 0 && (
        <p className="text-muted-foreground">
          还没有工作区，点「新建工作区」创建一个吧。
        </p>
      )}
      {workspaces && workspaces.length > 0 && (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {workspaces.map((ws) => (
            <WorkspaceCard key={ws.id} workspace={ws} />
          ))}
        </div>
      )}
    </div>
  );
}