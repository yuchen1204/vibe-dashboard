import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "@/lib/query";
import { Sidebar } from "@/components/layout/Sidebar";
import { useGlobalStatus } from "@/hooks/useGlobalStatus";
import { WorkspacesPage } from "@/pages/WorkspacesPage";
import { WorkspaceViewPage } from "@/pages/WorkspaceViewPage";

function AppShell() {
  const { healthOk } = useGlobalStatus();

  return (
    <div className="flex min-h-screen">
      <Sidebar healthOk={healthOk} />
      <main className="flex-1">
        <Routes>
          <Route path="/" element={<WorkspacesPage />} />
          <Route path="/workspaces/:wid" element={<WorkspaceViewPage />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </main>
    </div>
  );
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <AppShell />
      </BrowserRouter>
    </QueryClientProvider>
  );
}

export default App;