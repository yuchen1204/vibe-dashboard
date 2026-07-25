import { useTargets } from "@/hooks/useTargets";
import { CreateTargetDialog } from "./CreateTargetDialog";

interface Props {
  workspaceId: string;
  selectedTargetId: string | null;
  onSelect: (id: string | null) => void;
}

export function TargetList({
  workspaceId,
  selectedTargetId,
  onSelect,
}: Props) {
  const { data: targets, isLoading } = useTargets(workspaceId);

  return (
    <div className="w-56 border-r bg-card p-4 space-y-2">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold">Targets</h3>
        <CreateTargetDialog workspaceId={workspaceId} />
      </div>
      <button
        onClick={() => onSelect(null)}
        className={`block w-full text-left rounded-md px-2 py-1.5 text-sm hover:bg-accent ${
          selectedTargetId === null ? "bg-accent font-medium" : ""
        }`}
      >
        全部
      </button>
      {isLoading && <p className="text-xs text-muted-foreground">加载中…</p>}
      {targets?.map((t) => (
        <button
          key={t.id}
          onClick={() => onSelect(t.id)}
          className={`block w-full text-left rounded-md px-2 py-1.5 text-sm hover:bg-accent truncate ${
            selectedTargetId === t.id ? "bg-accent font-medium" : ""
          }`}
        >
          {t.title}
        </button>
      ))}
    </div>
  );
}