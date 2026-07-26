import { create } from "zustand";
import type { ReviewFinding } from "@/types/api";

export interface ReviewProgress {
  reviewId: string;
  jobId: string;
  todoId: string;
  status: "running" | "completed" | "failed";
  findings: ReviewFinding[];
  summary?: string;
  score?: number;
}

interface ReviewState {
  /** Map of review_id -> review progress */
  reviews: Record<string, ReviewProgress>;
  /** Map of todo_id -> latest review_id */
  reviewByTodo: Record<string, string>;
  setReviewStarted: (reviewId: string, jobId: string, todoId: string) => void;
  addFinding: (reviewId: string, finding: ReviewFinding) => void;
  setReviewCompleted: (reviewId: string, summary: string, score: number) => void;
  setReviewError: (reviewId: string, message: string) => void;
}

export const useReviewStore = create<ReviewState>((set) => ({
  reviews: {},
  reviewByTodo: {},
  setReviewStarted: (reviewId, jobId, todoId) =>
    set((state) => ({
      reviews: {
        ...state.reviews,
        [reviewId]: {
          reviewId,
          jobId,
          todoId,
          status: "running",
          findings: [],
        },
      },
      reviewByTodo: {
        ...state.reviewByTodo,
        [todoId]: reviewId,
      },
    })),
  addFinding: (reviewId, finding) =>
    set((state) => {
      const existing = state.reviews[reviewId];
      if (!existing) return state;
      return {
        reviews: {
          ...state.reviews,
          [reviewId]: {
            ...existing,
            findings: [...existing.findings, finding],
          },
        },
      };
    }),
  setReviewCompleted: (reviewId, summary, score) =>
    set((state) => {
      const existing = state.reviews[reviewId];
      if (!existing) return state;
      return {
        reviews: {
          ...state.reviews,
          [reviewId]: {
            ...existing,
            status: "completed",
            summary,
            score,
          },
        },
      };
    }),
  setReviewError: (reviewId, message) =>
    set((state) => {
      const existing = state.reviews[reviewId];
      if (!existing) return state;
      return {
        reviews: {
          ...state.reviews,
          [reviewId]: {
            ...existing,
            status: "failed",
            summary: message,
          },
        },
      };
    }),
}));