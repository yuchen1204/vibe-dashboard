import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { getJson, postJson, putJson } from "@/lib/api";
import type { Review, ReviewDetail, CreateReviewInput, CreateFindingInput } from "@/types/api";

export function useReviewsByTodo(todoId: string) {
  return useQuery({
    queryKey: ["reviews", "todo", todoId],
    queryFn: () => getJson<Review[]>(`/api/reviews/todo/${todoId}`),
    enabled: !!todoId,
  });
}

export function useReviewsByJob(jobId: string) {
  return useQuery({
    queryKey: ["reviews", "job", jobId],
    queryFn: () => getJson<Review[]>(`/api/reviews/job/${jobId}`),
    enabled: !!jobId,
  });
}

export function useReviewDetail(reviewId: string | null) {
  return useQuery({
    queryKey: ["review", reviewId],
    queryFn: () => getJson<ReviewDetail>(`/api/reviews/${reviewId}`),
    enabled: !!reviewId,
  });
}

export function useCreateReview() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateReviewInput) =>
      postJson<Review>("/api/reviews", input),
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ["reviews", "todo", data.todo_id] });
    },
  });
}

/** 触发 LLM 代码审查（后台运行，结果通过 WS 推送） */
export function useTriggerReview() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: { job_id: string; todo_id: string }) =>
      postJson<Review>("/api/reviews/trigger", input),
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ["reviews", "todo", data.todo_id] });
      qc.invalidateQueries({ queryKey: ["reviews", "job", data.job_id] });
    },
  });
}

export function useAddFinding(reviewId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateFindingInput) =>
      postJson(`/api/reviews/${reviewId}/findings`, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["review", reviewId] });
    },
  });
}

export function useUpdateReviewSummary(reviewId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: { summary?: string; score?: number; total_findings?: number }) =>
      putJson(`/api/reviews/${reviewId}/summary`, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["review", reviewId] });
    },
  });
}