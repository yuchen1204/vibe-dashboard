import { create } from "zustand";

export interface JobRecord {
  jobId: string;
  todoId: string;
  status: string;
}

interface ExecutionState {
  /** Map of todo_id -> latest job info */
  jobByTodo: Record<string, JobRecord>;
  setJobStatus: (jobId: string, todoId: string, status: string) => void;
  removeJob: (todoId: string) => void;
}

export const useExecutionStore = create<ExecutionState>((set) => ({
  jobByTodo: {},
  setJobStatus: (jobId, todoId, status) =>
    set((state) => ({
      jobByTodo: {
        ...state.jobByTodo,
        [todoId]: { jobId, todoId, status },
      },
    })),
  removeJob: (todoId) =>
    set((state) => {
      const { [todoId]: _, ...rest } = state.jobByTodo;
      return { jobByTodo: rest };
    }),
}));