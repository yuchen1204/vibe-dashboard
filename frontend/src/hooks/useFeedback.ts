import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getJson, postJson } from "@/lib/api";
import type { ReviewFeedback, ReviewIteration } from "@/types/api";

export function useFeedbackByReview(reviewId: string) {
  return useQuery({
    queryKey: ["feedback", "review", reviewId],
    queryFn: () => getJson<ReviewFeedback[]>(`/api/reviews/${reviewId}/feedback`),
    enabled: !!reviewId,
  });
}

export function useAcceptFinding() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ findingId, targetId }: { findingId: string; targetId?: string }) =>
      postJson<ReviewFeedback>(`/api/feedback/${findingId}/accept`, {
        target_id: targetId,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["feedback"] });
    },
  });
}

export function useIgnoreFinding() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (findingId: string) =>
      postJson<ReviewFeedback>(`/api/feedback/${findingId}/ignore`, {}),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["feedback"] });
    },
  });
}

export function useIterationsByTodo(todoId: string) {
  return useQuery({
    queryKey: ["iterations", "todo", todoId],
    queryFn: () => getJson<ReviewIteration[]>(`/api/todos/${todoId}/iterations`),
    enabled: !!todoId,
  });
}

export function useTriggerAutoFix() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      todoId,
      prompt,
    }: {
      todoId: string;
      prompt?: string;
    }) => postJson<ReviewIteration>(`/api/todos/${todoId}/auto-fix`, {
      prompt,
    }),
    onSuccess: (_, { todoId }) => {
      qc.invalidateQueries({ queryKey: ["iterations", "todo", todoId] });
    },
  });
}

export function useTriggerAutoFixSync() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      todoId,
      prompt,
    }: {
      todoId: string;
      prompt?: string;
    }) =>
      postJson<ReviewIteration[]>(
        `/api/todos/${todoId}/auto-fix-sync`,
        { prompt },
      ),
    onSuccess: (_, { todoId }) => {
      qc.invalidateQueries({ queryKey: ["iterations", "todo", todoId] });
    },
  });
}