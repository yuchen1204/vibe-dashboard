import { Badge } from "@/components/ui/badge";

export function Sidebar() {
  return (
    <aside className="w-60 border-r bg-card min-h-screen p-4">
      <h2 className="text-lg font-semibold mb-4">Vibe Dashboard</h2>
      <nav className="space-y-2">
        <div className="flex items-center justify-between px-2 py-1.5 rounded-md hover:bg-accent cursor-pointer">
          <span className="text-sm">Workspaces</span>
          <Badge variant="secondary" className="text-xs">L2</Badge>
        </div>
      </nav>
      <div className="mt-8 text-xs text-muted-foreground px-2">
        基础设施层已就绪
      </div>
    </aside>
  );
}
