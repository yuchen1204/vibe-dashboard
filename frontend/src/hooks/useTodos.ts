import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { del, getJson, postJson, putJson } from "@/lib/api";
import type { CreateTodo, Todo, UpdateTodo } from "@/types/api";

export function useTodos(workspaceId: string) {
  return useQuery({
    queryKey: ["todos", workspaceId],
    queryFn: () => getJson<Todo[]>(`/api/workspaces/${workspaceId}/todos`),
    enabled: !!workspaceId,
  });
}

export function useCreateTodo(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ targetId, input }: { targetId: string; input: CreateTodo }) =>
      postJson<Todo>(`/api/targets/${targetId}/todos`, input),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: ["todos", workspaceId] }),
  });
}

export function useUpdateTodo(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateTodo }) =>
      putJson<Todo>(`/api/todos/${id}`, input),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: ["todos", workspaceId] }),
  });
}

export function useDeleteTodo(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => del<void>(`/api/todos/${id}`),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: ["todos", workspaceId] }),
  });
}
