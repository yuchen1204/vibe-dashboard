import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "@/lib/query";
import { Sidebar } from "@/components/layout/Sidebar";
import { HomePage } from "@/pages/HomePage";

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div className="flex min-h-screen">
        <Sidebar />
        <HomePage />
      </div>
    </QueryClientProvider>
  );
}

export default App;
