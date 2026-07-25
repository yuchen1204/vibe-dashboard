import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { del, getJson, postJson, putJson } from "@/lib/api";
import type { CreateTarget, Target, UpdateTarget } from "@/types/api";

export function useTargets(workspaceId: string) {
  return useQuery({
    queryKey: ["targets", workspaceId],
    queryFn: () => getJson<Target[]>(`/api/workspaces/${workspaceId}/targets`),
    enabled: !!workspaceId,
  });
}

export function useCreateTarget(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateTarget) =>
      postJson<Target>(`/api/workspaces/${workspaceId}/targets`, input),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: ["targets", workspaceId] }),
  });
}

export function useUpdateTarget(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateTarget }) =>
      putJson<Target>(`/api/targets/${id}`, input),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: ["targets", workspaceId] }),
  });
}

export function useDeleteTarget(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => del<void>(`/api/targets/${id}`),
    onSuccess: () =>
      qc.invalidateQueries({ queryKey: ["targets", workspaceId] }),
  });
}
