import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { useReviewDetail, useReviewsByTodo } from "@/hooks/useReview";
import { useFeedbackByReview, useAcceptFinding, useIgnoreFinding } from "@/hooks/useFeedback";
import { useReviewStore } from "@/stores/review";
import type { ReviewFeedback } from "@/types/api";
import { useState } from "react";

interface Props {
  todoId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const SEVERITY_COLORS: Record<string, string> = {
  critical: "bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300",
  major: "bg-orange-100 text-orange-800 dark:bg-orange-900 dark:text-orange-300",
  minor: "bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300",
  suggestion: "bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-300",
};

const SEVERITY_LABEL: Record<string, string> = {
  critical: "严重",
  major: "主要",
  minor: "次要",
  suggestion: "建议",
};

const ACTION_LABEL: Record<string, string> = {
  pending: "待处理",
  accepted: "已接受 ✓",
  ignored: "已忽略",
  auto_fix: "自动修复",
};

export function FeedbackDialog({ todoId, open, onOpenChange }: Props) {
  const { data: reviews } = useReviewsByTodo(todoId);
  const latestReview = reviews?.[0];
  const { data: reviewDetail } = useReviewDetail(latestReview?.id ?? null);
  const { data: feedbacks } = useFeedbackByReview(latestReview?.id ?? "");
  const acceptFinding = useAcceptFinding();
  const ignoreFinding = useIgnoreFinding();
  const [acceptingId, setAcceptingId] = useState<string | null>(null);

  const liveReviewId = useReviewStore((s) => s.reviewByTodo[todoId]);
  const liveReview = useReviewStore((s) => (liveReviewId ? s.reviews[liveReviewId] : undefined));

  const isRunning = liveReview?.status === "running";

  const feedbackMap = new Map<string, ReviewFeedback>();
  feedbacks?.forEach((f) => feedbackMap.set(f.finding_id, f));

  const handleAccept = (findingId: string) => {
    setAcceptingId(findingId);
    acceptFinding.mutate(
      { findingId },
      {
        onSettled: () => setAcceptingId(null),
      },
    );
  };

  const handleIgnore = (findingId: string) => {
    ignoreFinding.mutate(findingId);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle>
            审查反馈
            {isRunning && (
              <span className="ml-2 text-sm font-normal text-yellow-600 animate-pulse">
                · 审查中...
              </span>
            )}
            {reviewDetail?.score != null && !isRunning && (
              <span className="ml-2 text-sm font-normal text-muted-foreground">
                · 评分 {reviewDetail.score}/10
              </span>
            )}
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto space-y-4">
          {isRunning && (
            <div className="text-sm text-muted-foreground animate-pulse">
              AI 审查中，完成后可在此处处理 findings...
            </div>
          )}

          {reviewDetail?.summary && (
            <div className="bg-muted rounded-lg p-4">
              <h4 className="text-sm font-semibold mb-1">审查总结</h4>
              <p className="text-sm text-muted-foreground whitespace-pre-wrap">
                {reviewDetail.summary}
              </p>
            </div>
          )}

          {reviewDetail?.findings.map((finding) => {
            const feedback = feedbackMap.get(finding.id);
            const action = feedback?.action ?? "pending";

            return (
              <div
                key={finding.id}
                className={`border rounded-lg p-4 space-y-2 transition-opacity ${
                  action === "ignored"
                    ? "opacity-50"
                    : action === "accepted"
                      ? "border-green-300 dark:border-green-700"
                      : ""
                }`}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="flex items-center gap-2 min-w-0">
                    <span
                      className={`text-xs font-medium px-2 py-0.5 rounded-full shrink-0 ${
                        SEVERITY_COLORS[finding.severity] ?? SEVERITY_COLORS.minor
                      }`}
                    >
                      {SEVERITY_LABEL[finding.severity] ?? finding.severity}
                    </span>
                    <span className="text-xs text-muted-foreground shrink-0">
                      {finding.category}
                    </span>
                    <h4 className="text-sm font-medium truncate">{finding.title}</h4>
                  </div>
                  <span className="text-xs text-muted-foreground shrink-0">
                    {ACTION_LABEL[action] ?? action}
                  </span>
                </div>

                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <code className="bg-muted px-1.5 py-0.5 rounded text-xs">
                    {finding.file_path}
                  </code>
                  {finding.line_number && <span>: {finding.line_number}</span>}
                </div>

                {finding.description && (
                  <p className="text-sm text-muted-foreground">{finding.description}</p>
                )}

                {finding.suggestion && (
                  <div className="bg-green-50 dark:bg-green-950 border border-green-200 dark:border-green-800 rounded p-3">
                    <p className="text-xs font-medium text-green-700 dark:text-green-400 mb-1">
                      建议
                    </p>
                    <p className="text-sm text-green-600 dark:text-green-300">
                      {finding.suggestion}
                    </p>
                  </div>
                )}

                {action === "pending" && (
                  <div className="flex items-center gap-2 pt-1">
                    <Button
                      size="sm"
                      variant="default"
                      className="h-7 text-xs"
                      onClick={() => handleAccept(finding.id)}
                      disabled={acceptingId === finding.id}
                    >
                      {acceptingId === finding.id ? "处理中..." : "接受并创建 Todo"}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7 text-xs"
                      onClick={() => handleIgnore(finding.id)}
                    >
                      忽略
                    </Button>
                  </div>
                )}
              </div>
            );
          })}

          {reviewDetail?.findings.length === 0 && reviewDetail?.status === "completed" && (
            <div className="py-8 text-center text-sm text-muted-foreground">
              未发现任何问题 ✓
            </div>
          )}
        </div>

        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">关闭</Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}