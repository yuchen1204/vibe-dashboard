import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getJson, postJson } from "@/lib/api";
import type { ExecutionJob, ExecuteTodo } from "@/types/api";

export function useJobs(workspaceId: string) {
  return useQuery({
    queryKey: ["jobs", workspaceId],
    queryFn: () => getJson<ExecutionJob[]>(`/api/workspaces/${workspaceId}/jobs`),
    enabled: !!workspaceId,
  });
}

export function useJob(jobId: string | null) {
  return useQuery({
    queryKey: ["job", jobId],
    queryFn: () => getJson<ExecutionJob>(`/api/jobs/${jobId}`),
    enabled: !!jobId,
    refetchInterval: (query) => {
      const data = query.state.data;
      if (!data) return 1000;
      if (data.status === "running" || data.status === "pending") return 1000;
      return false;
    },
  });
}

export function useExecuteTodo(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ todoId, input }: { todoId: string; input?: ExecuteTodo }) =>
      postJson<ExecutionJob>(`/api/todos/${todoId}/execute`, input ?? {}),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["jobs", workspaceId] });
      qc.invalidateQueries({ queryKey: ["todos", workspaceId] });
    },
  });
}

export function useCancelJob(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (jobId: string) => postJson<ExecutionJob>(`/api/jobs/${jobId}/cancel`, {}),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["jobs", workspaceId] });
      qc.invalidateQueries({ queryKey: ["todos", workspaceId] });
    },
  });
}