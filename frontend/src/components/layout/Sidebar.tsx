import { NavLink } from "react-router-dom";
import { useWorkspaces } from "@/hooks/useWorkspaces";
import { useUiStore } from "@/stores/ui";

function StatusDot({ ok, label }: { ok: boolean; label: string }) {
  return (
    <div className="flex items-center gap-2">
      <span
        className={`h-2.5 w-2.5 rounded-full ${ok ? "bg-green-500" : "bg-red-500"}`}
      />
      <span className="text-xs text-muted-foreground">{label}</span>
    </div>
  );
}

export function Sidebar({ healthOk }: { healthOk: boolean }) {
  const { data: workspaces } = useWorkspaces();
  const wsStatus = useUiStore((s) => s.wsStatus);

  return (
    <aside className="flex w-60 flex-col border-r bg-card min-h-screen p-4">
      <h2 className="text-lg font-semibold mb-4">Vibe Dashboard</h2>
      <nav className="space-y-1 flex-1">
        <NavLink
          to="/"
          end
          className={({ isActive }) =>
            `block rounded-md px-2 py-1.5 text-sm hover:bg-accent ${
              isActive ? "bg-accent font-medium" : ""
            }`
          }
        >
          所有工作区
        </NavLink>
        {workspaces?.map((ws) => (
          <NavLink
            key={ws.id}
            to={`/workspaces/${ws.id}`}
            className={({ isActive }) =>
              `block rounded-md px-2 py-1.5 text-sm hover:bg-accent truncate ${
                isActive ? "bg-accent font-medium" : ""
              }`
            }
          >
            {ws.name}
          </NavLink>
        ))}
      </nav>
      <div className="space-y-2 border-t pt-3">
        <StatusDot ok={healthOk} label="后端" />
        <StatusDot ok={wsStatus === "open"} label="WebSocket" />
      </div>
    </aside>
  );
}